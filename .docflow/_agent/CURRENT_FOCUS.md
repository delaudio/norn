# Current Focus

Git is authoritative. Durable history lives in WORKLOG and plan/done.

## Active state

- **Goal:** complete remaining epic #294 through sequential verified PR/squash
  merges; no Norn reviews, per explicit maintainer instruction.
- **Branch:** `issue-357-native-provider-preview`; plan 0057 claimed before code.
- **Merged baseline:** PR #367 at `22b5b1a441593b2dce9423dd3006b0b143dabf2b`;
  exact `5718adb` UI and verify green (verify 11m45s), merged 2026-10-12 local.
- **Current state:** plan 0057 implementation/local gate complete; preparing
  verified PR, not merged yet. Native previews preserve saved-run identity and
  share bounded immutable fork raster/Pierre evidence; F3/F5 and EOF fences.
- **Gate:** root 123 tests plus tooling/typecheck/lint/protocol, UI 172 tests/
  1057 asserts/115 native frames (ten new, 105 unchanged), Rust 697 core plus
  two existing ignored/53 stdio/six CLI/five onboarding/three bin tests; strict
  all-feature/all-target Clippy/fmt/no-GUI, Archgate 12/12 and docflow audit pass.
  React Doctor 64/100, three known native OpenTUI property false positives,
  no complexity finding/errors/suppressions.
- **Tracking correction:** maintainer closed #357 at 2026-10-11 22:01:23Z,
  observed via fresh events/API. Preserve closed state; exact-evidence review
  start and explicit approve/external provider browsing still require retained
  workflow verification under #306.
- **Baseline verification:** root 123 tests plus tooling/typecheck/lint/protocol,
  UI 164 tests/994 assertions/105 native frames; Rust 686 core with two existing
  ignored/53 stdio/six isolated CLI/five onboarding/three bin tests, strict
  all-feature/all-target Clippy/fmt/no-GUI and Archgate 12/12 pass.
- **HTTP fixture cause:** macOS inherited O_NONBLOCK; delayed/fragmented test
  failed before fix with immediate WouldBlock/BrokenPipe. Both fixtures restore
  blocking workers, regression/full Mac gate and Ubuntu CI pass. No production
  deadline/retry changes; all temporary diagnostics removed.
- **User data:** unrelated untracked `.norn.yaml` must not be read or staged.

## Next items

- Finish remaining provider workflows under #306, then #358–#364, reconcile
  #306, harden #307, matched packaging
  #308, removal/cutover #309, and close epic only after full verified delivery.
- Native profile recovery/platform runtime remain unobserved; ADR 0020 remains
  Accepted. No actual tokens, private profiles, live provider mutations or public
  release publication. Synthetic API roots/auth remain private cfg(test).
- Implementation remains single-agent and sequential.
