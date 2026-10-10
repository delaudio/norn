# Review History

## Owning ADRs

- `../../adr/0020-opentui-rust-stdio-workspace.md`

## Scope

Add a typed `review.history` method returning the stored review runs for a
target (most recent first, bounded to 20) with status, turn kind and finding
count, reusing the Rust review store. Surface the run count on the findings
panel so prior reviews are visible.

GitHub issue: #303 (epic #294 step 09/15).

Pending (tracked): `review.reply`/`review.fix` IPC, the conversation component,
finding grouping and cancellation semantics.

## Exit Criteria

- `review.history` returns `{ runs: [{ runId, createdAt, status, turnKind, findingsCount }] }` for stored PR targets; empty otherwise.
- The findings panel shows the stored run count when present.
- `pnpm run lint/typecheck/test`, `archgate check`, the `ui` job, Rust tests and
  `frames:check` pass.

## Dependencies

- `../../adr/0020-opentui-rust-stdio-workspace.md`
- `../../../docs/opentui-migration/migration-contract.md`
