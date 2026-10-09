# Current Focus

This file is the live snapshot of any in-flight session. It is short on
purpose; the durable record lives in git, `_agent/WORKLOG.md`, and
`plan/done/`. Queued work lives in `plan/todo/`.

If status files and git disagree, git is authoritative; correct this file.

## Active state

- **Branch:** `issue-297-opentui-protocol` (to be created).
- **Active item:** epic #294 step 03/15, issue #297 - define and generate the
  versioned typed stdio protocol.
- **Change:** single contract source `protocol/norn-protocol.schema.json`
  (JSON Schema draft-07) with shared fixtures in `protocol/fixtures/`. Generated
  TypeScript types + schema copy in `src/protocol/generated/`
  (`pnpm run protocol:generate`) and a runtime ajv validator in
  `src/protocol/validate.ts`. Rust types and newline-delimited framing in
  `src-tauri/src/protocol.rs` (handshake, envelopes, error taxonomy,
  first-slice methods, operation events, `SequenceGuard`). `pnpm run
  protocol:check` fails CI on drift and is wired into the CI `verify` lane and
  the release gate.
- **Plan items:** `.docflow/plan/todo/0018-opentui-protocol.md` (this item);
  `plan/done/2026-10-09-opentui-rust-architecture.md` (#295).

## Next item

- After #297 merges, proceed to #298 (persistent Rust stdio backend and
  operation lifecycle), blocked by #296 and #297 (both done).

## Epic queue

#295 -> #296/#297 -> #298 -> #299 -> #300 -> #301 -> #302/#303/#305 ->
#304 -> #306 -> #307 -> #308 -> #309 -> close #294.
