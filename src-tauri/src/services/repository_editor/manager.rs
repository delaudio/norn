use super::*;
use integration::Integration;
use std::{
    collections::HashMap,
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex,
    },
    thread,
};

struct Plan {
    params: PrepareParams,
    proof: FileProof,
    integration: Integration,
    expires: Instant,
}
struct Job {
    editor: Mutex<Editor>,
    cancel: AtomicBool,
    done: AtomicBool,
    source_cancel: LocalReviewCancellation,
    claim: Mutex<()>,
    committed: AtomicBool,
}
pub(crate) struct Manager {
    plans: Mutex<HashMap<String, Plan>>,
    jobs: Mutex<HashMap<String, Arc<Job>>>,
    stop: Arc<AtomicBool>,
    cancellation: LocalReviewCancellation,
    terminal: Arc<AtomicBool>,
    gui: Arc<AtomicUsize>,
    active: Arc<AtomicUsize>,
}
impl Manager {
    pub fn new() -> Self {
        Self {
            plans: Mutex::new(HashMap::new()),
            jobs: Mutex::new(HashMap::new()),
            stop: Arc::new(AtomicBool::new(false)),
            cancellation: LocalReviewCancellation::new(),
            terminal: Arc::new(AtomicBool::new(false)),
            gui: Arc::new(AtomicUsize::new(0)),
            active: Arc::new(AtomicUsize::new(0)),
        }
    }
    pub fn prepare(&self, params: PrepareParams) -> Result<Prepared, String> {
        if self.stop.load(Ordering::Acquire) || self.terminal.load(Ordering::Acquire) {
            return Err("Finish the current editor before opening another file.".into());
        }
        let control = control(self.cancellation.clone());
        let proof = file(&params, &control)?;
        let selected = integration::select(&params, &proof.canonical, &control)?;
        let Some(integration) = selected else {
            return Ok(Prepared {request:params,token:None,integration:None,expires_in_ms:0,message:Some("This editor integration is unavailable. Configure VISUAL or EDITOR for terminal editing.".into())});
        };
        let name = match &integration {
            Integration::Command(value) => value.label.clone(),
            #[cfg(windows)]
            Integration::System => "System application".into(),
        };
        let mut plans = self.plans.lock().map_err(|_| invalid())?;
        plans.retain(|_, plan| plan.expires > Instant::now());
        if plans.len() >= 8 {
            return Err("Too many pending editor actions. Retry shortly.".into());
        }
        let token = token()?;
        plans.insert(
            token.clone(),
            Plan {
                params: params.clone(),
                proof,
                integration,
                expires: Instant::now() + Duration::from_secs(30),
            },
        );
        let message = (name == "System application" && params.line.is_some()).then(|| {
            "The system application opens the file without selecting a source line.".into()
        });
        Ok(Prepared {
            request: params,
            token: Some(token),
            integration: Some(name),
            expires_in_ms: 30000,
            message,
        })
    }
    pub fn open(&self, params: OpenParams) -> Result<Opened, String> {
        params.validate()?;
        let plan = {
            let mut plans = self.plans.lock().map_err(|_| invalid())?;
            let selected = plans.get(&params.token).ok_or_else(invalid)?;
            if selected.params.context != params.context {
                return Err(invalid());
            }
            plans.remove(&params.token).ok_or_else(invalid)?
        };
        if plan.expires <= Instant::now() || self.stop.load(Ordering::Acquire) {
            return Err("The editor action expired. Select the file again.".into());
        }
        let terminal = plan.params.mode == Mode::Terminal;
        if terminal {
            if self
                .terminal
                .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                .is_err()
            {
                return Err("A terminal editor is already running.".into());
            }
        } else if self
            .gui
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| {
                (n < 4).then_some(n + 1)
            })
            .is_err()
        {
            return Err("Four editor launchers are already active.".into());
        }
        let id = match token() {
            Ok(id) => id,
            Err(error) => {
                if terminal {
                    self.terminal.store(false, Ordering::Release);
                } else {
                    self.gui.fetch_sub(1, Ordering::AcqRel);
                }
                return Err(error);
            }
        };
        let name = match &plan.integration {
            Integration::Command(value) => value.label.clone(),
            #[cfg(windows)]
            Integration::System => "System application".into(),
        };
        let editor = Editor {
            launch_id: id.clone(),
            context: plan.params.context.clone(),
            path: plan.params.path.clone(),
            line: plan.params.line,
            mode: plan.params.mode.clone(),
            state: State::Starting,
            integration: name,
            exit_code: None,
            message: None,
        };
        let job = Arc::new(Job {
            editor: Mutex::new(editor.clone()),
            cancel: AtomicBool::new(false),
            done: AtomicBool::new(false),
            source_cancel: LocalReviewCancellation::new(),
            claim: Mutex::new(()),
            committed: AtomicBool::new(false),
        });
        {
            let mut jobs = self.jobs.lock().map_err(|_| invalid())?;
            if jobs.len() >= 32 {
                jobs.retain(|_, job| !job.done.load(Ordering::Acquire));
            }
            jobs.insert(id, job.clone());
        }
        self.active.fetch_add(1, Ordering::AcqRel);
        let stop = self.stop.clone();
        let cancellation = self.cancellation.clone();
        let active = self.active.clone();
        let terminal_active = self.terminal.clone();
        let gui_active = self.gui.clone();
        thread::Builder::new()
            .name("norn-editor".into())
            .spawn(move || {
                let result = run(&plan, &job, &stop, &cancellation);
                if let Ok(mut editor) = job.editor.lock() {
                    match result {
                        Ok((state, code, message)) => {
                            editor.state = state;
                            editor.exit_code = code;
                            editor.message = message;
                        }
                        Err(message) => {
                            editor.state = if stop.load(Ordering::Acquire)
                                || job.cancel.load(Ordering::Acquire)
                            {
                                State::Canceled
                            } else {
                                State::Failed
                            };
                            editor.message = Some(message);
                        }
                    }
                }
                job.done.store(true, Ordering::Release);
                if terminal {
                    terminal_active.store(false, Ordering::Release);
                } else {
                    gui_active.fetch_sub(1, Ordering::AcqRel);
                }
                active.fetch_sub(1, Ordering::AcqRel);
            })
            .map_err(|_| {
                self.active.fetch_sub(1, Ordering::AcqRel);
                if terminal {
                    self.terminal.store(false, Ordering::Release);
                } else {
                    self.gui.fetch_sub(1, Ordering::AcqRel);
                }
                if let Ok(mut jobs) = self.jobs.lock() {
                    jobs.remove(&editor.launch_id);
                }
                "The editor worker is unavailable.".to_owned()
            })?;
        Ok(Opened {
            request: params,
            editor,
        })
    }
    pub fn status(&self, params: StatusParams, cancel: bool) -> Result<Status, String> {
        params.validate()?;
        let job = self
            .jobs
            .lock()
            .map_err(|_| invalid())?
            .get(&params.launch_id)
            .cloned()
            .ok_or_else(invalid)?;
        let editor = job.editor.lock().map_err(|_| invalid())?.clone();
        if editor.context != params.context {
            return Err(invalid());
        }
        if cancel {
            let _claim = job.claim.lock().map_err(|_| invalid())?;
            if editor.mode != Mode::Terminal && job.committed.load(Ordering::Acquire) {
                return Err(
                    "A launched graphical application is controlled by its own window.".into(),
                );
            }
            job.cancel.store(true, Ordering::Release);
            job.source_cancel.cancel();
        }
        Ok(Status {
            request: params,
            editor,
        })
    }
    pub fn terminal_active(&self) -> bool {
        self.terminal.load(Ordering::Acquire)
    }
    pub fn stop(&self) {
        self.stop.store(true, Ordering::Release);
        self.cancellation.cancel();
        if let Ok(jobs) = self.jobs.lock() {
            for job in jobs.values() {
                job.source_cancel.cancel();
                job.cancel.store(true, Ordering::Release);
            }
        }
        if let Ok(mut plans) = self.plans.lock() {
            plans.clear();
        }
        let deadline = Instant::now() + Duration::from_secs(3);
        while self.active.load(Ordering::Acquire) > 0 && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(10));
        }
    }
}
impl Drop for Manager {
    fn drop(&mut self) {
        self.stop();
    }
}

