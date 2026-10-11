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

## Status

- Claimed by: default-agent, 2026-10-11, `issue-304-persistent-drafts`.
- Blockers: local verification, #350 merge, PR CI/merge.
- Stopped:
