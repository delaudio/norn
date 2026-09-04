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
- **Verification:** 626 Rust tests pass with 2 ignored, alongside 110 frontend
  tests plus tooling, all-target Clippy, typecheck, lint, and Archgate 17/17.
  Git subprocess output and duration are bounded, local snapshot history keeps
  the newest 20 entries per repository transactionally, and preview
  fingerprints are required before file access; the final pre-push Norn review
  is pending.

## Last shipped

`1e08045` - release Norn v0.2.9 with the shared desktop/browser diff UI.

## Next item

- Implement the shared local snapshot and terminal Local review target.
