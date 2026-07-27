# Current Focus

This file is the live snapshot of any in-flight session. It is short on
purpose; the durable record lives in git, `_agent/WORKLOG.md`, and
`plan/done/`. Queued work lives in `plan/todo/`.

If status files and git disagree, git is authoritative; correct this file.

## Active state

- **Branch:** `agent/review-event-contract`
- **Active item:** GitHub issue #90 and `.docflow/plan/todo/0004-shared-review-service-program.md`
- **Blockers:** none; use the available SSH key for signed commits.
- **Uncommitted work:** provider-neutral Rust pull-request review event contract.

## Last shipped

`f7119f1` - accept the shared-service trust boundary through PR #117.

## Next item

Implement GitHub issue #91 after the event-contract PR merges.
