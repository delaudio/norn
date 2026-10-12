# Current Focus

Git is authoritative. Durable history lives in WORKLOG and plan/done.

## Active state

- Goal: finish epic #294 through verified sequential PR/squash merges; no Norn
  review per maintainer instruction. Single agent, no delegation.
- Branch: `issue-306-provider-evidence-start`, isolated worktree
  `/private/tmp/norn-provider-evidence-start`; plan0060 claimed before code.
- Base: #370 final head `789bd0347eed8b56bdc6ef79ca9f50c1966dc5e2`;
  predecessor #370 subsequently verified merged at `a529463b523f7e8039068ccc59fe9243baba0e36`
  after exact final-head `bc7c2ca` CI38193561843 passed. The first CI failure was
  an owned clone missing local Git identity; all58 stdio also pass with personal/
  system Git config disabled. Rebase onto actual squash and audit before PR.
- State: durable exact provider evidence, no-clone review start and saved restart
  files/diff/images/Pierre/reply integration. UI profile/source/generation fences.
- Predecessor local gate: root123+tooling/UI172/1057 assertions/115frames,
  Rust706+two ignored/58stdio/six CLI/five onboarding/three bin, strict Clippy,
  fmt/protocol/no-GUI/Archgate/audit.
- Root unrelated `.norn.yaml` must never be read/copied/staged.

## Remaining work

- Explicit PR approval/external native browsing, #358–#364/#306, #307 hardening,
  #308 packaging, #309 verified removal/cutover, then epic closure.
- Preserve maintainer closures (#357/#305). ADR0020 Accepted until actual full
  completion. Native profile/platform runtime still unobserved. No private
  profiles, live provider writes or public releases authorized.
