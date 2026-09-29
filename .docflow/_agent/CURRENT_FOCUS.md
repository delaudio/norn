# Current Focus

This file is the live snapshot of any in-flight session. It is short on
purpose; the durable record lives in git, `_agent/WORKLOG.md`, and
`plan/done/`. Queued work lives in `plan/todo/`.

If status files and git disagree, git is authoritative; correct this file.

## Active state

- **Branch:** `main` (current tree).
- **Last shipped:** `v0.3.3` through
  [PR #274](https://github.com/delaudio/norn/pull/274). Diff-only reviews no
  longer disable every OpenCode tool, so DeepSeek uses OpenCode's native
  tool-call channel instead of leaking raw markup into the review. Read-only
  tools stay registered; mutation, shell, subagent, and network tools stay
  disabled; `lsp` and reads outside the working directory are denied; a
  line-anchored leaked-markup response is rejected; and an empty or leaked
  response is retried once. The release workflow published both macOS
  architecture archives, the formula smoke passed on both architectures, and
  the Homebrew tap advanced automatically.
- **Preceding releases:** `v0.3.2` fixed the readiness legacy-name scan
  (PR #272); `v0.3.1` shipped the OpenCode DeepSeek provider (ADR 0018,
  Implemented, PR #264).
- **Queued:** `.docflow/plan/todo/0014-local-review-target-desktop.md`
  alongside `0001` and `0004`.

## Residual

- Isolated isolation relies on OpenCode enforcing
  `permission.external_directory = "deny"` (plus `permission["*"] = "deny"`).
  Verified empirically against OpenCode 1.18.31; an automated integration
  assertion would need to invoke the installed CLI.
- Low: desktop OpenCode model `<select>` exposes only the two DeepSeek presets;
  a custom `provider/model` set via the TUI is not shown as selected.

## Next item

- Implement the desktop Local review target without changing the shipped TUI
  snapshot contract.
