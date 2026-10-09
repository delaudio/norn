# OpenTUI Vertical Slice: File -> Diff -> Finding

## Owning ADRs

- `../../adr/0020-opentui-rust-stdio-workspace.md`

## Scope

Deliver the first real end-to-end review slice over the stdio backend and
measure it. Add `review.files` and `review.findings` to the protocol (schema,
generated TypeScript, Rust `protocol.rs`, Rust `backend.rs`) and drive them from
the OpenTUI shell: changed-file list, lazy diff with stale-response fencing, and
finding navigation with anchor resolution and a missing-anchor notice. Cover the
slice with Rust backend integration tests, transport/store/shell tests and
deterministic frames. Add `ui/scripts/bench.ts` and record the method and
OpenTUI numbers in `docs/opentui-migration/vertical-slice.md`.

GitHub issue: #301 (epic #294 step 07/15).

Out of scope: publishing findings, the AI review operation UI, and an
AI-generated finding set.

## Exit Criteria

- The slice is exercised against the real Rust backend in
  `src-tauri/tests/backend_stdio.rs`.
- A late `diff.file` response never replaces a newer selection; a finding whose
  anchor does not resolve surfaces a notice.
- The OpenTUI benchmark command (`bun run --cwd ui bench`) is committed and
  repeatable.
- **Pending gate:** Ratatui-vs-OpenTUI numbers on the same host satisfy the
  migration-contract thresholds. Until that comparison exists this item stays
  open even though the functional slice is delivered.
- `pnpm run typecheck`, `pnpm run test`, `archgate check` and the `ui` job pass.

## Dependencies

- `../../adr/0020-opentui-rust-stdio-workspace.md`
- `../../../docs/opentui-migration/migration-contract.md`
- `../../../docs/opentui-migration/vertical-slice.md`
