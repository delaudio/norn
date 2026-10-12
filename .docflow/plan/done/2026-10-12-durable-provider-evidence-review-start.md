# Durable provider evidence and exact-preview review start

## Owning ADRs

- `../../adr/0020-opentui-rust-stdio-workspace.md` (AC 3–5, 8–10).
- `../../adr/0007-headless-review-cli.md`.
- `../../adr/0008-shared-review-service-boundary.md`.
- `../../adr/0003-credentials-keychain.md`.

## Scope

Connect fresh provider previews to the qualified native engine without requiring
or fabricating a local checkout. Persist the exact immutable captured diff and
provider/fork provenance separately from bounded conversation JSON, and bind
saved runs to durable evidence. Retain provider revision/approval checks,
organization policy, cancellation and legacy behavior. Native UI Enter starts
from its explicit snapshot; saved files/diff/images/Pierre/replies remain
reachable after restart. This is retained provider work under #306; explicit PR
approval/external provider browsing and broader history/cutover remain separate.

## Exit Criteria

1. Bounded SQLite evidence records own explicit provider/workspace/repo/PR,
   branches, title, source repository, head/destination/actual-diff-base pins,
   exact patch hash/bytes and proved inventory. No credentials or transient
   handle IDs are persisted as run identity. New schema preserves existing
   retention/disconnect/backup contracts and rejects corrupt/cross-scope evidence.
2. review.start accepts an optional snapshot reference only with exactly matching
   provider target. Validate handle/expiry/cache ownership before credentials or
   agent execution. Recheck current provider pins/metadata/fork provenance using
   bounded shared provider read gates; drift fails rather than refresh/recompute.
3. Persist exact evidence before agent execution and associate the new actual
   run with that evidence. The shared qualified Rust engine gets explicit scope,
   optional canonical clone and actual profile. No clone never implies cwd or a
   providerless lookup; required local/organization evidence fails honestly.
4. Distinct destination approval and actual diff-base pins survive initial and
   reply runs. Replies inherit durable exact evidence and proved conversation,
   including on restart without a clone. Legacy saved runs remain unchanged and
   retain their existing clone-based fallback only when they lack native evidence.
5. Saved evidence supplies bounded file inventory/diff/Pierre previews after
   restart and lets immutable image reads use proved captured source pins;
   corrupt/missing evidence never silently loads current PR or foreign clone.
6. UI sends its exact active snapshot and optional selected/default profile,
   tracks scoped streaming/cancel, and selects the actual completed saved run.
   Preview/source/generation fences prevent stale completion from overwriting
   another PR. Profile/start/reply errors remain recoverable and clear.
7. Owned provider fake HTTP/core/framing and real stdio fake-agent fixtures prove
   exact bytes, provenance drift, no-clone execution, restart/reply, expiry,
   cancellation/EOF, corruption/bounds and provider collisions. No live provider
   writes, private credentials/profiles or Norn review. Full local gates, visual
   frames, ReactDoctor where applicable, exact-head CI and verified squash merge.
8. Integrate only after PR #370 is verified merged. Rebase this isolated work onto
   the actual squash baseline and audit number collisions before opening its PR.

## Dependencies

- Plan0059 / PR #370 qualified native engine: verified squash merge
  `a529463b523f7e8039068ccc59fe9243baba0e36`, CI38193561843 on final
  `bc7c2ca69c5b41de7012d4561805aba1878a389d`.
- Existing immutable snapshot manager/provider capture, shared prompt/policy,
  scoped storage/session and approved filesystem boundaries.


## Prepared completion receipt

Prepared 2026-10-12 from implementation commit
`402c889` on `issue-306-provider-evidence-start`. This becomes a completion
record only after exact-head CI passes and verified squash merge into main.

- Actual predecessor squash `a529463b523f7e8039068ccc59fe9243baba0e36` is the
  branch baseline. Rebased and audited ADR/plan numbering/INDEX/links; no collision.
- Criteria 1/4/5: owned storage dedup/hash/corruption/provider isolation,
  parent cascade/legacy retention and consistent-backup tests; saved-run distinct
  destination/diff-base pins; real stdio GitHub/Bitbucket restart/files/patch/reply
  without clone/current PR. Raw legacy row bytes remain unchanged. Missing or
  corrupt native evidence does not invoke legacy/current checkout fallback.
- Criteria 2/3/7: owned HTTP both-provider metadata recheck rejects head,
  destination, title, branches/fork and GitHub count changes with exactly one
  GET/no diff recapture; cache ownership/missing handle/cancel/shutdown fences
  before agent. Real shared initial pipeline with owned fake CLI proves exact
  raw patch, durable actual run association and no-clone isolated execution.
  Existing snapshot expiry/bounds/framing and native policy/cancel tests pass.
- Criterion 6: strict generated DTO/source/profile tests and UI generation
  fences; native F6 explicit/default/cancel/paste behavior at 120 and 50 columns.
  Added two profile scenes, 117 native frames verified including changed F6 help.
  Named/default profile semantics retain shared Rust policy validation; unknown
  explicit profile is an error. Catalog/editor parity remains retained #306 work.
- Local gate: root typecheck/123 tests plus tooling/lint/protocol; UI typecheck,
  177 tests/1088 assertions/117 frames; Rust 712 core plus two existing ignored,
  59 stdio/six CLI/five onboarding/three bin; strict all-feature/all-target Clippy,
  fmt, all-target core/no-GUI, Archgate17 and docflow audit. React Doctor reports
  no changed-file issues, existing score64. No suppressed native prop rules.
- New schema18 requires a compatible binary on restore; packaging rollback
  remains a required #308 owned-backup/runtime gate. Evidence quota-full errors
  do not silently evict saved runs. Broader history/recovery, explicit PR
  approval/external browsing, catalog/editor/platform/install/cutover remain.
- No Norn reviews, live provider writes, private profile/data or releases.
  ADR0020 remains Accepted; this change does not close #306 or epic #294.

Shipped implementation commit (conditional on verified PR merge): `402c889`.
