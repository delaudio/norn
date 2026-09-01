# Current Focus

This file is the live snapshot of any in-flight session. It is short on
purpose; the durable record lives in git, `_agent/WORKLOG.md`, and
`plan/done/`. Queued work lives in `plan/todo/`.

If status files and git disagree, git is authoritative; correct this file.

## Active state

- **Branch:** `docs/ship-tui-settings-ux`.
- **Active item:** `.docflow/plan/done/2026-09-01-refine-tui-settings-experience.md`.
- **Plan items:** record the completed TUI settings layout and provider
  credential input refinement shipped through PR #224.
- **Verification:** typecheck, lint, 104 frontend tests plus tooling suites,
  production build, the command-distribution Rust lane (547 library tests, 2
  ignored, plus CLI and onboarding targets), Clippy, Archgate 17/17, focused
  normal/narrow settings rendering, masked paste, and Bitbucket back-step tests.

## Last shipped

`c654019` - refine the TUI settings workspace and guided provider credential
input through PR #224.

## Next item

- `.docflow/plan/todo/0001-agentic-code-policy-pack.md` is the lowest-numbered
  remaining queue item.
