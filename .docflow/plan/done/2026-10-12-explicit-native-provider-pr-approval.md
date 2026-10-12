# Explicit native provider PR approval with durable uncertain-write recovery

## Owning ADRs

- `../../adr/0020-opentui-rust-stdio-workspace.md` (AC 3–5, 8–10).
- `../../adr/0002-http-in-rust.md`.
- `../../adr/0003-credentials-keychain.md`.

## Scope

Port the retained desktop human PR-approval action into the native provider
list/detail with a read-only preview and explicit confirmation. Rust owns actual
provider identity, reviewer credentials/principal, current PR metadata, pinned
approval and remote write. Do not require a clone or an AI review; this is the
existing human PR action, separate from staged comment publication and review
fix filesystem approval. Persist uncertain attempts before POST and reconcile
read-only after restart instead of retrying an unacknowledged write. No live
provider writes/private credentials/profiles are authorized for implementation
tests. Profile catalog/editor and other #306 parity remain subsequent work.

## Exit Criteria

1. Generated strict preview/confirm/status/reconcile DTOs use full validated
   provider/repository/ordinary PR identity; never accept a URL/provider HTTP
   body or client assertion that a write was approved. Bound opaque token,
   frames/text/collections and reject reserved local IDs/controls/unknown fields.
2. Read-only preview obtains bounded native metadata and the actual authenticated
   reviewer with stable principal ID (GitHub numeric user ID, Bitbucket account
   ID), not display-name/config-order guesses. Prove native destination repository
   identity, head/destination pins, branches/title/fork and OPEN/non-draft state.
   Existing approval is displayed without a duplicate POST. No clone/agent start.
3. Single-use expiring bounded server token binds the exact preview, native actor
   and full scope. Confirm consumes it before provider writes, rechecks native
   actor/repository/pins/metadata/state and rejects drift/expiry/cross-scope/replay.
   GitHub sends explicit captured commit_id and APPROVE; Bitbucket uses the
   native approve endpoint. No automatic comments or unrelated provider mutation.
4. Durable bounded journal transitions to writing before any POST (FULL-sync and
   atomic claim). Identity includes immutable native repository/principal IDs and
   both pins, so alias/title/handle changes cannot bypass an unresolved attempt.
   Acknowledged success is validated; definitive rejection is distinguished from
   timeout/malformed acknowledgment/transport/5xx/uncertain persistence. Unknown
   writes are never retried automatically or silently described as not sent.
5. Restart/status/reconciliation are GET-only, fully scoped and actor pinned.
   Uncertain/writing records block another POST for that approval identity until
   native evidence proves approval. Bitbucket cannot claim a historical commit
   approval from an unpinned mutable participant flag; drift stays explicit.
   Journal follows validated native row ownership, deletion/backup contracts and
   bounds. New schema and rollback compatibility documented; legacy rows intact.
6. Native preview shows provider/repo/PR/reviewer/head/destination/action and the
   explicit effect. Separate confirm/cancel keys; no pasted/late/stale/repeated
   inputs send. Outcome/uncertainty/read-only reconciliation/browser recovery are
   clear, with full-selection fences. Wide/narrow input tests and native frames.
7. Owned HTTP fixtures exercise both actual API adapters and exact request bodies,
   metadata/actor/repo drift, replay/expiry, rejection, accepted-write dropped ack,
   malformed/redirect/rate/size/deadline failures. Fake API/storage and real stdio
   fixtures prove durable claim/restart/no-double-send/full scope/controls. Full
   root/UI/Rust/protocol/strict Clippy/fmt/no-GUI/Archgate/audit/frames/ReactDoctor.
8. Verify #372 merge, rebase onto actual squash and audit collisions before PR.
   Exact-head CI and verified squash merge are required. No #306/epic closure,
   ADR0020 completion, private profile/platform/install waiver or Norn review.

## Dependencies

- PR #372 verified merged on 2026-10-12 at 03:29:22 UTC; squash
  `84921b7cd19eba4d3ed2a07a7b8a4f904f01dd86`. CI38198661868 passed both jobs
  for fixed head `648e253e0dc7c6aca078ae6a7394e4629aa9ed15`. Rebased onto the actual squash.
- Native provider browser/read gates, generated protocol, qualified native owner
  storage and existing explicit publication/recovery practices.
- Official GitHub review commit_id/APPROVE API and Bitbucket approve endpoint;
  internal implementation references must document the latter's missing commit
  CAS parameter rather than invent provider atomicity.


## Prepared completion receipt

Prepared 2026-10-12 from implementation `d503f9b938b0c095d5eb6e0dab04be421d408f2b`
on `issue-306-provider-approval`. Effective only after exact-head CI passes and
verified squash merge into main. This is prepared, not a pre-merge shipped claim.

- Predecessor #372 verified merged at `84921b7cd19eba4d3ed2a07a7b8a4f904f01dd86`
  after exact648e253 CI38198661868 success. Rebased onto actual squash; audit
  passed numbering, metadata/status, INDEX, sections, links and main collision.
  Existing unrelated coverage advisories remain for Accepted0011/0013.
- Native actor/repository/pin proof, strict generated preview/confirm/status/
  reconcile IPC, single-use five-minute bounded token and pre-POST durable
  schema19 journal. GET-only recovery preserves uncertain writes after restart;
  old GitHub commit evidence is distinguished from current reviewer status, and
  Bitbucket cannot infer historical commit approval from changed pins.
- Native list/detail `a` preview, explicit `y`, dismiss, status/reconcile,
  provider-page recovery and keyboard scroll. UI fences full selection,
  generations, paste, late responses, expired previews and repeated inputs.
- Root typecheck, 123 Vitest tests plus tooling, lint/protocol and Archgate17
  passed. UI typecheck, 190 tests/1,181 assertions and 127 native golden frames
  passed; final owned input rerun passed9/64assertions. Inspected both providers'
  wide/narrow preview/uncertainty frames, including compact scrolling.
- Rust no-default-feature full suite passed725 core tests +2 existing ignored,
  61 real stdio, 6 CLI, 5 onboarding and 3 binary tests. Final owned journal
  rerun passed6 tests including consistent SQLite backup after a failed receipt.
  Strict all-features/all-targets Clippy, fmt, no-default all-target compile and
  no-GUI active dependency proof passed.
- ReactDoctor rerun score62; the React complexity finding was fixed by factoring
  outcome selection. Remaining17 DOM-property warnings are false positives on
  OpenTUI native `text` props, proved by renderer types/native frames; no rule
  suppression, dependency installation or unrelated cleanup.
- Fixtures exercised exact HTTP requests, dropped/slow/malformed/foreign/
  oversized/redirect/rate/5xx acknowledgment, drift, expiry/replay, alias/owner,
  corruption/quota, receipt CAS/failure, restart and deletion. No live provider
  writes, private credentials/profiles, Norn review or public release.
- This bounded item leaves #306/#294 and ADR0020 open/Accepted. Profile catalog/
  editor, other parity, native-profile runtime and #307–#309 remain required.
