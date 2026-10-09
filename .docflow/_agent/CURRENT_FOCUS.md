# Current Focus

This file is the live snapshot of any in-flight session. It is short on
purpose; the durable record lives in git, `_agent/WORKLOG.md`, and
`plan/done/`. Queued work lives in `plan/todo/`.

If status files and git disagree, git is authoritative; correct this file.

## Active state

- **Branch:** `issue-298-opentui-backend` (to be created).
- **Active item:** epic #294 step 04/15, issue #298 - persistent Rust stdio
  backend and operation lifecycle.
- **Change:** `src-tauri/src/backend.rs` + the `norn-backend` binary: versioned
  handshake, typed dispatch (`repository.status`, `diff.file`,
  `review.start`, `operation.status`, `operation.cancel`, `shutdown`), a single
  serialized writer with a bounded queue, operations on their own threads,
  cancellation honoured after the last file, untracked-file diffs, provider
  target resolution, and graceful shutdown on `shutdown`/EOF. Real subprocess
  integration tests in `src-tauri/tests/backend_stdio.rs`.
- **Plan items:** `.docflow/plan/todo/0019-opentui-backend.md` (this item);
  `plan/done/2026-10-09-opentui-rust-architecture.md` (#295).

## Limitation

- `review.start` drives the operation lifecycle over the collected target
  snapshot; the AI/review engine is connected in #301/#303. It does not fabricate
  findings and fails when the target has no reviewable local repository.

## Next item

- After #298 merges, proceed to #299 (OpenTUI React shell and typed backend
  client), blocked by #297 and #298 (both done).

## Epic queue

#295 -> #296/#297 -> #298 -> #299 -> #300 -> #301 -> #302/#303/#305 ->
#304 -> #306 -> #307 -> #308 -> #309 -> close #294.
