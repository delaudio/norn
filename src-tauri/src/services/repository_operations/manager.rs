use super::*;
use crate::local_review::{GitRunControl, LocalReviewCancellation};
use crate::services::repository_reader::context;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant, SystemTime},
};

struct Plan {
    request: PreviewParams,
    root: PathBuf,
    before: Checkout,
    configuration: String,
    configured_version: String,
    expires: Instant,
}
struct Job {
    value: Mutex<Operation>,
    cancellation: LocalReviewCancellation,
    done: AtomicBool,
}
pub(crate) struct Manager {
    plans: Mutex<HashMap<String, Plan>>,
    jobs: Mutex<HashMap<String, Arc<Job>>>,
    stop: Arc<AtomicBool>,
    active: Arc<AtomicBool>,
    cancellation: LocalReviewCancellation,
}
fn make_control(cancel: LocalReviewCancellation, seconds: u64) -> GitRunControl {
    GitRunControl::new(Duration::from_secs(seconds), cancel, "Repository operation")
}
fn token() -> Result<String, String> {
    let mut bytes = [0u8; 32];
    getrandom::getrandom(&mut bytes).map_err(|_| invalid())?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}
fn phase(job: &Job, state: OperationState, message: &str) {
    if let Ok(mut value) = job.value.lock() {
        value.state = state;
        value.message = message.into();
    }
}
impl Manager {
    pub fn new() -> Self {
        Self {
            plans: Mutex::new(HashMap::new()),
            jobs: Mutex::new(HashMap::new()),
            stop: Arc::new(AtomicBool::new(false)),
            active: Arc::new(AtomicBool::new(false)),
            cancellation: LocalReviewCancellation::new(),
        }
    }
    pub fn active(&self) -> bool {
        self.active.load(Ordering::Acquire)
    }
    pub fn stop(&self) {
        self.stop.store(true, Ordering::Release);
        self.cancellation.cancel();
        if let Ok(jobs) = self.jobs.lock() {
            for job in jobs.values() {
                job.cancellation.cancel();
            }
        }
        if let Ok(mut plans) = self.plans.lock() {
            plans.clear();
        }
        let deadline = Instant::now() + Duration::from_secs(3);
        while self.active() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(10));
        }
    }
    pub fn list(&self, params: ListParams) -> Result<Page, String> {
        params.validate()?;
        let control = make_control(self.cancellation.clone(), 20);
        let source = match context::resolve(&params.repository, &control) {
            Ok(source) => source,
            Err(message) => {
                let candidates: Vec<_> = crate::config::load()
                    .repos
                    .into_iter()
                    .filter(|r| {
                        r.provider == params.repository.provider
                            && r.workspace == params.repository.workspace
                            && r.repo == params.repository.repo
                    })
                    .collect();
                let missing = candidates.len() == 1
                    && candidates[0]
                        .local_path
                        .as_ref()
                        .is_none_or(|p| !Path::new(p).exists());
                return Ok(Page {
                    checkout: Checkout {
                        repository: params.repository.clone(),
                        context: None,
                        state: if missing {
                            CheckoutState::Missing
                        } else {
                            CheckoutState::Invalid
                        },
                        branch: None,
                        head_sha: None,
                        upstream: None,
                        stash_sha: None,
                        dirty: false,
                        filtered: false,
                        conflicted: false,
                        pending: false,
                        version: None,
                        message: Some(message),
                    },
                    request: params,
                    branches: vec![],
                    total: 0,
                    next_offset: None,
                });
            }
        };
        let context = Context {
            repository: params.repository.clone(),
            root_id: source.root_id,
        };
        let snapshot = state::snapshot(&context, &source.root, &control)?;
        if params
            .version
            .as_ref()
            .is_some_and(|v| snapshot.checkout.version.as_ref() != Some(v))
        {
            return Err(
                "Repository state changed. Start branch paging from the first page.".into(),
            );
        }
        if context::resolve_context(&context, &control)? != source.root {
            return Err(invalid());
        }
        let total = snapshot.branches.len();
        if params.offset > total {
            return Err(invalid());
        }
        let end = params.offset.saturating_add(params.limit).min(total);
        Ok(Page {
            branches: snapshot.branches[params.offset..end].to_vec(),
            checkout: snapshot.checkout,
            total,
            next_offset: (end < total).then_some(end),
            request: params,
        })
    }
    pub fn preview(&self, params: PreviewParams) -> Result<Prepared, String> {
        params.validate()?;
        if self.stop.load(Ordering::Acquire) || self.active() {
            return Err("Finish the current repository operation first.".into());
        }
        let control = make_control(self.cancellation.clone(), 20);
        let configured_version = configured_fingerprint(&params.context.repository)?;
        let root = context::resolve_context(&params.context, &control)?;
        let before = state::snapshot(&params.context, &root, &control)?;
        let summary = validate_action(&root, &before, &params.action, &control)?;
        let checked = state::snapshot(&params.context, &root, &control)?;
        if before.checkout.version != checked.checkout.version
            || context::resolve_context(&params.context, &control)? != root
            || configured_fingerprint(&params.context.repository)? != configured_version
        {
            return Err(invalid());
        }
        let mut warnings=vec!["Git may use configured hooks, filters and credential helpers after confirmation. No remote push is performed.".into()];
        if before.checkout.filtered {
            warnings.push("The preview compares raw content; Git verifies filtered cleanliness after confirmation.".into());
        }
        if matches!(params.action, Action::Fetch {}) {
            warnings.push("Fetch prunes stale origin-tracking references; local branches and local commits are preserved.".into());
        }
        if matches!(params.action, Action::Stash {}) {
            warnings.push("Stash saves tracked changes and untracked files, then clears them from the checkout. Ignored files remain in place.".into());
        }
        let token = token()?;
        let mut plans = self.plans.lock().map_err(|_| invalid())?;
        plans.retain(|_, p| p.expires > Instant::now());
        if plans.len() >= 8 {
            return Err("Too many pending repository approvals. Retry shortly.".into());
        }
        plans.insert(
            token.clone(),
            Plan {
                request: params.clone(),
                root,
                before: before.checkout.clone(),
                configuration: before.configuration.digest,
                configured_version,
                expires: Instant::now() + Duration::from_secs(30),
            },
        );
        Ok(Prepared {
            request: params,
            token,
            expires_in_ms: 30000,
            before: before.checkout,
            summary,
            warnings,
        })
    }
    pub fn apply(
        &self,
        params: ApplyParams,
        readers: [Arc<AtomicUsize>; 2],
    ) -> Result<Applied, String> {
        params.validate()?;
        let plan = {
            let mut plans = self.plans.lock().map_err(|_| invalid())?;
            let plan = plans.get(&params.token).ok_or_else(invalid)?;
            if plan.request.context != params.context || plan.request.action != params.action {
                return Err(invalid());
            }
            plans.remove(&params.token).ok_or_else(invalid)?
        };
        if plan.expires <= Instant::now() || self.stop.load(Ordering::Acquire) {
            return Err("Repository approval expired. Preview the action again.".into());
        }
        if self
            .active
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Err(
                "A repository write is already active. Preview again after it finishes.".into(),
            );
        }
        let id = match token() {
            Ok(id) => id,
            Err(e) => {
                self.active.store(false, Ordering::Release);
                return Err(e);
            }
        };
        let value = Operation {
            operation_id: id.clone(),
            context: params.context.clone(),
            action: params.action.clone(),
            state: OperationState::Starting,
            before: plan.before.clone(),
            after: None,
            exit_code: None,
            state_changed: None,
            recovery_required: false,
            message: "Approval consumed; checking the current checkout.".into(),
        };
        let job = Arc::new(Job {
            value: Mutex::new(value.clone()),
            cancellation: LocalReviewCancellation::new(),
            done: AtomicBool::new(false),
        });
        {
            let mut jobs = match self.jobs.lock() {
                Ok(jobs) => jobs,
                Err(_) => {
                    self.active.store(false, Ordering::Release);
                    return Err(invalid());
                }
            };
            if jobs.len() >= 32 {
                jobs.retain(|_, j| !j.done.load(Ordering::Acquire));
            }
            jobs.insert(id.clone(), job.clone());
        }
        let active = self.active.clone();
        let stop = self.stop.clone();
        let worker = job.clone();
        let spawned = thread::Builder::new()
            .name("norn-repository-operation".into())
            .spawn(move || {
                execute(plan, &worker, &stop, &active, &readers);
                worker.done.store(true, Ordering::Release);
                active.store(false, Ordering::Release);
            });
        if spawned.is_err() {
            job.done.store(true, Ordering::Release);
            self.active.store(false, Ordering::Release);
            phase(
                &job,
                OperationState::Failed,
                "The owned Git worker could not start. Preview again explicitly.",
            );
            return Err("The repository worker is unavailable.".into());
        }
        Ok(Applied {
            request: params,
            operation: value,
        })
    }
    pub fn status(&self, params: JobParams, cancel: bool) -> Result<Status, String> {
        params.validate()?;
        let job=self.jobs.lock().map_err(|_| invalid())?.get(&params.operation_id).cloned()
            .ok_or_else(|| "This operation is no longer known. Refresh the actual checkout; no action was replayed.".to_string())?;
        let value = job.value.lock().map_err(|_| invalid())?;
        if value.context != params.context {
            return Err(invalid());
        }
        if cancel && !job.done.load(Ordering::Acquire) {
            job.cancellation.cancel();
        }
        Ok(Status {
            request: params,
            operation: value.clone(),
            cancel_requested: job.cancellation.is_cancelled(),
        })
    }
}
fn configured_fingerprint(repository: &RepositoryIdentity) -> Result<String, String> {
    use sha2::{Digest, Sha256};
    let entries: Vec<_> = crate::config::load()
        .repos
        .into_iter()
        .filter(|entry| {
            entry.provider == repository.provider
                && entry.workspace == repository.workspace
                && entry.repo == repository.repo
        })
        .collect();
    if entries.len() != 1 {
        return Err(invalid());
    }
    let bytes = serde_json::to_vec(
        &serde_json::json!({"repository":repository,"path":entries[0].local_path}),
    )
    .map_err(|_| invalid())?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

fn validate_action(
    root: &Path,
    snapshot: &state::Snapshot,
    action: &Action,
    control: &GitRunControl,
) -> Result<String, String> {
    if !matches!(action, Action::Fetch {})
        && (snapshot.checkout.pending || snapshot.checkout.conflicted)
    {
        return Err("Finish the existing Git operation or resolve its conflicts externally before changing this checkout.".into());
    }
    match action {
        Action::Checkout {
            reference,
            branch_kind,
        } => {
            if snapshot.checkout.dirty && !snapshot.checkout.filtered {
                return Err(
                    "Cannot checkout while the repository has uncommitted or untracked changes."
                        .into(),
                );
            }
            let selected = snapshot
                .branches
                .iter()
                .find(|b| b.reference == *reference && b.kind == *branch_kind)
                .ok_or_else(|| {
                    "The selected branch disappeared. Refresh the branch list.".to_string()
                })?;
            if let Some(local) = reference.strip_prefix("refs/remotes/origin/") {
                let existing = snapshot
                    .branches
                    .iter()
                    .find(|b| b.reference == format!("refs/heads/{local}"));
                return Ok(if let Some(existing) = existing {
                    format!(
                        "Checkout existing local branch {local} at {}; keep its current commits.",
                        existing.oid
                    )
                } else {
                    format!(
                        "Create local tracking branch {local} from {} at {}.",
                        selected.reference, selected.oid
                    )
                });
            }
            Ok(if *branch_kind == BranchKind::Local {
                format!(
                    "Checkout local branch {} at {}.",
                    selected.name, selected.oid
                )
            } else {
                format!(
                    "Checkout {} at {} with detached HEAD.",
                    selected.reference, selected.oid
                )
            })
        }
        Action::Fetch {} => {
            fetch_boundary(root, &snapshot.configuration, "origin", control)?;
            Ok("Fetch origin and prune stale origin-tracking branches.".into())
        }
        Action::Pull {} => {
            if snapshot.checkout.head_sha.is_none() || snapshot.checkout.branch.is_none() {
                return Err("Pull requires a current local branch.".into());
            }
            if snapshot.checkout.dirty && !snapshot.checkout.filtered {
                return Err(
                    "Cannot pull while the repository has uncommitted or untracked changes.".into(),
                );
            }
            let upstream = snapshot
                .checkout
                .upstream
                .as_ref()
                .ok_or_else(|| "This branch has no configured upstream.".to_string())?;
            let branch = snapshot.checkout.branch.as_ref().ok_or_else(invalid)?;
            let key = format!("branch.{branch}.remote");
            let remote = git::text(
                root,
                &["config", "--get", &key],
                &snapshot.configuration,
                control,
            )?;
            let destination = if remote == "." {
                "the local repository".to_string()
            } else {
                fetch_boundary(root, &snapshot.configuration, &remote, control)?;
                let urls = git::text(
                    root,
                    &["remote", "get-url", "--all", &remote],
                    &snapshot.configuration,
                    control,
                )?;
                let urls: Vec<_> = urls.lines().collect();
                if urls.len() != 1 {
                    return Err(invalid());
                }
                let (provider, owner, repo) =
                    crate::local_repo::parse_git_remote(urls[0]).map_err(|_| invalid())?;
                RepositoryIdentity {
                    provider,
                    workspace: owner.clone(),
                    repo: repo.clone(),
                }
                .validate()?;
                let provider = match provider {
                    crate::config::ReviewProvider::Github => "GitHub",
                    crate::config::ReviewProvider::Bitbucket => "Bitbucket",
                };
                format!("{provider} {owner}/{repo} via remote {remote}")
            };
            Ok(format!("Pull {upstream} from {destination} with fast-forward only; preserve local commits on divergence."))
        }
        Action::Stash {} => {
            if snapshot.checkout.head_sha.is_none() {
                return Err("Stash requires an existing commit.".into());
            }
            Ok(if snapshot.checkout.dirty {
                "Stash tracked changes and untracked files; preserve ignored files.".into()
            } else {
                "The checkout is clean; stash makes no change.".into()
            })
        }
    }
}
fn fetch_boundary(
    root: &Path,
    cfg: &git::Configuration,
    remote: &str,
    control: &GitRunControl,
) -> Result<(), String> {
    if remote.starts_with('-') || !safe_reference(&format!("refs/remotes/{remote}/check")) {
        return Err(invalid());
    }
    let key = format!("remote.{remote}.fetch");
    let values = git::text(root, &["config", "--get-all", &key], cfg, control)?;
    let prefix = format!("refs/remotes/{remote}/");
    let mut count = 0;
    for line in values.lines() {
        count += 1;
        if let Some(source) = line.strip_prefix('^') {
            if !source.starts_with("refs/heads/") {
                return Err(invalid());
            }
            git::checked(
                root,
                &["check-ref-format", "--refspec-pattern", source],
                4096,
                Some(cfg),
                control,
            )?;
            continue;
        }
        let line = line.strip_prefix('+').unwrap_or(line);
        let Some((source, destination)) = line.split_once(':') else {
            return Err("Fetch configuration must target remote-tracking branches.".into());
        };
        if !source.starts_with("refs/heads/")
            || !destination.starts_with(&prefix)
            || source.bytes().filter(|b| *b == b'*').count()
                != destination.bytes().filter(|b| *b == b'*').count()
        {
            return Err("Fetch configuration must preserve local branches and local tags.".into());
        }
        for reference in [source, destination] {
            git::checked(
                root,
                &["check-ref-format", "--refspec-pattern", reference],
                4096,
                Some(cfg),
                control,
            )?;
        }
    }
    if count == 0 || count > 128 {
        return Err(invalid());
    }
    Ok(())
}
fn execute(
    plan: Plan,
    job: &Job,
    stop: &AtomicBool,
    active: &AtomicBool,
    readers: &[Arc<AtomicUsize>; 2],
) {
    phase(
        job,
        OperationState::Checking,
        "Checking the approved checkout and Git configuration.",
    );
    let control = make_control(job.cancellation.clone(), 90);
    let mut attempted = false;
    let mut exit = None;
    let mut expected_checkout: Option<(Option<String>, String)> = None;
    let result = (|| -> Result<(), String> {
        control.check()?;
        let read_deadline = Instant::now() + Duration::from_secs(20);
        while readers
            .iter()
            .any(|counter| counter.load(Ordering::Acquire) > 0)
        {
            control.check()?;
            if stop.load(Ordering::Acquire) || Instant::now() >= read_deadline {
                return Err("Repository reads did not settle before the write deadline.".into());
            }
            thread::sleep(Duration::from_millis(10));
        }
        if stop.load(Ordering::Acquire) {
            return Err("Workspace is shutting down.".into());
        }
        if context::resolve_context(&plan.request.context, &control)? != plan.root {
            return Err(invalid());
        }
        let current = state::snapshot(&plan.request.context, &plan.root, &control)?;
        if current.checkout.version != plan.before.version
            || current.configuration.digest != plan.configuration
            || configured_fingerprint(&plan.request.context.repository)? != plan.configured_version
        {
            return Err("Checkout or Git configuration changed after preview. Approval was consumed; preview again.".into());
        }
        validate_action(&plan.root, &current, &plan.request.action, &control)?;
        // Filtered cleanliness can execute normal configured helpers only now,
        // after explicit approval of the fingerprinted Git configuration.
        if matches!(
            plan.request.action,
            Action::Checkout { .. } | Action::Pull {}
        ) {
            let status = git::run(
                &plan.root,
                &["status", "--porcelain=v1", "-z", "--untracked-files=all"],
                1024 * 1024,
                None,
                false,
                &control,
            )?;
            if !status.status.success() || !status.stdout.is_empty() {
                return Err(
                    "Git reports uncommitted or untracked changes. The action was not applied."
                        .into(),
                );
            }
        }
        control.check()?;
        if state::snapshot(&plan.request.context, &plan.root, &control)?
            .checkout
            .version
            != plan.before.version
            || git::configuration(&plan.root, &control)?.digest != plan.configuration
            || context::resolve_context(&plan.request.context, &control)? != plan.root
            || configured_fingerprint(&plan.request.context.repository)? != plan.configured_version
        {
            return Err(invalid());
        }
        let mut args: Vec<String> = Vec::new();
        match &plan.request.action {
            Action::Checkout {
                reference,
                branch_kind,
            } => {
                let selected = current
                    .branches
                    .iter()
                    .find(|branch| branch.reference == *reference && branch.kind == *branch_kind)
                    .ok_or_else(invalid)?;
                let local = reference
                    .strip_prefix("refs/remotes/origin/")
                    .or_else(|| reference.strip_prefix("refs/heads/"));
                let target = local
                    .and_then(|name| {
                        current
                            .branches
                            .iter()
                            .find(|branch| branch.reference == format!("refs/heads/{name}"))
                    })
                    .unwrap_or(selected);
                expected_checkout = Some((local.map(ToOwned::to_owned), target.oid.clone()));
                args.extend([
                    "switch".into(),
                    "--no-guess".into(),
                    "--no-overwrite-ignore".into(),
                ]);
                if let Some(local) = reference.strip_prefix("refs/remotes/origin/") {
                    if current
                        .branches
                        .iter()
                        .any(|b| b.reference == format!("refs/heads/{local}"))
                    {
                        args.extend(["--".into(), local.into()]);
                    } else {
                        args.extend([
                            "--track".into(),
                            "-c".into(),
                            local.into(),
                            "--".into(),
                            reference.clone(),
                        ]);
                    }
                } else if *branch_kind == BranchKind::Local {
                    args.extend([
                        "--".into(),
                        reference
                            .strip_prefix("refs/heads/")
                            .ok_or_else(invalid)?
                            .into(),
                    ]);
                } else {
                    args.extend(["--detach".into(), "--".into(), reference.clone()]);
                }
            }
            Action::Fetch {} => args.extend(
                [
                    "fetch",
                    "--quiet",
                    "--prune",
                    "--no-prune-tags",
                    "--no-recurse-submodules",
                    "origin",
                ]
                .map(String::from),
            ),
            Action::Pull {} => {
                let branch = current.checkout.branch.as_ref().ok_or_else(invalid)?;
                let key = format!("branch.{branch}.remote");
                let remote = git::text(
                    &plan.root,
                    &["config", "--get", &key],
                    &current.configuration,
                    &control,
                )?;
                if remote != "." {
                    phase(job, OperationState::Running, "Fetching the configured upstream; merge starts only after a fresh checkout check.");
                    attempted = true;
                    let output = git::run(
                        &plan.root,
                        &[
                            "fetch",
                            "--quiet",
                            "--no-prune",
                            "--no-prune-tags",
                            "--no-recurse-submodules",
                            &remote,
                        ],
                        256 * 1024,
                        None,
                        false,
                        &control,
                    )?;
                    exit = output.status.code();
                    if !output.status.success() {
                        return Err("Upstream fetch failed. No merge was started; inspect any updated remote references.".into());
                    }
                }
                control.check()?;
                let after_fetch = state::snapshot(&plan.request.context, &plan.root, &control)?;
                if after_fetch.working_version != current.working_version
                    || after_fetch.configuration.digest != plan.configuration
                    || context::resolve_context(&plan.request.context, &control)? != plan.root
                    || configured_fingerprint(&plan.request.context.repository)?
                        != plan.configured_version
                {
                    return Err(
                        "The checkout or configuration changed during fetch. No merge was started."
                            .into(),
                    );
                }
                let upstream = current.checkout.upstream.as_ref().ok_or_else(invalid)?;
                let dirty = git::run(
                    &plan.root,
                    &["status", "--porcelain=v1", "-z", "--untracked-files=all"],
                    1024 * 1024,
                    None,
                    false,
                    &control,
                )?;
                if !dirty.status.success() || !dirty.stdout.is_empty() {
                    return Err(
                        "Git reports local changes after fetch. No merge was started.".into(),
                    );
                }
                control.check()?;
                if state::snapshot(&plan.request.context, &plan.root, &control)?.working_version
                    != current.working_version
                    || git::configuration(&plan.root, &control)?.digest != plan.configuration
                    || context::resolve_context(&plan.request.context, &control)? != plan.root
                    || configured_fingerprint(&plan.request.context.repository)?
                        != plan.configured_version
                {
                    return Err(
                        "The checkout changed before fast-forward. No merge was started.".into(),
                    );
                }
                phase(job, OperationState::Running, "Applying the verified upstream with fast-forward only; ignored files and local commits stay protected.");
                args.extend([
                    "merge".into(),
                    "--ff-only".into(),
                    "--no-autostash".into(),
                    "--no-overwrite-ignore".into(),
                    "--".into(),
                    upstream.clone(),
                ]);
            }
            Action::Stash {} => {
                if !current.checkout.dirty {
                    return Ok(());
                }
                let timestamp = SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .map_err(|_| invalid())?
                    .as_secs();
                args.extend([
                    "stash".into(),
                    "push".into(),
                    "--include-untracked".into(),
                    "-m".into(),
                    format!("norn repository view stash {timestamp}"),
                ]);
            }
        }
        phase(
            job,
            OperationState::Running,
            "Running the approved Git operation. Cancel stops only its owned process tree.",
        );
        attempted = true;
        let args: Vec<_> = args.iter().map(String::as_str).collect();
        let output = git::run(&plan.root, &args, 256 * 1024, None, false, &control)?;
        exit = output.status.code();
        if !output.status.success() {
            return Err("Git rejected the operation. Inspect the refreshed state; local work was not forcibly discarded.".into());
        }
        Ok(())
    })();
    phase(
        job,
        OperationState::Inspecting,
        "Inspecting the actual checkout after the operation.",
    );
    let observed = (|| -> Result<Checkout, String> {
        if stop.load(Ordering::Acquire) {
            return Err("Workspace is shutting down.".into());
        }
        // Reconciliation reads the same complete snapshot as preview (including
        // config, refs, index and pending-operation paths). Use its bounded read
        // budget too: Windows process startup can exhaust five seconds even for
        // a small, clean checkout. Never retry the write or infer success here.
        let control = make_control(LocalReviewCancellation::new(), 20);
        if context::resolve_context(&plan.request.context, &control)? != plan.root {
            return Err(invalid());
        }
        Ok(state::snapshot(&plan.request.context, &plan.root, &control)?.checkout)
    })();
    if let Ok(mut value) = job.value.lock() {
        value.exit_code = exit;
        match observed {
            Ok(after) => {
                value.state_changed = Some(after.version != plan.before.version);
                value.after = Some(after);
            }
            Err(_) => {
                value.recovery_required = attempted || result.is_ok();
            }
        }
        let checkout_drift = result.is_ok()
            && expected_checkout.as_ref().is_some_and(|(branch, head)| {
                value.after.as_ref().is_some_and(|after| {
                    after.branch != *branch || after.head_sha.as_ref() != Some(head)
                })
            });
        let canceled = job.cancellation.is_cancelled() || stop.load(Ordering::Acquire);
        value.state = if checkout_drift || (value.after.is_none() && (attempted || result.is_ok()))
        {
            OperationState::Uncertain
        } else if result.is_ok() {
            OperationState::Completed
        } else if canceled {
            OperationState::Canceled
        } else {
            OperationState::Failed
        };
        value.recovery_required |= checkout_drift || (attempted && result.is_err());
        value.message = if checkout_drift {
            "Git exited successfully, but the observed branch or commit differs from the approved checkout. Inspect actual state before another action; no replay or rollback occurred.".into()
        } else if value.after.is_none() && (attempted || result.is_ok()) {
            "The Git process ended but the checkout could not be verified. Inspect its actual state before approving another action. No replay or rollback occurred.".into()
        } else {
            match result {
            Ok(())=>"Git finished; the actual checkout state is shown. No operation will be replayed.".into(),
            Err(message)=>format!("{message} Refresh and inspect the checkout before explicitly approving another action. No automatic retry or rollback occurred."),
        }
        };
        active.store(false, Ordering::Release);
        job.done.store(true, Ordering::Release);
    }
}

impl Drop for Manager {
    fn drop(&mut self) {
        self.stop();
    }
}
