# Current Focus

Git is authoritative. Durable history lives in WORKLOG and plan/done.

## Active state

- Goal: complete epic #294 through sequential verified PR/squash merges;
  no Norn reviews, per explicit maintainer instruction.
- Branch: `issue-306-provider-review-scope`; plan 0058 claimed before code.
- Baseline: PR #368 merged at `1f9b3d3e0d491c2321817e21bd6f1944194665df`
  on 2026-10-12T00:18:43Z after exact `af7f523` UI/verify success;
  CI 38186484683, verify 12m58s. Plan 0057 receipt is now effective.
- State: implementing provider-qualified native review storage/session identity
  before exact cached-evidence review-start integration. Preview Enter remains
  unavailable until the complete shared Rust engine integration is verified.
- Maintainer closed #357 on 2026-10-11T22:01:23Z; preserve closure. Retained
  review-start/profile/approval/external browsing remains tracked by #306.
- User data: unrelated untracked `.norn.yaml` must not be read or staged.

## Next items

- Finish retained provider workflows, #358–#364 and parity #306, then #307
  hardening, #308 matched packaging, #309 verified removal/cutover and epic.
- Native profile recovery/platform runtime remain unobserved; ADR 0020 Accepted.
  No real credentials, private profiles, live provider writes or public releases.
- Single-agent, sequential implementation; do not use Norn for code review.
