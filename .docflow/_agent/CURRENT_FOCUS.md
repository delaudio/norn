# Current Focus

Git is authoritative. Durable history lives in WORKLOG and plan/done.

## Active state

- Goal: complete remaining epic #294 via verified sequential PR/squash merges;
  no Norn review, per maintainer instruction. Single-agent implementation.
- Branch: `issue-306-provider-inline-engine`, isolated worktree
  `/private/tmp/norn-provider-inline-engine`; plan0059 claimed before code.
- Predecessor #369 merged at `68c69399cc879b29873846a65fd7c79ad42baee9`
  after exact final-head CI38190456486 passed both jobs, 2026-10-12 01:22:43Z.
- State: native context/profile/initial/reply/fix integration implemented;
  local gates passed and rebased onto the actual squash baseline with no
  tree changes; collision audit passed. Preparing PR; exact-head CI/merge pending.
- Fresh-preview review start stays unavailable until durable cached evidence.
- #369 local gate: 705 core+two ignored/55 stdio/six CLI/five onboarding/three
  bin, root123/UI172+1057asserts/115frames, strict Clippy/fmt/no-GUI/Archgate/audit.
- Root unrelated untracked `.norn.yaml` must never be read/copied/staged.

## Next items

- Exact cached-evidence start/approval/external browsing, #358–#364/#306, #307
  hardening, #308 packaging and #309 full verified removal/cutover, then epic.
- #357 maintainer closure preserved. ADR0020 Accepted; native profile/platform
  runtime remains unobserved. No live provider writes/private profiles/releases.
