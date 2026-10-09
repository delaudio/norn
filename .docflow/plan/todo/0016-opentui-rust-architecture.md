# OpenTUI Rust Architecture and Migration Scope

## Owning ADRs

- `../../adr/0020-opentui-rust-stdio-workspace.md`

## Scope

Record the implementation contract for replacing the Ratatui terminal UI and the
Tauri desktop app with an OpenTUI + React + TypeScript workspace over the
existing Rust engine. Inventory every shipped terminal and desktop workflow into
a parity matrix with a disposition and owning child issue, record the target
package boundaries and executable names, define the command/runtime contract and
supported platform/toolchain matrix, define the performance measurement
procedure, reconcile the pending desktop issues, and queue the migration work.
This item changes documentation only; it does not alter production behavior.

Child GitHub issues: #296 (UI-independent engine), #297 (protocol), #298 (stdio
backend), #299 (OpenTUI shell/client), #300 (native-frame Storybook), #301
(vertical slice), #302 (diff/image/browser), #303 (review/conversation/fix),
#304 (drafts/publication), #305 (settings/onboarding/user data), #306 (parity
close-out), #307 (hardening), #308 (packaging), #309 (cutover removal).

Out of scope: implementing the UI, removing Tauri/Ratatui, choosing a new
provider architecture, or treating unshipped backlog as migration requirements.

## Exit Criteria

- ADR 0020 is Accepted and supersedes ADR 0006; the index reflects both.
- Every shipped terminal and desktop workflow has a disposition (`port`,
  `preserve`, `retire`, `migrate`, or `shared`) and an owning child issue in
  `../../../docs/opentui-migration/migration-contract.md`.
- Package boundaries, executable names, command/runtime contract, platform
  matrix, pinned versions, and the performance measurement procedure are
  recorded.
- Pending desktop issues #255, #253, and #284 are reconciled with an explicit
  disposition; #256, #251, and #164 remain separately scoped.
- The migration is queued as plan/issue work before code changes; no production
  behavior changes in this item.
- `pnpm run typecheck`, `pnpm run test`, and `archgate check` pass.

## Dependencies

- `../../adr/0020-opentui-rust-stdio-workspace.md`
- `../../../docs/opentui-migration/migration-contract.md`
