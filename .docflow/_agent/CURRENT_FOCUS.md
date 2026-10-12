# Current Focus

Git is authoritative. Durable history lives in WORKLOG and plan/done.

## Active state

- Goal: complete remaining epic #294 via verified sequential PR/squash merges;
  no Norn review, per maintainer instruction. Single-agent implementation.
- Branch: `issue-306-provider-inline-engine`, isolated worktree
  `/private/tmp/norn-provider-inline-engine`; plan0059 claimed before code.
- Base: final #369 head `0e828f7`; predecessor CI 38190456486 running. Do not
  open/integrate this PR until #369 exact-head verification and squash merge;
  rebase onto actual merged baseline and audit number collisions first.
- State: explicit scoped native inline execution/profile/reply/fix integration.
  Fresh-preview review start stays unavailable until durable cached evidence.
- #369 local gate: 705 core+two ignored/55 stdio/six CLI/five onboarding/three
  bin, root123/UI172+1057asserts/115frames, strict Clippy/fmt/no-GUI/Archgate/audit.
- Root unrelated untracked `.norn.yaml` must never be read/copied/staged.

## Next items

- Exact cached-evidence start/approval/external browsing, #358–#364/#306, #307
  hardening, #308 packaging and #309 full verified removal/cutover, then epic.
- #357 maintainer closure preserved. ADR0020 Accepted; native profile/platform
  runtime remains unobserved. No live provider writes/private profiles/releases.
