# Review Session Identity in the Terminals

## Owning ADRs

- `../../adr/0020-opentui-rust-stdio-workspace.md`

## Scope

Begin the AI review/agent migration (#303) with the operation-identity slice:
preserve the running review operation and its streamed log across target
changes, label the operation with the target it belongs to, and keep the
operation states (queued/running/succeeded/failed/cancelled) visible. This
change does not add reply/fix/history IPC yet; it makes the existing
`review.start`/`operation.status`/`operation.cancel` lifecycle honest across
navigation.

GitHub issue: #303 (epic #294 step 09/15).

Pending (tracked): typed `review.reply`/`review.fix`/`review.history` methods,
the conversation component, finding grouping/filtering, and cancellation
semantics beyond the current single active operation.

## Exit Criteria

- Switching targets does not discard a running operation; its logs keep
  streaming and its target label stays.
- The shell shows `review <state> · <target> · <last event>`.
- `pnpm run typecheck`, `pnpm run test`, `archgate check`, the `ui` job and
  `frames:check` pass.

## Dependencies

- `../../adr/0020-opentui-rust-stdio-workspace.md`
- `../../../docs/opentui-migration/migration-contract.md`
