# Current Focus

Git is authoritative. Durable history lives in WORKLOG and plan/done.

## Active state

- **Goal:** finish all remaining epic #294 migration work through verified PRs
  and squash merges; no Norn code reviews (explicit maintainer instruction).
- **Branch:** `issue-306-canonical-scope-picker`.
- **Current change:** plan 0053 / #355 locally verified at `e4264db`; prepared
  completion becomes effective only on merge. PR/checks/integration outstanding.
- **Merged baseline:** `4a303f1042e1747c7479d645392640da0921aed9` (PR #354);
  #304 closed. #302/#303/#305 already closed on GitHub.
- **Verified:** root gate, protocol, UI 143 tests/93 frames, Rust 666 core/51
  stdio/five CLI/five onboarding, strict all-feature Clippy, no-GUI guard.
  Empty TCP connections in inherited HTTP fixtures diagnosed/fixed; assertions
  and production deadlines unchanged. Native recovery is still unobserved.
- **User data:** preserve unrelated untracked `.norn.yaml`; do not read/stage it.

## Next item

- Integrate #355 after exact-head CI passes; complete #306 retained parity
  (provider PR navigation/detail, local base/layer, explorer/branch operations,
  history/analytics, context/polling and native recovery), then #307 hardening,
  #308 matched packaging, #309 removal/cutover, and epic closure.
- ADR 0020 remains Accepted until all cutover criteria are achieved. No live
  provider publication, release publication, real credentials/private profiles
  or user-data removal is authorized by automated verification.
