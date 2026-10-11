# Current Focus

Git is authoritative. Durable history lives in WORKLOG and plan/done.

## Active state

- **Goal:** complete all remaining epic #294 work through verified PRs/squash
  merges; no Norn reviews, per explicit maintainer instruction.
- **Branch:** `issue-357-native-pr-browser`; plan 0055 implemented at `df7ab05`; prepared receipt
  in plan/done, effective only on verified PR merge.
- **Merged baseline:** PR #365 at `2f2cd50d3314b09528f323f40778c0eb45cc4879`,
  exact `8e2e36d` UI/verify green. #355 closed; #357/#306 remain open.
- **Current work:** integrate second #357 delivery after exact-head UI/verify CI.
  Native filters/pages/detail/full-scope selection implemented and locally
  verified; pre-review pinned diffs and explicit approval follow.
- **Verified bridge:** root/protocol/UI 160 tests/105 frames, Rust 672 core/52
  stdio/six CLI/five onboarding, strict all-feature Clippy/no-GUI. CLI status now
  runs under an owned child mock keyring; inherited publication fixture
  intermittency remains tracked in #307, never called fixed by a rerun.
- **User data:** unrelated untracked `.norn.yaml` must not be read or staged.

## Next items

- Finish #357, #358–#364, reconcile #306, harden #307, matched packaging #308,
  removal/cutover #309 and epic closure only after full verified delivery.
- Native-profile recovery/platform runtime remains unobserved. ADR 0020 remains
  Accepted until full cutover. Tests use owned resources/synthetic providers/fake
  agents, never actual credentials/private profiles/live provider mutations.
- Temporary next-plan brief: `/tmp/norn-plan-0055.md`; repository plan above is
  authoritative after claim. Implementation stays single-agent/sequential.
