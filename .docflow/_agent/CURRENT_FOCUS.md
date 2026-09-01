# Current Focus

This file is the live snapshot of any in-flight session. It is short on
purpose; the durable record lives in git, `_agent/WORKLOG.md`, and
`plan/done/`. Queued work lives in `plan/todo/`.

If status files and git disagree, git is authoritative; correct this file.

## Active state

- **Branch:** `feat/terminal-onboarding-skills-settings-impl`.
- **Active item:** validate and integrate terminal-first credential onboarding,
  managed Codex and Claude Code skill distribution, and complete TUI settings
  for issues #216, #217, and #218.
- **Plan items:** `.docflow/plan/todo/0002-secure-terminal-credential-onboarding.md`,
  `.docflow/plan/todo/0003-managed-agent-review-skills.md`, and
  `.docflow/plan/todo/0005-complete-tui-settings.md`.
- **Verification:** typecheck, lint, 104 frontend tests plus tooling suites,
  production build, 541 Rust tests (2 ignored), 4 Tauri IPC smoke tests, Clippy,
  Archgate 17/17, and a real temporary-home skill lifecycle pass.

## Last shipped

`e89cce4` - close the Homebrew Formula temporary-tap plan after PR #214.

## Next item

- Run the bounded Norn review, commit and push the implementation, then merge
  its pull request and ship plan items 0002, 0003, and 0005.
