# Current Focus

This file is the live snapshot of any in-flight session. It is short on
purpose; the durable record lives in git, `_agent/WORKLOG.md`, and
`plan/done/`. Queued work lives in `plan/todo/`.

If status files and git disagree, git is authoritative; correct this file.

## Active state

- **Branch:** `codex/norn-onboarding-foundation`
- **Active item:** GitHub issue #186 (`.docflow/plan/todo/0009-norn-onboarding-contract.md`)
  and ADR `0012-norn-onboarding-contract.md`.
- **Completed predecessor:** plan items `0006`, `0007`, and `0008` for issues
  #174, #176, and #175 are shipped via PR #196 (`36353fb`), and recorded in
  `plan/done` with the same commit.
- **Blockers:** no code blockers for the contract definition itself. Runtime
  compatibility behavior is covered by existing migration artifacts.
- **Branch-local work:** define machine-owned setup, repository-owned init, and
  read-only readiness surfaces.

## Last shipped

`e149569` - compare image diffs side by side in the terminal UI through PR #148.

## Next item

Finish issue #186, then implement issue #187 (read-only readiness probe), then
continue with #188 and #189 in order.
