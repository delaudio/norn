# Current Focus

This file is the live snapshot of any in-flight session. It is short on
purpose; the durable record lives in git, `_agent/WORKLOG.md`, and
`plan/done/`. Queued work lives in `plan/todo/`.

If status files and git disagree, git is authoritative; correct this file.

## Active state

- **Branch:** `agent/retention-deletion`
- **Active item:** GitHub issue #109 and `.docflow/plan/todo/0004-shared-review-service-program.md`
- **Blockers:** none; use the available SSH key for signed commits.
- **Uncommitted work:** durable retry and dead-letter implementation pending signed commit.

## Last shipped

`136e87c` - ship encrypted credential broker through PR #139.

## Next item

Merge GitHub issue #109, then continue the shared review service program.
