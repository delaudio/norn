# Current Focus

Git is authoritative. Durable history lives in WORKLOG and plan/done.

## Active state

- **Goal:** complete remaining epic #294 through verified sequential PR/squash
  merges; no Norn reviews, per explicit maintainer instruction.
- **Branch:** `issue-357-provider-snapshots`; plan 0056 claimed before code.
- **Merged baseline:** PR #366 at `dd7562d29b712df6576ed31582123c4a8005e523`,
  exact `8acede5` UI/verify green (verify 13m43s). Plan 0055 shipped.
- **Current work:** bounded immutable provider PR snapshot/file/diff IPC and
  strict client contracts. Keep destination tip, review merge base and head
  distinct; opaque preview handles never replace saved run IDs. Native preview/
  review-start integration and explicit approval/browser opening follow in #357.
- **Local baseline:** root/protocol/UI 160 tests/970 assertions/105 native frames,
  Rust 672 core/52 stdio/six isolated CLI/five onboarding/three bin tests,
  strict all-feature Clippy/fmt/no-GUI and Archgate 12/12 passed.
- **Owned fixture:** `/tmp/norn-owned-pr-snapshot-vjcacx1w`, actual ahead
  destination, dirty working tree, rename/Unicode/binary/mode/header-like source.
- **User data:** unrelated untracked `.norn.yaml` must not be read or staged.

## Next items

- Finish #357, then #358–#364, reconcile #306, harden #307, matched packaging
  #308, removal/cutover #309 and epic closure only after full verified delivery.
- Native profile recovery/platform runtime remains unobserved; ADR 0020 remains
  Accepted until cutover. Inherited publication-fixture intermittency is tracked
  in #307, not called fixed by a passing rerun. ADR 0011/0013 historical
  todo-coverage advisories unchanged, to reconcile relevant retained decisions.
- Tests use owned resources/synthetic providers/fake agents. No actual tokens,
  private profiles, live provider mutations or release publication. Synthetic
  API roots/auth are private cfg(test), never production environment switches.
- Implementation remains single-agent and sequential.
