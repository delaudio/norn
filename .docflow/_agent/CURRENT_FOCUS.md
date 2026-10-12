# Current Focus

Git is authoritative. Durable history lives in WORKLOG and plan/done.

## Active state

- Goal: complete epic#294 through verified sequential PR/squash merges. Single
  writer, no delegation; never use Norn for review per maintainer instruction.
- PR#375 verified MERGED5e4e1b293b7044c693b5f8f2e8308f59d810cebf
  at05:58:26UTC after both required CI38209164847 jobs passed on
  final42dea41dc7723be0a5ad503fc973038f3b47ba89. Root main synced.
  Plan0064 is shipped: qualified immutable local snapshots, schema20 evidence,
  independent layers, same-comparison stale recheck/cancel and validating client.
  Gates: Rust744/64stdio/6CLI/5onboarding/3binary+2existingignored;
  root123+tooling/Archgate17; UI201/1254/135frames; Doctor100;
  Clippy/fmt/noGUI/protocol/docflow audit passed.
- Branch issue-358-local-review-lifecycle, isolated
  /private/tmp/norn-local-review-lifecycle; plan0065 claimed before code.
  Connect exact captured local source to native engine/payload/operation/cancel
  and durable history/findings/output first. Actual local terminal controls,
  immutable preview/Pierre and full workflow proof remain the next #358 slice.
  Qualified native start/recheck/cancel and bounded local history/findings/output/
  conversations are implemented in the isolated branch; local final gates pass:
  Rust745 core/72stdio/6CLI/5onboarding/3binary+2existingignored; root123+tooling,
  lint/protocol/Archgate17/audit; UI206/1277/135frames; Doctor100;
  noDefault allTargets/strict allFeature allTarget Clippy/fmt/noGUI. PR pending.
  Eight real owned native child/stdio cases cover providers/wrong defaults/layers/
  base/missing checkout/clone/persistence/cancel/EOF/preflight/failure/oversize/publication.
  Observed failures led to RAII read-permit release and compact-envelope private
  resume redaction; preserve logs. Required-preflight test also separated provider
  clean-commit checks from captured Local dirty/opaque evidence. Cancellation
  remains in the native context through policy/preflight/retries. Client five/23.
  Actual UI/Pierre proof is still a following slice, not claimed from backend tests.
- Fixtures only: no live provider writes/private credentials/profiles/releases,
  no root unrelated .norn.yaml read/copy/stage.

## Remaining work

- Complete #358–#364/#306, #307 hardening, #308 matched installed packaging,
  #309 verified removal/cutover, then close the epic. Profile catalog374 and
  immutable local foundation375 are merged. Policy editor is #359; canonical
  prompt editor is shipped. Local UI/engine parity is still open.
- Preserve maintainer closures including #357/#305. ADR0020 Accepted until
  actual complete cutover. Native desktop-profile runtime/platform recovery
  remains unobserved; compile/copy smoke is not evidence of recovery.
