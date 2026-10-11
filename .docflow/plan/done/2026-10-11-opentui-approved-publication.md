# Approved publication, remote threads and finding feedback

## Owning ADRs

- `../../adr/0020-opentui-rust-stdio-workspace.md` (AC 3–6, 8, 9).

## Scope

Complete #304 on canonical drafts and the durable publication journal. Rust
owns bounded provider comment reads/writes and exact reconciliation. Add typed
IPC batch preparation with an exact rendered preview, supported modes/parents,
current head/base, canonical record version and a single-use expiring approval.
Explicit publication consumes that approval; navigation, edits, preview and
paste never send provider comments. Represent per-item confirmed/rejected/
uncertain/not-attempted/stale outcomes; retain outstanding drafts and never
silently relocate unsupported anchors. Add remote thread viewing and explicit
reply staging through the canonical editor. Preserve atomic saved finding
publication/feedback updates and recovery after acknowledged remote results.

## Exit Criteria

1. GitHub/Bitbucket adapters send at most one POST per attempt, bound response
   bytes/pages/time and restrict pagination/redirect/auth destinations. General,
   file and old/new inline modes use documented payloads; replies preserve the
   actual parent thread and never fall back to a new top-level comment. Exact
   authenticated-author/body/marker/parent/anchor reconciliation rejects
   ambiguous or incomplete evidence. Unsupported actions are explicit.
2. Prepare is read-only, validates all canonical draft IDs/metadata and current
   remote revision/paths/anchors, previews exact rendered text and issues an
   expiring single-use approval bound to scope/CAS/pins/payloads. Apply rechecks
   these boundaries and exposes per-item outcomes through a responsive bounded
   worker/operation. Disconnect/cancellation stops remaining writes; any possibly
   sent request remains journaled for read-only reconciliation.
3. Confirmed unchanged drafts can be cleaned up only after durable remote
   acknowledgment and atomic associated finding publication/feedback updates.
   Concurrent edits and partial/uncertain/stale outcomes retain content. Repair
   after restart preserves other run/chat/finding data. Local staging/removal
   associations remain consistent with canonical drafts and feedback semantics.
4. Terminal controls cover preview/cancel/approve/status/reconcile, visible
   staged counts and thread selection/reply staging with focus, full bounded
   bodies and multiline paste. Native input/frames, protocol contracts and fake
   provider tests prove no publication before explicit action, partial batches,
   timeout-after-success/restart, stale anchors, metadata tampering and scope
   fencing. No actual provider/keychain/agent access or Norn review in tests.
5. Root/UI/core/stdio/Clippy and applicable visual/lifecycle gates pass. Integrate
   after #353 CI/merge; close #304 only after this complete behavior is merged.
   #305 final recovery and #306–#309 parity/hardening/packaging/cutover remain.

## Dependencies

- #353 durable journal; #352 finding staging; #351 draft editor; canonical data.

## Status

- Claimed by: default-agent, 2026-10-11, `issue-304-approved-publication`.
- Blockers: implementation/gates and PR CI/merge; #353 merged at `3385fe1`.
- Stopped:

## Completion (effective on merge to main)

Implementation commit `7b2ec318414eeb7b59ecc0cf5454855d0b9250e2`, based on
journal PR #353 merge `3385fe1`. Completes #304 with bounded provider adapters,
expiring single-use exact previews and sequential native approval, durable
per-item status/cancel/read-only reconciliation, root replies and atomic
finding publication/feedback before conditional canonical cleanup. Preserves
original identity/content and outstanding or uncertain drafts without blind
POST retries. Evidence: `docs/opentui-migration/approved-publication.md`.

Root typecheck/123 tests+tooling/lint/Archgate 12/12, UI typecheck/136 tests/830
assertions/89 native frames, full core 660+2 ignored, 49 stdio, five CLI/five
onboarding, no-GUI dependency guard and strict all-feature/all-target Clippy
pass. Native input and fake-provider tests cover explicit action, partial
batches, source/scope/CAS fencing, timeout/restart, cancellation, stale anchors,
legacy journal migration, unknown-field retention and feedback rollback.
React Doctor now reports only seven DOM-rule warnings for OpenTUI's typed
native `flexShrink`; no suppressions. Main slot/contiguous ADRs/INDEX metadata
and active references pass; historical ADR 0011 coverage remains unchanged.

Prepared for PR integration, not yet shipped before merge. ADR 0020 remains
Accepted until complete cutover. #305–#309 and the epic remain open. No live
provider publication, real credential/agent access, Norn review or user-data
removal. The unrelated `.norn.yaml` remains untouched.
