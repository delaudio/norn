# Current Focus

This file is the live snapshot of any in-flight session. It is short on
purpose; the durable record lives in git, `_agent/WORKLOG.md`, and
`plan/done/`. Queued work lives in `plan/todo/`.

If status files and git disagree, git is authoritative; correct this file.

## Active state

- **Branch:** `codex/closeout-release-hardening`.
- **Active item:** Docflow closeout for merged PR #205 and release-readiness
  assessment.
- **Plan item:** issues #198-#204 moved to `plan/done`; ADR 0014 is Implemented.
- **Verification:** PR #205 shipped at `643ff6f` after typecheck, 104 frontend
  tests plus tooling suites, complete Rust gates, a real Tauri build, Archgate
  17/17, and Norn review. Formula and cask publication remain intentionally
  release-triggered; the external tap was not mutated during implementation.

## Last shipped

`643ff6f` - ship release, Homebrew lifecycle, durable installation, and sandbox
permission diagnostics through PR #205.

## Next item

- Determine whether the next governed release can be tagged now or needs a
  release-candidate dry run and secret/bootstrap preparation first.
