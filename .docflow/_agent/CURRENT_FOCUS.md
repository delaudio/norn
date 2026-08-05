# Current Focus

This file is the live snapshot of any in-flight session. It is short on
purpose; the durable record lives in git, `_agent/WORKLOG.md`, and
`plan/done/`. Queued work lives in `plan/todo/`.

If status files and git disagree, git is authoritative; correct this file.

## Active state

- **Branch:** `codex/norn-onboarding-foundation`
- **Active item:** GitHub issue #188 (onboarding follow-up and operator ergonomics).
  No local plan item is yet queued for this number.
- **Completed predecessor:** plan items `0006`, `0007`, and `0008` for issues
  #174, #176, and #175 are shipped via PR #196 (`36353fb`), and recorded in
  `plan/done` with the same commit.
- **Additional completed predecessor:** issue #186 is now closed in
  `.docflow/plan/done/2026-08-05-norn-onboarding-contract.md`.
- **Blockers:** no code blockers for the contract definition itself. Runtime
  compatibility behavior is covered by existing migration artifacts.
- **Branch-local work:** define machine-owned setup, repository-owned init, and
  read-only readiness surfaces.

## Last shipped

`e149569` - compare image diffs side by side in the terminal UI through PR #148.

## Next item

Issue #187 (`norn doctor`) is shipped. Next: implement issue #188 and #189 in order.
