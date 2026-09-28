# Current Focus

This file is the live snapshot of any in-flight session. It is short on
purpose; the durable record lives in git, `_agent/WORKLOG.md`, and
`plan/done/`. Queued work lives in `plan/todo/`.

If status files and git disagree, git is authoritative; correct this file.

## Active state

- **Branch:** `docs/record-opencode-provider-shipment` (preparing the follow-up
  docs record; not yet committed).
- **Shipped:** the OpenCode DeepSeek review provider (ADR 0018) shipped through
  [PR #264](https://github.com/delaudio/norn/pull/264) as release `v0.3.1`.
  The release workflow succeeded: both macOS architecture builds, release
  publication, Homebrew formula smoke on both architectures, stable
  finalization, and Homebrew tap publication. Desktop signing/DMG jobs were
  intentionally skipped because the desktop release variable is absent.
- **In flight:** this branch moves
  `.docflow/plan/todo/0015-opencode-ai-review-provider.md` to
  `plan/done/2026-09-28-opencode-ai-review-provider.md`, advances ADR 0018 from
  `Accepted` to `Implemented`, regenerates `.docflow/INDEX.md`, and appends the
  shipment to `.docflow/_agent/WORKLOG.md`.
- **Plan items:** `.docflow/plan/todo/0014-local-review-target-desktop.md`
  remains queued alongside `0001` and `0004`.
- **Verification:** the shipping change passed CI `verify` (8m42s) plus the
  local gate: typecheck, lint, build, all-feature Clippy, formatting, the full
  Rust suite (665 tests, 2 ignored), Archgate 17/17, and a pre-push Norn review.

## Residual

- Low: the desktop OpenCode model `<select>` exposes the two DeepSeek presets
  only (mirroring the Codex field); a custom `provider/model` set through the
  TUI is not shown as a selected option. Candidate follow-up.

## Next item

- Open and merge the docs-record PR for this branch.
- Implement the desktop Local review target without changing the shipped TUI
  snapshot contract.
