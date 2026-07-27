# Current Focus

This file is the live snapshot of any in-flight session. It is short on
purpose; the durable record lives in git, `_agent/WORKLOG.md`, and
`plan/done/`. Queued work lives in `plan/todo/`.

If status files and git disagree, git is authoritative; correct this file.

## Active state

- **Branch:** `agent/provider-webhook-ingress`
- **Active item:** GitHub issue #95 and `.docflow/plan/todo/0004-shared-review-service-program.md`
- **Blockers:** none; use the available SSH key for signed commits.
- **Uncommitted work:** coordination log only; implementation commits through `fbbd332` await final review and push.

## Last shipped

`d2f0fae` - store administrative audit events through PR #122.

## Next item

Implement GitHub issue #96 after the provider webhook ingress PR merges.
