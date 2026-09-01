# Current Focus

This file is the live snapshot of any in-flight session. It is short on
purpose; the durable record lives in git, `_agent/WORKLOG.md`, and
`plan/done/`. Queued work lives in `plan/todo/`.

If status files and git disagree, git is authoritative; correct this file.

## Active state

- **Branch:** `main`.
- **Active item:** none; issues #216, #217, and #218 are shipped and closed.
- **Plan items:** none in progress.
- **Verification:** typecheck, lint, 104 frontend tests plus tooling suites,
  production build, 541 Rust tests (2 ignored), 4 Tauri IPC smoke tests, Clippy,
  Archgate 17/17, a real temporary-home skill lifecycle pass, and a clean Norn
  branch review.

## Last shipped

`9179adc` - ship terminal credential onboarding, managed Codex and Claude Code
skills, and complete TUI settings through PR #220.

## Next item

- `.docflow/plan/todo/0001-agentic-code-policy-pack.md` is the lowest-numbered
  remaining queue item.
