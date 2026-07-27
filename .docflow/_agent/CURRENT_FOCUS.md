# Current Focus

This file is the live snapshot of any in-flight session. It is short on
purpose; the durable record lives in git, `_agent/WORKLOG.md`, and
`plan/done/`. Queued work lives in `plan/todo/`.

If status files and git disagree, git is authoritative; correct this file.

## Active state

- **Branch:** `agent/incremental-review-scope`
- **Active item:** GitHub issue #92 and `.docflow/plan/todo/0004-shared-review-service-program.md`
- **Blockers:** none; use the available SSH key for signed commits.
- **Uncommitted work:** read-only incremental review scope between explicit commit SHAs.

## Last shipped

`a28aca8` - persist last-reviewed-head cursors through PR #119.

## Next item

Implement GitHub issue #93 after the incremental-scope PR merges.
