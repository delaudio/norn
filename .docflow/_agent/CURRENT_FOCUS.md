# Current Focus

This file is the live snapshot of any in-flight session. It is short on
purpose; the durable record lives in git, `_agent/WORKLOG.md`, and
`plan/done/`. Queued work lives in `plan/todo/`.

If status files and git disagree, git is authoritative; correct this file.

## Active state

- **Branch:** `feat/tui-side-by-side-image-diff`
- **Active item:** side-by-side terminal image-diff capability.
- **Blockers:** none.
- **Branch-local work:** ADR, plan item, TUI renderer, and focused tests only.

## Last shipped

`fa13281` - ship progressive TUI loading and readable diff paths through PR #146.

## Next item

Accept the image-comparison decision, then implement and validate the queued item.
