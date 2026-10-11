# Durable approved-publication journal

## Owning ADRs

- `../../adr/0020-opentui-rust-stdio-workspace.md` (AC 3, 5, 8, 9).

## Scope

Implement the engine's durable publication state machine before exposing remote
writes in the terminal. Journal the exact approved payload and stable scoped
local draft identity before POST; preserve uncertain outcomes across restarts.
An existing in-flight/uncertain identity never triggers another POST, even when
its draft body changes. Reconciliation is read-only and requires an exact,
authenticated-author/body/marker/anchor match supplied by the provider adapter.
A missing match remains uncertain. Definite pre-write rejection can be retried
only with new explicit approval. Provider adapters, IPC/UI batch approval and
finding feedback follow in separate increments; no live provider publication.

## Exit Criteria

1. SQLite migration and typed journal/publisher enforce scoped identity, immutable
   payload digest, durable writing-before-send, fenced acknowledgments and no
   expiry-based blind retry. Supported modes/paths/lines/pins/identities/body limits
   validate before writes. New code does not print provider bodies or secrets.
2. Fake provider tests cover success, partial batches via independent entries,
   timeout after remote success, restart/reconcile, concurrent approval, stale
   revisions before/after send, rejected writes and storage failures. A replay or
   edited uncertain draft cannot create duplicates; absent matches do not retry.
3. Core/full relevant Rust tests, strict Clippy and root gates pass. No actual
   provider/keychain/agent access or Norn review. Integrate after #352 CI/merge.
4. #304 stays open for concrete provider adapters, typed IPC/native UI approval,
   remote threads, feedback and cleanup/reconciliation integration. No cutover.

## Dependencies

- #352 authoritative finding staging and canonical drafts.

## Status

- Claimed by: default-agent, 2026-10-11, `issue-304-publication-journal`.
- Blockers: implementation/gates, #352 merge, PR CI/merge.
- Stopped:
