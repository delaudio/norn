# Current Focus

This file is the live snapshot of any in-flight session. It is short on
purpose; the durable record lives in git, `_agent/WORKLOG.md`, and
`plan/done/`. Queued work lives in `plan/todo/`.

If status files and git disagree, git is authoritative; correct this file.

## Active state

- **Branch:** `feat/tui-side-by-side-image-diff`
- **Active item:** side-by-side terminal image-diff capability.
- **Blockers:** Lachesi and GitHub CLI authentication are required before the
  mandatory review gate can publish this branch.
- **Branch-local work:** accepted ADR, queued plan item, TUI renderer, and
  focused tests are committed locally.

## Last shipped

`fa13281` - ship progressive TUI loading and readable diff paths through PR #146.

## Next item

Authenticate the review/publishing clients, then run the pre-push review and
open a pull request for the accepted image-comparison plan item.
