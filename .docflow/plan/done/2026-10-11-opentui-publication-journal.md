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

## Delivery

Effective on merge to main of this publication-journal PR. Implementation
`0b60c44`, based on PR #352 merge `5f518b5` (UI/verify green; verify 13m38s).
SQLite schema 16 and the Rust publisher bind exact approved payloads to stable
scoped local draft identity, persist writing before provider access, fence
attempt acknowledgments and preserve unknown outcomes without expiry retries.
Read-only exact-match reconciliation cannot POST or remove comments. Verified
postflight mismatch and unavailable postflight evidence have distinct states;
both retain confirmed remote identity without claiming ordinary publication.

Root typecheck, 123 tests/tooling, lint and Archgate 17/17 pass. Corrected core
638 tests (2 ignored), no-GUI dependency guard, 48 backend subprocess tests and
strict all-feature/all-target Clippy pass. Nine journal tests initialize actual
production migrations in owned SQLite files and cover concurrency, durable
writing-before-send, restart, partial independent outcomes, malformed/corrupt
storage, original payload/body matching, stale revisions and storage failures.
Final focused tests/Clippy rerun exit 0. Initial full core attempt failed on
sandbox loopback restrictions and the old explicit migration expectation ending
at schema 15; updated it to 16 and reran the complete lane with authorized
loopback permissions. No UI changes; prior UI gates remain PR #352 evidence.

Pre-integration main-slot/contiguous ADRs/INDEX membership/active links pass;
historical ADR 0011 plan coverage remains, no global audit-clean claim. Concrete
provider adapters, single-use typed IPC/native batch approval, remote threads,
canonical cleanup and atomic feedback integration remain #304. No live provider
publication, real credentials/agent access or Norn review. ADR 0020 stays
Accepted until complete cutover; remaining migration issues stay open.
