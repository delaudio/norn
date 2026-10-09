# Define the Typed Stdio Protocol

## Owning ADRs

- `../../adr/0020-opentui-rust-stdio-workspace.md`

## Scope

Define one machine-checked contract shared by Rust and TypeScript for local
requests, responses, operation events, and state recovery. The single source is
`protocol/norn-protocol.schema.json`; the TypeScript types and the copied schema
are generated into `src/protocol/generated/`, and `src/protocol/validate.ts`
validates untrusted input at runtime with ajv. Rust defines the types and
newline-delimited framing in `src-tauri/src/protocol.rs`. Shared fixtures in
`protocol/fixtures/` are validated by both sides.

GitHub issue: #297 (epic #294 step 03/15).

Out of scope: business logic in the schema, a network listener, or copying every
legacy Tauri command into the protocol.

## Exit Criteria

- Rust and TypeScript validate the same request/response/event fixtures.
- Envelope shape, versioned handshake, error taxonomy, first-slice methods,
  operation events, cancellation, and shutdown are specified with examples.
- Framing limits, version mismatch, malformed/oversized/empty frames,
  duplicate/out-of-order event sequences, and mutation-retry rules are covered
  by tests.
- Regeneration is deterministic and `pnpm run protocol:check` (wired into CI and
  the release gate) detects contract drift.
- `pnpm run typecheck`, `pnpm run test`, and `archgate check` pass.

## Dependencies

- `../../adr/0020-opentui-rust-stdio-workspace.md`
- `../../../docs/opentui-migration/migration-contract.md`
