# Current Focus

This file is the live snapshot of any in-flight session. It is short on
purpose; the durable record lives in git, `_agent/WORKLOG.md`, and
`plan/done/`. Queued work lives in `plan/todo/`.

If status files and git disagree, git is authoritative; correct this file.

## Active state

- **Branch:** `agent/headless-review-jobs`
- **Active item:** GitHub issue #96 and `.docflow/plan/todo/0004-shared-review-service-program.md`
- **Blockers:** none; use the available SSH key for signed commits.
- **Uncommitted work:** issue #96 coordination closeout through hardening commit `5f3f42d`; final branch review and PR integration remain.

## Last shipped

`2bf6483` - receive authenticated provider webhooks through PR #123.

## Next item

Implement GitHub issue #97 after the headless review job coordinator PR merges.
