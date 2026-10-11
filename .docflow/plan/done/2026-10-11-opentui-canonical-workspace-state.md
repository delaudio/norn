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
Rust prompt builder for start/reply, preserving desktop precedence: a nonblank
persisted repo user prompt overrides repository/default content; an empty override
falls back to the selected repository policy. Headless semantics remain unchanged.
Repository policy parse errors (when no user override is active) fail
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
4. Fake-agent tests prove selected-repository replace/extend policy and canonical
   user-prompt precedence reach start/reply, with no unrelated prompt or silent policy loss.
5. Root/UI/protocol/Rust/Archgate gates pass. Draft publication, reference-provider
   enrichment, physical recovery and broader settings/streaming/platform gates stay
   with their owning issues unless actually delivered and verified here.

## Dependencies

- Plans 0038/0040 merged; integrate after plan 0041 / PR #343.
- #305 and #303 are unblocked by merged #301. #304 remains publication owner.

## Status

- Claimed by: default-agent, 2026-10-11, `issue-305-canonical-workspace-state`.
- Blockers: None; PR #343 merged at `ccd133d` with both CI gates green.
- Stopped:

## Delivery

Effective on merge to main of the canonical-workspace PR. Implementation
commit: `7d492d3`. The pre-completion root gate passes: typecheck, 123 tests
plus tooling checks and Archgate 17/17 (exit 0). Local issue-specific gates:
623 core tests with two intentionally ignored, no-GUI dependency guard, 42 real
backend subprocess tests, strict all-feature/all-target Clippy, terminal tests
including six workspace cases and native keyboard/paste fencing, 48 native
frames and protocol regeneration checks. React Doctor reports no issues in
the changed UI files; its overall heuristic score is not a clean-baseline claim.

Canonical records preserve reference/draft fields for their subsequent owners.
Prompt/viewed UI edits are explicit and generation/version fenced. Shared
selected-repository prompt resolution reaches new review and fallback/fresh
reply payloads; successful Claude resume retains the existing conversation
and sends only the new message. Fake-agent tests verify policy precedence
and invalid-policy rejection without a live review. Imported theme application,
reference enrichment, staged publication and physical platform recovery remain
open with their owning issues. ADR 0020 remains Accepted; this delivery does
not claim complete migration or authorize deletion of desktop data.
