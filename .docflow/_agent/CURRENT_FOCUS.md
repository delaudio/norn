# Current Focus

This file is the live snapshot of any in-flight session. It is short on
purpose; the durable record lives in git, `_agent/WORKLOG.md`, and
`plan/done/`. Queued work lives in `plan/todo/`.

If status files and git disagree, git is authoritative; correct this file.

## Active state

- **Branch:** `agent/shared-service-boundary`
- **Active item:** GitHub issue #89 and `.docflow/adr/0008-shared-review-service-boundary.md`
- **Blockers:** none; use the available SSH key for signed commits.
- **Uncommitted work:** accepted shared-service trust boundary and program plan.

## Last shipped

`f0cc40b` - close the shipped headless review plan through PR #116.

## Next item

Implement GitHub issue #90 after the trust-boundary PR merges.
