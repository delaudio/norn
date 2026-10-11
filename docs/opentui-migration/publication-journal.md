# Durable publication journal

Plan 0051 implements the engine foundation for ADR 0020 publication under #304.
It does not expose provider writes, IPC approval or a terminal publish shortcut.

## Exact approved payload

`ApprovedComment` contains provider/repository/PR/local draft identity, reviewed
head/base, explicit inline/file/general mode, path, one old/new line, optional
parent, and original body. Validation rejects local review IDs, missing pins,
unsafe paths, contradictory anchors, controls and oversized rendered markdown.
The actual provider body is `markdown()`: original text plus one deterministic
hidden marker. The eventual UI must preview this exact rendered text, select
only provider-supported modes/parent combinations, and verify canonical CAS and
single-use approval before calling `publish_approved`; the Rust API itself is
not an authorization token or a public IPC endpoint.

Scope aliases use lowercase workspace/repository in the stable identity key;
the approved payload digest still binds every exact input field. Editing an
uncertain or acknowledged payload does not create another reservation. A
separate provider or PR has a separate identity.

## Durability and state transitions

SQLite schema 16 adds `local_draft_publications`, preserving existing data and
schema migrations. An immediate transaction writes the exact payload, digest
and attempt token as `writing` before any provider access. Production journal
connections use synchronous FULL and a bounded busy timeout. No time-based
lease expiry converts an uncertain operation into a new POST.

- A verified preflight failure sends nothing and marks the reservation rejected.
- The adapter performs one POST. Only an unambiguous no-write result may be
  rejected; timeout/disconnect/invalid acknowledgment is uncertain.
- A confirmed remote ID is retained even when the head changes afterward.
  `publishedSnapshotChanged` records a verified mismatch; `publishedUnverified`
  records unavailable postflight evidence. Neither is a normal publication
  acknowledgment for finding feedback or automatic draft removal.
- Replaying writing/uncertain/acknowledged identity sends nothing. A changed
  pending payload is rejected. A definitely rejected attempt requires a new
  explicit approval, gets a new token and fences old acknowledgments.
- Reconciliation scans read-only through the provider adapter. Only an exact
  authenticated-author/body/marker/PR/parent/anchor match may acknowledge a
  pending write. Absence, incomplete scans, malformed matches or errors stay
  uncertain. Reconciliation never POSTs or deletes comments.
- An acknowledged but unverified entry rechecks only its revision, retaining
  the remote ID. A storage failure after remote creation leaves the durable
  writing reservation available for later read-only reconciliation.

The adapter contract is a trust boundary to implement next: a marker substring
alone does not establish a successful publication. These local guarantees do
not assert exactly-once delivery by third-party provider APIs. User actions on
remote comments cannot be inferred from a missing scan result.

## Evidence and remaining integration

Nine fake-provider tests use owned SQLite files initialized with the actual
production migration. They cover writing-before-send, success/replay, separate
batch entries with partial rejection, provider isolation, edited uncertain
payloads, scope aliases, timeout after remote success, reopening and exact
reconciliation, missing matches, crashes, concurrent reservations, corruption,
stale pre/postflight, unavailable postflight, renewed rejected attempts, old
acknowledgments and storage failures before/after send. No real provider,
credentials, agent or Norn review is invoked.

Concrete bounded GitHub/Bitbucket adapters, typed IPC single-use batch approval,
terminal preview/status/thread actions, canonical draft cleanup and atomic
finding-feedback/reconciliation integration remain #304. Batch acceptance and
native publication input are not proven by independent journal-entry tests.
Final recovery, parity, hardening, installed packaging and cutover remain open.
