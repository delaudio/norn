# Current Focus

Git is authoritative. Durable history lives in WORKLOG and plan/done.

## Active state

- Goal: finish epic #294 via verified sequential PR/squash merges; no Norn
  review per maintainer instruction. Single agent, no delegation.
- Branch: `issue-306-review-profile-catalog`, isolated `/private/tmp/norn-review-profile-catalog`;
  plan0063 local gates passed; completion prepared, awaiting its fixed-head CI/verified merge. Native declared-profile catalog/effective-default
  picker with exact target/path policy resolution; no profile rewrite API.
- #371 merged3322ff0. #372 verified merged84921b7 on 2026-10-12 at 03:29:22 UTC;
  fixed648e253 CI38198661868 passed both jobs. Rebased onto actual main.
- #373 fixed2db1dcb CI38202396806 passed both jobs; verified merged ac077cd at 04:23:57 UTC.
  Native approval local proof is prepared. Keep its head fixed and verify the
  merge/rebase/audit before the next PR. Native profile catalog implementation and local proof complete; PR preparation next.
- Tests use only owned providers/agents/data; no live provider writes/private
  profiles/releases. Root unrelated `.norn.yaml` never read/copied/staged.

## Remaining work

- Ship declared profile catalog/default picker; policy file editor belongs to
  #359, canonical prompt editor is already delivered. #358–#364/#306, #307 hardening,
  #308 packaging, #309 verified removal/cutover, then epic closure.
- Preserve maintainer closures (#357/#305). ADR0020 Accepted until actual full
  completion. Native profile/platform runtime still unobserved. No private
  profiles, live provider writes or public releases authorized.
