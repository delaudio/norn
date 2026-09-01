# Current Focus

This file is the live snapshot of any in-flight session. It is short on
purpose; the durable record lives in git, `_agent/WORKLOG.md`, and
`plan/done/`. Queued work lives in `plan/todo/`.

If status files and git disagree, git is authoritative; correct this file.

## Active state

- **Branch:** `feat/terminal-onboarding-skills-settings`.
- **Active item:** define and implement terminal-first credential onboarding,
  managed Codex and Claude Code skill distribution, and complete TUI settings
  for issues #216, #217, and #218.
- **Plan item:** pending guided ADR approval and plan authoring.
- **Verification:** `v0.2.4` is the current stable command release. The working
  tree started clean from `main` at `e89cce4`.

## Last shipped

`e89cce4` - close the Homebrew Formula temporary-tap plan after PR #214.

## Next item

- Approve ADR 0015, create the three issue-backed plan items, and implement
  them in dependency order.
