# Current Focus

This file is the live snapshot of any in-flight session. It is short on
purpose; the durable record lives in git, `_agent/WORKLOG.md`, and
`plan/done/`. Queued work lives in `plan/todo/`.

If status files and git disagree, git is authoritative; correct this file.

## Active state

- **Branch:** `agent/review-cursor-storage`
- **Active item:** GitHub issue #91 and `.docflow/plan/todo/0004-shared-review-service-program.md`
- **Blockers:** none; use the available SSH key for signed commits.
- **Uncommitted work:** tenant-scoped last-reviewed-head cursor persistence.

## Last shipped

`93ee0a8` - add the provider-neutral PR event contract through PR #118.

## Next item

Implement GitHub issue #92 after the review-cursor PR merges.
