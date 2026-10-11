# Authoritative local finding staging

Plan 0050 implements finding-to-draft conversion for #304 under ADR 0020 AC 3–6/9.
The new draft.prepare/draft.stage protocol methods are additive, generated from
the schema and advertised through draft-staging. Their boundary requires a
provider/PR target pinned to a run, finding ID and explicit mode. They never take
UI-provided authoritative body/fingerprint/revisions.

Rust loads the selected run and checks provider/workspace/repository/PR/run and
unambiguous finding identity. Dismissed/fixed/stale findings and saved publication
acknowledgments are not staged again. Supported anchors retain original side,
line, path and full reviewed revisions; invalid positions fail without fallback.
General/local-only requires a separate choice. Colon characters within ordinary
relative path components remain valid; absolute/drive/backslash/traversal/control
positions are refused. Publication eligibility against current remote state is
deliberately not inferred by local staging.

Preparation returns the actual draft, canonical record version and an opaque
preview hash. Staging recomputes that content and uses the original record CAS.
The content hash excludes only creation time, which new drafts obtain when
actually saved; existing creation time is still covered by full-record CAS.
Every body/anchor/mode/finding/revision field is bound. A repeated stage returns
an existing linked draft with its edits, original mode and identity; conflicting
duplicate associations/IDs fail rather than replacing or adding another draft.

The client validates exact target/key/hash/body/ref and acknowledgment invariants.
Transient selection, mode, preview, busy and completion state have generation
fences, duplicate input coalescing and explicit preview/stage/cancel controls.
Native paste cannot stage, publish or launch an agent. Same-target metadata
refresh preserves the preview; actual scope/run selection clears it. Scoped
staged counts/list contents derive from canonical saved drafts. Shared draft
validation accepts canonical CRLF while native display normalizes line endings
only for presentation; saved bodies remain unchanged.

Core tests cover mode construction, reviewed identity, content hashes, malformed
positions and explicit general/local-only choice. Real backend stdio tests prove
read-only preparation, preview/content/CAS rejection, restart persistence,
duplicate edited-draft preservation, wrong provider/missing run, non-publishable
anchors and stored publication acknowledgment guards. The fake agent must remain
unexecuted. Native input covers preview/paste/cancel, same-run refresh, explicit
stage and opening a single persisted draft; controller tests cover late results,
malformed DTOs, contradictory acknowledgments and retained errors.

Eight native stories cover choosing, wide/compact preview, general, unsupported
anchor, successful stage, existing edits and conflict. This increment performs
no provider/credential access, live publication or Norn review. Remote threads,
approved batch publication, partial failure, feedback and uncertain-result
reconciliation remain #304 follow-ups; #305/#306 and final cutover remain gated.
