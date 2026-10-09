# Current Focus

This file is the live snapshot of any in-flight session. It is short on
purpose; the durable record lives in git, `_agent/WORKLOG.md`, and
`plan/done/`. Queued work lives in `plan/todo/`.

If status files and git disagree, git is authoritative; correct this file.

## Active state

- **Branch:** `issue-295-opentui-architecture` (to be created).
- **Active item:** epic #294 step 01/15, issue #295 - inventory the migration
  scope and record the OpenTUI/Rust architecture. Documentation only; no
  production behavior change.
- **Deliverables:** ADR 0020 (Accepted; supersedes ADR 0006) and
  `docs/opentui-migration/migration-contract.md` (package boundaries, executable
  names, command/runtime contract, platform matrix, performance procedure, and
  the per-capability parity matrix with dispositions and owning child issues).
  Plan item `.docflow/plan/todo/0016-opentui-rust-architecture.md` queues the
  work.
- **Reconciliation:** #255 (off critical path), #253 (obsoleted by Tauri
  retirement; open PR #259 in this category), #284 (carry the stale-response
  guarantee into the OpenTUI client). #256/#251/#164 stay separate.

## Next item

- After #295 merges, proceed to #296 (extract a UI-independent Rust engine and
  isolate Tauri adapters), which is blocked by #295.

## Epic queue

#295 -> #296/#297 -> #298 -> #299 -> #300 -> #301 -> #302/#303/#305 ->
#304 -> #306 -> #307 -> #308 -> #309 -> close #294.
