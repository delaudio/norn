# Current Focus

This file is the live snapshot of any in-flight session. It is short on
purpose; the durable record lives in git, `_agent/WORKLOG.md`, and
`plan/done/`. Queued work lives in `plan/todo/`.

If status files and git disagree, git is authoritative; correct this file.

## Active state

- **Branch:** `agent/headless-review-cli`
- **Active item:** `.docflow/plan/todo/0003-headless-review-cli.md`
- **Blockers:** none; use the available SSH key for signed commits.
- **Uncommitted work:** validated ADR, analyzer policy across headless/TUI/GUI,
  sandbox-safe headless storage, and the Codex skill fast path.

## Last shipped

`b12fe0c` - document `lac` current-repository startup and improve terminal startup errors.

## Next item

Create the signed commit and PR for the validated headless review work, merge
after CI passes, then ship the plan item.