type Outcome = (State, Option<i32>, Option<String>);
fn run(
    plan: &Plan,
    job: &Job,
    stop: &AtomicBool,
    cancellation: &LocalReviewCancellation,
) -> Result<Outcome, String> {
    if cancellation.is_cancelled() {
        job.source_cancel.cancel();
    }
    let control = control(job.source_cancel.clone());
    if file(&plan.params, &control)? != plan.proof
        || integration::select(&plan.params, &plan.proof.canonical, &control)?.as_ref()
            != Some(&plan.integration)
    {
        return Err("The selected file or editor changed before launch. Refresh it.".into());
    }
    if stop.load(Ordering::Acquire) || job.cancel.load(Ordering::Acquire) {
        return Ok((State::Canceled, None, None));
    }
    {
        let _claim = job.claim.lock().map_err(|_| invalid())?;
        if stop.load(Ordering::Acquire) || job.cancel.load(Ordering::Acquire) {
            return Ok((State::Canceled, None, None));
        }
        job.committed.store(true, Ordering::Release);
    }
    #[cfg(windows)]
    if plan.integration == Integration::System {
        integration::system_open(&plan.proof.canonical)?;
        return Ok((State::Launched, None, None));
    }
    #[cfg(not(windows))]
    let Integration::Command(invocation) = &plan.integration;
    #[cfg(windows)]
    let invocation = match &plan.integration {
        Integration::Command(value) => value,
        #[cfg(windows)]
        Integration::System => unreachable!(),
    };
    let terminal = if plan.params.mode == Mode::Terminal {
        Some(terminal::Terminal::open()?)
    } else {
        None
    };
    let mut command = if let Some(console) = &terminal {
        let payload = serde_json::to_string(&terminal::ChildParams {
            invocation: invocation.clone(),
            parent: std::process::id(),
            previous_group: console.previous_group(),
        })
        .map_err(|_| invalid())?;
        let mut command = Command::new(std::env::current_exe().map_err(|_| invalid())?);
        command.args(["--terminal-child", &payload]);
        console.configure(&mut command)?;
        command
    } else {
        let mut command = Command::new(&invocation.program);
        command
            .args(&invocation.args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            command.process_group(0);
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(windows_sys::Win32::System::Threading::CREATE_NO_WINDOW);
        }
        command
    };
    let mut child = command
        .spawn()
        .map_err(|_| "The selected editor could not start.".to_owned())?;
    let tree = if terminal.is_some() {
        Some(
            crate::local_review::GitProcessTree::attach_and_resume(&child).map_err(|_| {
                let _ = child.kill();
                let _ = child.wait();
                "The editor process could not be contained.".to_owned()
            })?,
        )
    } else {
        None
    };
    if let Ok(mut editor) = job.editor.lock() {
        editor.state = if terminal.is_some() {
            State::Running
        } else {
            State::Launched
        };
    }
    let deadline = Instant::now() + Duration::from_secs(3600);
    let outcome = loop {
        if stop.load(Ordering::Acquire)
            || job.cancel.load(Ordering::Acquire)
            || Instant::now() >= deadline
        {
            if let Some(tree) = &tree {
                crate::local_review::terminate_git_child(&mut child, tree);
                break (
                    State::Canceled,
                    None,
                    Some("The terminal editor was stopped.".into()),
                );
            }
            // GUI applications intentionally outlive the workspace. Only their
            // bounded monitor ends; shutdown must not close a user's app.
            break (State::Launched, None, None);
        }
        match child.try_wait() {
            Ok(Some(status)) => {
                if terminal.is_some() && matches!(status.code(), Some(130) | Some(-1073741510)) {
                    break (
                        State::Canceled,
                        status.code(),
                        Some("The terminal editor was interrupted.".into()),
                    );
                }
                break if status.success() {
                    (State::Exited, status.code(), None)
                } else {
                    (
                        State::Failed,
                        status.code(),
                        Some("The editor launcher exited unsuccessfully.".into()),
                    )
                };
            }
            Ok(None) => thread::sleep(Duration::from_millis(20)),
            Err(_) => {
                break (
                    State::Failed,
                    None,
                    Some("The editor process became unavailable.".into()),
                )
            }
        }
    };
    if let Some(tree) = &tree {
        crate::local_review::terminate_git_child(&mut child, tree);
    }
    #[cfg(windows)]
    drop(tree);
    // Foreground and console restoration completes before publishing a terminal
    // outcome; otherwise the UI could resume while the editor still owns input.
    drop(terminal);
    Ok(outcome)
}
