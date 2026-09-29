# Current Focus

This file is the live snapshot of any in-flight session. It is short on
purpose; the durable record lives in git, `_agent/WORKLOG.md`, and
`plan/done/`. Queued work lives in `plan/todo/`.

If status files and git disagree, git is authoritative; correct this file.

## Active state

- **Branch:** `fix/opencode-isolated-tool-leak` (release prep pending commit).
- **Active item:** ship `v0.3.3`, fixing DeepSeek-through-OpenCode reviews.
- **Change:** OpenCode diff-only reviews no longer disable every tool. The
  read-only inspection tools stay registered so DeepSeek uses OpenCode's native
  tool-call channel; mutation, shell, subagent, and network tools stay disabled,
  `lsp` and reads outside the working directory are denied, a response whose
  line begins with the leaked DSML markup token is rejected, and an empty or
  leaked response is retried once. Repository-backed reviews keep the same
  read-only tool set with a bounded working directory. ADR 0018 AC4 and AC6
  updated (r4).
- **Version:** sources aligned at `0.3.3`.
- **Verification:** 672 Rust library tests pass with 2 ignored; typecheck,
  lint, frontend tests, build, formatting, all-feature Clippy, `version:verify`,
  and Archgate 17/17 pass. Three consecutive headless reviews through OpenCode
  and `deepseek/deepseek-flash` succeeded with no leaked markup, and a fourth
  review after the detector fix reported findings without provider failure.

## Residual

- Isolated isolation relies on OpenCode enforcing `permission.external_directory
  = "deny"` (plus the `permission["*"] = "deny"` default). Verified empirically
  against OpenCode 1.18.31 (external absolute reads are denied); an automated
  integration assertion would need to invoke the installed CLI.
- Low: desktop OpenCode model `<select>` still exposes only the two DeepSeek
  presets; a custom `provider/model` set via the TUI is not shown as selected.

## Next item

- Open and merge the PR, tag `v0.3.3`, and confirm the release workflow and the
  Homebrew tap update.
- Implement the desktop Local review target without changing the shipped TUI
  snapshot contract.
