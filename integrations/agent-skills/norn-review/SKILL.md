---
name: norn-review
description: Review code changes with the local Norn headless CLI after implementation work, before completing a coding task, and before every git push. Use for local working-tree, branch, or pull-request review, structured finding triage, and one bounded remediation pass. Do not trigger inside a Norn reviewer child process.
---

# Norn Review

Run Norn as an independent, read-only reviewer after the repository's normal
validation commands pass. Headless review skips repository analyzers by default
because this workflow has already run the task's validation gate.

Use `--ai-provider claude` or `--ai-provider codex` when the task or user
request requires one provider. The command will use the configured default when
omitted.

## Pre-Push Requirement

Before every `git push` that publishes code changes, run Norn on the exact
changes about to be pushed.

- If the changes are uncommitted, review `--scope working-tree` before commit
  and again after any remediation if the pushed branch will include additional
  committed changes.
- If the changes are already committed, review `--scope branch` before pushing.
- If Norn returns in-scope findings at or above the configured threshold, fix
  them before pushing unless the user explicitly instructs otherwise.
- If Norn fails for setup, provider, or runtime reasons, report the failure
  and do not push unless the user explicitly approves pushing without the
  Norn gate.

## Fast Path

When the user explicitly asks for a Norn review, launch Norn immediately.
Do not inspect package scripts, read `.norn` policy files, precompute the
diff, or rerun repository validation. Norn resolves the repository, base,
configuration, policy packs, and diff itself.

Resolve `norn` from the active command `PATH`. A managed Homebrew installation
ships the skill and executable at the same version; a source installation may
provide its own active command path.

## Consent And Host Permission

Treat these as two separate boundaries:

1. Diff-sharing consent authorizes Norn to send only the diff selected by the
   command, plus its review instructions, to the configured AI provider. It
   never includes credentials, unrelated files, or a broader repository scope.
2. Host permission authorizes the local `norn review` process to run outside
   the coding agent's sandbox so it can reach the selected provider CLI,
   provider configuration, OS credential store, and network.

An explicit user request to run Norn review is one-run consent for the exact
target they selected. Add `--allow-provider-diff` to that review command. For
automatic post-task or pre-push activation, do not infer one-run consent. Run
without that flag and rely on the user's persistent local choice; if Norn
returns `review.diffConsentRequired`, ask the user before retrying. The user can
persist or revoke the local choice with:

```bash
norn setup --allow-provider-diff --yes
norn setup --deny-provider-diff --yes
```

Always request host permission on the first Norn invocation. Do not try inside
the sandbox first and do not interpret diff-sharing consent as host approval.

### Codex

Use Codex's escalated or outside-sandbox shell execution for the first
`norn review` call. Explain that the command launches the configured local AI
provider and sends the selected diff. When Codex offers a reusable approval,
scope it to the `norn review` command prefix only. Do not request a general
shell prefix or edit Codex settings.

### Claude Code

Use Claude Code's Bash permission request for the exact `norn review` command
and request execution outside the Claude sandbox when sandboxing is enabled.
If the host offers a reusable allow rule, scope it only to `norn review`
commands. Do not edit Claude Code settings or broaden Bash permissions.

If the host denies permission, report that Norn did not run. Do not retry with
different tools, copy the diff into another command, or bypass the host policy.

## Guard

If `NORN_REVIEW_CHILD=1`, stop this workflow immediately. The current agent
is already the reviewer launched by Norn.

If `norn` is not executable from `PATH`, report setup failure instead of
substituting an ad hoc review.

## Select The Target

- Use `--pr <id>` when the user names a pull request.
- For an explicit standalone repository review with no target, use
  `--scope branch`; let Norn resolve the default base.
- After an implementation task, use `--scope working-tree` for the changes just
  made. Use `--scope branch` when that task's changes are already committed.
- Pass `--base <ref>` only when the user or repository instructions name it.
- When committed and uncommitted task changes coexist, review branch and
  working tree separately and deduplicate findings by fingerprint.

Do not include unrelated pre-existing user changes in remediation decisions.
Do not run exploratory `rg`, `ls`, `git diff`, or config reads merely to prepare
the Norn command.

## Execute

For a post-task working-tree review, invoke the executable selected by the
guard above:

```bash
norn review --repo-path . --scope working-tree --format json \
  --fail-on-findings
```

When this is an explicit user-requested review, append
`--allow-provider-diff` as the one-run consent described above. Automatic
post-task and pre-push review must not append it.

To force a provider in this explicit workflow:

```bash
norn review --repo-path . --scope working-tree \
  --format json --fail-on-findings --allow-provider-diff \
  --ai-provider codex
```

or

```bash
norn review --repo-path . --scope working-tree \
  --format json --fail-on-findings --allow-provider-diff \
  --ai-provider claude
```

If `norn` is unavailable from `PATH`, report setup failure.

For an explicit standalone branch review where validation has already run,
replace the scope with `branch`. The task agent owns validation before
completion; do not rerun it as preparation for Norn. If the user asked only
for review, do not run validation first.

Pass an explicit `--profile`, `--ai-provider`, model, effort, base, or PR only
when repository guidance or the user selects it.

Do not pass `--run-analyzers` in this post-task workflow. That option is for
explicit standalone review runs where validation has not already executed and
the user wants Norn to run configured analyzers. Analyzer commands are
trusted local commands and must use non-mutating check modes.

Interpret exit codes as follows:

- `0`: review completed without a configured failing condition.
- `1`: review completed and returned findings at or above the threshold.
- `2` or greater: setup, config, repository, analyzer, provider, or runtime
  failure; report the failure and do not treat it as a code finding. Retry only
  when the error itself identifies a transient failure; do not begin a new
  discovery phase.

For JSON failures, handle these codes directly:

- `review.diffConsentRequired`: ask for one-run consent or tell the user how to
  persist it; do not add the one-run flag without their answer.
- `review.sandboxRestricted`: request the host's outside-sandbox permission and
  retry the exact command once.
- `review.providerTimeout`: report the timeout. Retry once outside the sandbox
  only when the timed-out attempt was not already host-approved.

## Triage And Rerun

Read the structured findings. Fix findings that are high-confidence, in scope,
and supported by the diff or repository evidence. Do not change code merely to
satisfy speculative or duplicate findings.

After fixes, rerun affected tests and Norn once. Stop after that bounded
rerun and report any residual findings.

Never publish provider comments, commit, push, or broaden permissions as part
of this workflow unless the user explicitly requests that separate action.
