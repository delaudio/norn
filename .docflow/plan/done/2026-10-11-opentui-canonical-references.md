# Canonical reference editor and review-context consumption

## Owning ADRs

- `../../adr/0020-opentui-rust-stdio-workspace.md` (AC 5, 9).

## Scope

Expose imported per-provider/per-repository/per-PR canonical references in a
terminal list/form. Support explicit add/edit/remove with existing per-record
CAS, preserve original identity/timestamps and metadata, retain edits on conflict
and discard on cancellation. Use Rust-owned records to include bounded saved
reference context in new inline PR reviews and fresh-session replies, with
provider fencing and unchanged resumed conversation semantics. Working-tree
reviews have no invented PR reference scope. No implicit URL fetch or local file
access; enrichment/provider fetch parity remains a later controlled step.

## Exit Criteria

1. All imported reference kinds are visible/editable; creating/removing/saving
   requires explicit input and never publishes or accesses a provider/credential.
   Wrong scope, malformed responses, stale saves and late results fail safely.
2. Rust review payloads include only the selected provider/PR records and preserve
   text, with bounded reference context and safe Markdown fences. Oversize context
   fails clearly before agent launch rather than silently dropping saved content.
3. Native input/frames and fake-agent real stdio tests prove editing/cancellation,
   restored references, conflicts, source/provider isolation and fresh review/reply
   payloads. Root typecheck/test/lint/Archgate, UI and Rust/core gates pass.
4. Integrate sequentially after PR #349; #305 remains open for draft consumption
   and recovery gates, #306 for reference enrichment and complete parity.

## Dependencies

- PR #344 canonical record read/mutation ownership, #349 imported-theme recovery.
- Independent preparation while #349 CI runs; integrate only after its merge.

## Status

- Claimed by: default-agent, 2026-10-11, `issue-305-canonical-references`.
- Blockers: local gate, PR #349 merge and this PR CI/merge.
- Stopped:

## Delivery

Effective on merge to main of this reference PR. Implementation `e9930a1`,
based on PR #349 merge `da0b87f` (all UI/verify/contract and three-platform
archive build/copy gates green). Terminal references use typed canonical
reads and per-record CAS; explicit form save/removal preserves imported
identity/source/creation time and other scopes. Native geometry follows wrapped
fields, frame listeners clean up, pasted shortcuts remain text, conflicts retain
forms and same-PR metadata refresh preserves unsaved edits.

Rust includes only selected provider/PR references in new inline review and
fresh-session reply payloads, with Unicode-preserving JSON and widened fences.
256 KiB context rejection occurs before fake-agent invocation; no truncation.
Successful Claude resumed reply retains its prior conversation and sends only
the new user message. The reference panel/backend do not fetch URLs, read
reference paths, access credentials or publish.

Root typecheck, 123 tests/tooling, lint and Archgate 17/17 pass. UI typecheck,
115 tests/691 assertions and 67 native frames pass. Rust core 627 tests
(2 intentional ignored), no-GUI guard, 46 real backend subprocess tests and
strict all-feature/all-target Clippy pass. React Doctor after rebasing to the
merged parent reports eight unknown-DOM-property warnings for native flexShrink
props (installed types/native tests verify them) plus the pre-existing large
Shell component warning; the array-lookup concern is corrected with Set lookup.
No diagnostic suppression or global-clean claim.

Six reference stories cover saved/compact/edit/conflict/remove/local states;
existing imported-theme frames gain named stories. All tests use synthetic
records/temp repositories/fake agents with isolated configuration and storage.
No Norn code review, actual provider/keychain access or live publication.

#305 remains open for staged-draft consumption (#304) and final native recovery
gates; #306 owns enrichment and complete parity. The migration decision stays
Accepted and old renderers remain in place until final cutover.
