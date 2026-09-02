# Current Focus

This file is the live snapshot of any in-flight session. It is short on
purpose; the durable record lives in git, `_agent/WORKLOG.md`, and
`plan/done/`. Queued work lives in `plan/todo/`.

If status files and git disagree, git is authoritative; correct this file.

## Active state

- **Branch:** `feat/tui-browser-diff-viewer`.
- **Active item:** complete and harden the authenticated browser diff viewer.
- **Plan items:** `.docflow/plan/todo/0011-browser-diff-viewer.md`.
- **Verification:** typecheck, lint, 104 frontend tests plus tooling suites,
  576 passing Rust library tests (2 ignored), command-distribution onboarding
  E2E, Clippy, and Archgate 17/17 pass after audit remediation.

## Last shipped

`3b78900` - release Norn v0.2.6 after the TUI settings refinement.

## Next item

- `.docflow/plan/todo/0001-agentic-code-policy-pack.md` is the lowest-numbered
  remaining queue item.
