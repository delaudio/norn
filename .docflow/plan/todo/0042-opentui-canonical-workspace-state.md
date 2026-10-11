# Consume canonical workspace records and repository review prompts

## Owning ADRs

- `../../adr/0020-opentui-rust-stdio-workspace.md` (AC 2, 3, 5, 6).
- `../../adr/0008-shared-review-service-boundary.md`.

## Scope

Expose validated canonical workspace records through typed, bounded, paginated
read snapshots and per-item/scalar mutations with record-version compare-and-swap.
Preserve imported draft/reference fields for their owning native workflows; do not
transfer a whole 16 MiB document in one protocol frame or overwrite other scopes.
Read pages are pinned to the captured record version and mutations compare with
the current on-disk version. Source/provider/PR keys and page versions must be
validated; all user-data writes use the existing atomic canonical destination.
Consume saved/imported user prompts and viewed-file flags in the terminal, with
explicit edit/save/discard and stale-target fencing. Migrated references/drafts
must be reachable through this shared owner for #305/#304 parity, never copied
back into UI-only persistence. No provider comment publication in this increment.

Resolve the selected repository's review policy through the existing shared
Rust prompt builder for start/reply, and add its persisted repo user prompt without
changing headless analyzer/policy semantics. Repository policy parse errors fail
clearly; no fallback to the backend launch directory or another target's prompt.
Use synthetic policy files and fake agents only; do not invoke Norn reviews or
read the maintainer's real .norn.yaml/credentials.

## Exit Criteria

1. Read-only pages remain below the 1 MiB protocol limit for the largest valid
   single draft/reference and large imported scopes, with bounded snapshot cache
   and explicit expired/stale-page outcomes.
2. Per-record versions fence writes without replacing unrelated scopes; validation,
   conflict, corruption and interrupted writes preserve previous bytes. Existing
   non-destructive import/export behavior is retained.
3. Native terminal reads persisted prompts/viewed state, supports explicit mutation
   and fences late reads/writes across target changes. Stories cover saved/dirty/
   failed/stale states and restored imported data.
4. Fake-agent tests prove selected-repository replace/extend policy plus canonical
   user prompt reaches start/reply, with no unrelated prompt or silent policy loss.
5. Root/UI/protocol/Rust/Archgate gates pass. Draft publication, reference-provider
   enrichment, physical recovery and broader settings/streaming/platform gates stay
   with their owning issues unless actually delivered and verified here.

## Dependencies

- Plans 0038/0040 merged; integrate after plan 0041 / PR #343.
- #305 and #303 are unblocked by merged #301. #304 remains publication owner.

## Status

- Claimed by: default-agent, 2026-10-11, `issue-305-canonical-workspace-state`.
- Blockers: PR #343 CI pending; isolated local preparation only.
- Stopped:
