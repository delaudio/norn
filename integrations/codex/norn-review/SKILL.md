---
name: norn-review
description: Review code changes with the local Norn headless CLI after implementation work, before completing a coding task, and before every git push. Use for local working-tree, branch, or pull-request review, structured finding triage, and one bounded remediation pass. Do not trigger inside a Norn reviewer child process.
---

# Norn Review

Run Norn as an independent, read-only reviewer after the repository's normal
validation commands pass. Headless review skips repository analyzers by default
because this workflow has already run the task's validation gate.

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

Use `$HOME/.local/bin/norn` directly when it is executable; otherwise use
`norn` from `PATH`. During the compatibility window only, fall back to
`$HOME/.local/bin/lachesi` and then `lachesi` when the canonical executable is
unavailable.

## Guard

If `NORN_REVIEW_CHILD=1` or `LACHESI_REVIEW_CHILD=1`, stop this workflow immediately. The current agent
is already the reviewer launched by Norn.

If neither the canonical Norn executable nor its documented legacy alias is
executable, report that setup failure instead of substituting an ad hoc review.

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
guard above. Prefer:

```bash
"$HOME/.local/bin/norn" review --repo-path . --scope working-tree --format json \
  --fail-on-findings
```

If `$HOME/.local/bin/norn` is not executable, use the same arguments with
`norn` from `PATH`, then use the legacy alias only as the final compatibility
fallback.

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

## Triage And Rerun

Read the structured findings. Fix findings that are high-confidence, in scope,
and supported by the diff or repository evidence. Do not change code merely to
satisfy speculative or duplicate findings.

After fixes, rerun affected tests and Norn once. Stop after that bounded
rerun and report any residual findings.

Never publish provider comments, commit, push, or broaden permissions as part
of this workflow unless the user explicitly requests that separate action.
