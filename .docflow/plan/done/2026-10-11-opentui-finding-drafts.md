# Authoritative finding-to-draft staging

## Owning ADRs

- `../../adr/0020-opentui-rust-stdio-workspace.md` (AC 3–6, 9).

## Scope

Expose an explicit local finding-to-draft operation over typed stdio. Rust loads
the pinned persisted run, verifies provider/repository/PR/run/finding identity,
builds the comment and original anchor/review metadata, and stages through the
canonical draft CAS. Present a mode choice and preview before staging. Duplicate
staging preserves an existing edited draft; wrong/stale targets and unsupported
anchors fail rather than relocating. Counts derive from canonical associations.
Publication remains a separate approved operation with remote snapshot checks;
this increment does not send provider comments or invent remote acknowledgments.

## Exit Criteria

1. Typed protocol/schema/generated/runtime contracts cover local staging. The
   client supplies finding identity and explicit mode, never authoritative
   body/fingerprint/revisions. Backend rejects wrong scope/run/finding, malformed
   metadata, unavailable inline/file positions, unsafe input and stale draft CAS.
2. Unicode body and persisted finding metadata survive restart; existing staged
   edits are not replaced by repeated staging. Explicit stage input is isolated
   from navigation, agent launch and publication. Manual/imported drafts remain.
3. Core/fake-agent real stdio, native input and frames cover preview/cancel,
   modes, staging/counts, duplicate/conflict/late state and non-publishable anchors.
   Root gates, UI and strict relevant Rust gates pass; no live provider/keychain
   access or Norn review.
4. Integrate after #351. #304 remains open for provider publication, threads,
   partial failure, feedback and reconciliation. Migration cutover remains gated.

## Dependencies

- #351 local persistent draft editor and prior canonical records/run identity.

## Delivery

Effective on merge to main of this finding-staging PR. Implementation `c6eac6c`,
based on PR #351 merge `fc9837e` (UI/verify green). Rust prepares original saved
finding bodies, anchors and revisions with explicit mode selection; stage binds
the approved preview hash and canonical record version. Existing edited drafts
retain their body, mode, identity and creation time. Native input prevents paste
from invoking actions, retains a same-pinned-target preview and fences other
scopes and late results. Original CRLF persists with normalized display only.

Root typecheck, 123 tests/tooling, lint and Archgate 17/17 pass; UI 129 tests/800
assertions and 81 native frames; core 629 tests (2 ignored), no-GUI guard; full
48 backend subprocess tests and strict all-feature/all-target Clippy pass. A
shared fixture-root regression initially broke an existing reference-context
test; restoring that fixture and isolating the new restart fixture yields the
full passing lane. Final native test/frames and Clippy reruns exit 0. Doctor's
17 native flexShrink DOM-rule warnings are verified against installed types
and native tests, unsuppressed. Pre-integration contiguous ADR/INDEX membership,
active relative links and main plan slot pass; historical ADR 0011 coverage gap
remains, with no global audit-clean claim.

No provider publication, real agent/keychain access or Norn review. #304 remains
open for publication, threads, partial results, feedback and reconciliation;
#305 final recovery remains. ADR 0020 stays Accepted until complete cutover.
