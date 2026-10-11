# Persistent staged comment editor

## Owning ADRs

- `../../adr/0020-opentui-rust-stdio-workspace.md` (AC 4, 5, 6, 9).

## Scope

Restore canonical imported drafts in a terminal panel. Support explicit manual
creation, body/anchor/mode/parent editing and confirmed removal through existing
Rust-owned per-record CAS. Preserve imported identity, timestamps, source,
finding references and reviewed revisions. Retain unsaved edits on conflict,
isolate provider/repository/PR scopes and fence late reads/writes. This bounded
increment performs only local mutations; authoritative finding staging and
approved provider publication/reconciliation follow separately under #304.

## Exit Criteria

1. Drafts are visible with full bounded body, anchor, publication mode and source
   metadata; save/removal requires explicit input. Imported general, inline,
   file, local-only and parent comments retain all untouched fields.
2. Unicode/paste/control/line validation, cancellation, duplicate actions,
   stale CAS, same-PR metadata refresh and scope changes are verified with
   synthetic records and native terminal input. No provider, agent or credential
   access occurs in this editor.
3. Native frames/stories and real stdio persistence tests pass alongside root
   typecheck/test/lint/Archgate, UI gates and relevant Rust/core checks.
4. Integrate sequentially after #350. #304/#305 remain open until subsequent
   publication and final recovery gates; no migration cutover in this increment.

## Dependencies

- Canonical records/import (#340/#344) and references (#350).

## Delivery

Effective on merge to main of this draft-editor PR. Implementation `c4e3fff`,
based on PR #350 merge `bf7c15c` (UI/verify green). Existing canonical record
contracts preserve imported identity, creation time, source, finding references,
reviewed revisions and parent IDs through explicit CAS saves and confirmed local
removal. Same-PR refresh keeps forms; other scopes fence transient/late state.
Empty path/body fields remain strings, numeric lines and UTF-8/control limits
are validated, and pasted shortcuts cannot trigger publication or agents.

Root typecheck, 123 tests/tooling, lint and Archgate 17/17 pass. UI typecheck,
123 tests/747 assertions and 73 native frames pass; after separating native
input/form/list/follow helpers, both native input tests and frame check pass
again. Rust 47 backend subprocess tests and strict all-feature/all-target Clippy
pass, including restart persistence, full finding metadata, stale/wrong-PR CAS,
provider isolation and removal persistence. Final rebased React Doctor delta
reports only eight unknown-DOM native flexShrink warnings, verified against
installed types and native input/frames; complexity/effect concerns corrected
without diagnostic suppression. No global-clean claim.

Pre-integration numbering/status/relative-link/main-slot audit passes after an
editorial correction of ADR 0018's old ADR 0015 filename; metadata/decision/status
are unchanged. No actual provider, agent, keychain access, live publication or
Norn code review. #304 remains open for authoritative finding staging, approved
publication and uncertain-result reconciliation; #305 for final recovery gates;
#306 for complete parity. Migration ADR 0020 remains Accepted until cutover.
