# Persistent local comment editor evidence

Plan 0049 implements the local draft-consumption portion of #304/#305 under
ADR 0020 AC 4–6/9. It uses existing workspace.read/workspace.mutate contracts and
the canonical per-provider/per-repository/per-PR record CAS. No new storage
format or provider-write path is introduced.

Imported localId, createdAt, source, findingRef, reviewed revisions and nullable
metadata survive body-only edits. Manual new comments default to general; mode,
path, side lines and parent are explicit fields. Blank body/path remain strings,
and line/control/UTF-8 limits are checked before local mutations. Invalid saved
records, duplicates, wrong PR IDs and unknown fields prevent editing rather than
silently dropping metadata. Form cancellation and confirmed removal have distinct
inputs, pending mutations coalesce and late results cannot update another scope.

Scalar scope dependencies keep edits across same-PR metadata refresh. Native
completed-frame geometry follows wrapped fields, resizes and long bodies;
owned listeners clean up. Global shell paste cannot also submit an agent reply
while this panel (or the existing reference panel) owns the input.

Synthetic native input tests cover imported restores, pasted shortcuts, explicit
save/removal, same-PR refresh and a 2,000-character wrapped path before body
selection and resize. Six native frame stories cover saved wide/compact,
editing, conflict, removal and no-PR states. Real Rust stdio tests restart the
backend against temporary storage, preserve full finding metadata, reject stale
CAS/wrong-PR writes, isolate providers and persist confirmed removal.

Publication is incomplete in this increment. Authoritative finding staging,
publication preview/approval, provider semantics, partial failure and uncertain
result reconciliation remain #304 follow-ups. #305 and the overall migration
decision remain open/Accepted pending the final gates. No live provider,
credential-store access, real agent or Norn code review is used.
