# Current Focus

This file is the live snapshot of any in-flight session. It is short on
purpose; the durable record lives in git, `_agent/WORKLOG.md`, and
`plan/done/`. Queued work lives in `plan/todo/`.

If status files and git disagree, git is authoritative; correct this file.

## Active state

- **Branch:** `agent/generic-oidc-authentication`
- **Active item:** GitHub issue #103 and `.docflow/plan/todo/0004-shared-review-service-program.md`
- **Blockers:** none; use the available SSH key for signed commits.
- **Uncommitted work:** generic OIDC authentication is implemented, fully validated, and ready for its signed integration commit.

## Last shipped

`96e9d7d` - ship team identity and role authorization through PR #133.

## Next item

Merge GitHub issue #103, then continue the shared review service program.
