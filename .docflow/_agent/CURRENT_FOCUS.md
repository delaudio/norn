# Current Focus

This file is the live snapshot of any in-flight session. It is short on
purpose; the durable record lives in git, `_agent/WORKLOG.md`, and
`plan/done/`. Queued work lives in `plan/todo/`.

If status files and git disagree, git is authoritative; correct this file.

## Active state

- **Branch:** `feat/local-review-tui`.
- **Active item:** `.docflow/plan/todo/0013-local-review-target-tui.md`.
- **Plan items:** implement the shared local snapshot and TUI first, then
  `.docflow/plan/todo/0014-local-review-target-desktop.md`.
- **Verification:** 631 Rust tests pass with 2 ignored, alongside 110 frontend
  tests plus tooling, all-target Clippy, typecheck, lint, Archgate 17/17, and an
  isolated Windows target compile check. Local snapshots share one end-to-end
  deadline, superseded TUI loads cancel their process trees, Windows Git starts
  suspended before Job Object assignment, and Local-mode repository navigation
  is restricted to usable configured paths. Norn review run
  `run-1788509355794197000` confirmed the original findings are resolved and
  reported two residual items: interruptible preview hashing (high) and cached
  Local repository eligibility outside the render loop (low).

## Last shipped

`1e08045` - release Norn v0.2.9 with the shared desktop/browser diff UI.

## Next item

- Implement the shared local snapshot and terminal Local review target.
