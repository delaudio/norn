# Current Focus

This file is the live snapshot of any in-flight session. It is short on
purpose; the durable record lives in git, `_agent/WORKLOG.md`, and
`plan/done/`. Queued work lives in `plan/todo/`.

If status files and git disagree, git is authoritative; correct this file.

## Active state

- **Branch:** `main` (current tree).
- **Last shipped:** `v0.3.2` through
  [PR #272](https://github.com/delaudio/norn/pull/272). The readiness
  legacy-name scan no longer walks generated, gitignored `storybook-static/`
  and `.astro/` output, which had tripped `repository.legacyNameNotAllowed` and
  blocked the terminal UI preflight in the Norn source tree. The Storybook
  metadata now uses Norn naming. The release workflow published both macOS
  architecture archives, the formula smoke passed on both architectures, and
  the Homebrew tap advanced automatically.
- **Preceding release:** `v0.3.1` shipped the OpenCode DeepSeek review provider
  (ADR 0018, Implemented) through [PR #264](https://github.com/delaudio/norn/pull/264).
- **Queued:** `.docflow/plan/todo/0014-local-review-target-desktop.md`
  alongside `0001` and `0004`.

## Residual

- Low: the desktop OpenCode model `<select>` exposes the two DeepSeek presets
  only (mirroring the Codex field); a custom `provider/model` set through the
  TUI is not shown as a selected option. Candidate follow-up.

## Next item

- Implement the desktop Local review target without changing the shipped TUI
  snapshot contract.
