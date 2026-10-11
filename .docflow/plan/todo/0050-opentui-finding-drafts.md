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

## Status

- Claimed by: default-agent, 2026-10-11, `issue-304-finding-drafts`.
- Blockers: implementation/local gates, #351 merge, PR CI/merge.
- Stopped:
