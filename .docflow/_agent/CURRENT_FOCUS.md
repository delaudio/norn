# Current Focus

This file is the live snapshot of any in-flight session. It is short on
purpose; the durable record lives in git, `_agent/WORKLOG.md`, and
`plan/done/`. Queued work lives in `plan/todo/`.

If status files and git disagree, git is authoritative; correct this file.

## Active state

- **Branch:** `fix/tui-opencode-cli-readiness` (release prep pending commit).
- **Active item:** ship `v0.3.4`, fixing the terminal UI CLI readiness display.
- **Change:** the TUI CLI detector (`user_cli_available_in_path` and
  `user_cli_available`) only mapped `claude` and `codex`, so `opencode` fell
  through to the unknown-provider branch and always showed **Missing** under
  "CLI readiness" even though OpenCode-backed reviews worked. The detector now
  maps every AI provider through a shared `cli_executable_names` helper, with a
  test covering the mapping and the first-entry ordering macOS detection relies
  on.
- **Version:** sources aligned at `0.3.4`.
- **Verification:** 674 Rust library tests pass with 2 ignored; typecheck,
  lint, frontend tests, build, formatting, all-feature Clippy,
  `version:verify`, and Archgate 17/17 pass. Pre-push Norn review through
  OpenCode/DeepSeek was clean.

## Residual

- Isolated OpenCode isolation relies on OpenCode enforcing
  `permission.external_directory = "deny"` (verified empirically against
  OpenCode 1.18.31).
- Low: desktop OpenCode model `<select>` exposes only the two DeepSeek presets;
  a custom `provider/model` set via the TUI is not shown as selected.

## Next item

- Open and merge the PR, tag `v0.3.4`, and confirm the release workflow and the
  Homebrew tap update.
- Implement the desktop Local review target without changing the shipped TUI
  snapshot contract.
