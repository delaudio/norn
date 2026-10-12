# Current Focus

Git is authoritative. Durable history lives in WORKLOG and plan/done.

## Active state

- Goal: finish epic #294 through verified sequential PR/squash merges; no Norn
  review per maintainer instruction. Single agent, no delegation.
- Branch: `issue-306-provider-external-browser`, isolated worktree
  `/private/tmp/norn-provider-external-browser`; plan0061 claimed before code.
- PR #371 fixed head `78ca5d07949fe12303b618eca2affbe849787545`: UI passed,
  verify CI38197325446 running. Do not modify that fixed predecessor branch.
- State: explicit full-scope native provider PR external browser action; prepare
  here, integrate only after verified #371 merge, actual-squash rebase and audit.
- Root unrelated `.norn.yaml` must never be read/copied/staged.

## Remaining work

- Explicit PR approval/external native browsing, #358–#364/#306, #307 hardening,
  #308 packaging, #309 verified removal/cutover, then epic closure.
- Preserve maintainer closures (#357/#305). ADR0020 Accepted until actual full
  completion. Native profile/platform runtime still unobserved. No private
  profiles, live provider writes or public releases authorized.
