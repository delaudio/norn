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
