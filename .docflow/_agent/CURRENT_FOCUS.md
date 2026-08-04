# Current Focus

This file is the live snapshot of any in-flight session. It is short on
purpose; the durable record lives in git, `_agent/WORKLOG.md`, and
`plan/done/`. Queued work lives in `plan/todo/`.

If status files and git disagree, git is authoritative; correct this file.

## Active state

- **Branch:** `codex/norn-naming-contract`
- **Active item:** GitHub issue #173 and `.docflow/plan/todo/0005-norn-naming-and-compatibility.md`.
- **Blockers:** none for the decision record; external DNS, Homebrew, and
  signing operations remain separate delivery prerequisites.
- **Branch-local work:** accepted Norn naming and compatibility contract is
  ready for the documentation gate and integration.

## Last shipped

`e149569` - compare image diffs side by side in the terminal UI through PR #148.

## Next item

Merge the naming contract, then implement the runtime, repository-config, and
local-data migration slices in dependency order.
