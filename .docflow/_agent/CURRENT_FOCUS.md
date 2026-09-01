# Current Focus

This file is the live snapshot of any in-flight session. It is short on
purpose; the durable record lives in git, `_agent/WORKLOG.md`, and
`plan/done/`. Queued work lives in `plan/todo/`.

If status files and git disagree, git is authoritative; correct this file.

## Active state

- **Branch:** `docs/ship-agent-review-permissions`.
- **Active item:** close the completed agent review permission handoff after
  PR #227 merged to `main`.
- **Plan items:** move the shipped plan entry to `done`, record the completion
  event, and return the live snapshot to the remaining queue.
- **Verification:** PR #227 merged at `8975b92`; typecheck, 104 frontend tests
  plus tooling suites, and Archgate 17/17 pass on merged `main`. The focused
  Rust coverage, runtime consent/sandbox probes, Clippy, Tauri IPC tests, skill
  validation, and bounded Norn review also passed before integration.

## Last shipped

`3b78900` - release Norn v0.2.6 after the TUI settings refinement.

## Next item

- `.docflow/plan/todo/0001-agentic-code-policy-pack.md` is the lowest-numbered
  remaining queue item.
