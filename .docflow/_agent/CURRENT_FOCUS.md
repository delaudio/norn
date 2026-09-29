# Current Focus

This file is the live snapshot of any in-flight session. It is short on
purpose; the durable record lives in git, `_agent/WORKLOG.md`, and
`plan/done/`. Queued work lives in `plan/todo/`.

If status files and git disagree, git is authoritative; correct this file.

## Active state

- **Branch:** `main` (current tree).
- **Last shipped:** `v0.3.4` through
  [PR #276](https://github.com/delaudio/norn/pull/276). The terminal UI CLI
  readiness detector now maps every AI provider, so OpenCode no longer shows
  **Missing** when installed; both the PATH-only startup scan and the login-shell
  refresh resolve `opencode`.
- **Preceding releases:** `v0.3.3` fixed DeepSeek-through-OpenCode leaking
  tool-call markup (PR #274); `v0.3.2` fixed the readiness legacy-name scan
  (PR #272); `v0.3.1` shipped the OpenCode DeepSeek provider (ADR 0018,
  Implemented, PR #264).
- **Queued:** `.docflow/plan/todo/0014-local-review-target-desktop.md`
  alongside `0001` and `0004`.

## Residual

- Isolated OpenCode isolation relies on OpenCode enforcing
  `permission.external_directory = "deny"` (verified empirically against
  OpenCode 1.18.31).
- Low: desktop OpenCode model `<select>` exposes only the two DeepSeek presets;
  a custom `provider/model` set via the TUI is not shown as selected.

## Next item

- Implement the desktop Local review target without changing the shipped TUI
  snapshot contract.
