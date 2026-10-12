# Current Focus

Git is authoritative. Durable history lives in WORKLOG and plan/done.

## Active state

- Goal: finish epic #294 through verified sequential PR/squash merges; no Norn
  review per maintainer instruction. Single agent, no delegation.
- Branch: `issue-306-provider-external-browser`, isolated worktree
  `/private/tmp/norn-provider-external-browser`; plan0061 claimed before code.
- PR #371 fixed head `78ca5d07949fe12303b618eca2affbe849787545`: UI passed,
  verify CI38197325446 passed. #371 verified merged at
  `3322ff050c8d7c6bc115c01c9341cb0594cb4730`, 2026-10-12 03:00:52Z.
  This work is rebased onto actual squash; number/INDEX/link audit passed.
- State: explicit full-scope native provider PR external browser action; prepare
  here; local gates pass: root123, UI181/1117/119frames, Rust713+two
  ignored/60stdio/six CLI/five onboarding/three bin, strict Clippy/fmt/no-GUI/
  protocol/Archgate17/audit. Preparing PR/receipt; exact next-head CI required.
- Root unrelated `.norn.yaml` must never be read/copied/staged.

## Remaining work

- Explicit PR approval/external native browsing, #358–#364/#306, #307 hardening,
  #308 packaging, #309 verified removal/cutover, then epic closure.
- Preserve maintainer closures (#357/#305). ADR0020 Accepted until actual full
  completion. Native profile/platform runtime still unobserved. No private
  profiles, live provider writes or public releases authorized.
