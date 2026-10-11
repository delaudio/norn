# Current Focus

Git is authoritative. Durable history lives in WORKLOG and plan/done.

## Active state

- **Goal:** complete all remaining epic #294 work through verified PRs and squash
  merges; no Norn reviews, as explicitly instructed by the maintainer.
- **Branch:** `issue-357-provider-pr-reads`; plan 0054 claimed before code.
- **Merged baseline:** PR #356 at `2a7f0fe605d3012c2e2b9a0c03e1d15b60f7c574`,
  exact-head UI/verify CI green; #355 closed. #302/#303/#304/#305 closed.
- **Current work:** first bounded #357 delivery: native provider PR list/detail/
  current-user reads and strict bounded protocol. Terminal browsing, pinned
  pre-review diffs and explicit PR approval follow; #357/#306 remain open.
- **User data:** unrelated untracked `.norn.yaml` must not be read or staged.

## Next items

- Finish #357, then #358–#364 in order, reconcile #306, harden #307, package #308,
  remove legacy surfaces #309 and close epic only after verified full delivery.
- Native-profile recovery/platform runtime is unobserved; no mock/compile waiver.
  ADR 0020 remains Accepted until full cutover. Verification uses owned resources,
  synthetic provider fixtures and fake agents, never private profiles, actual
  credentials, live provider mutations or release publication.
