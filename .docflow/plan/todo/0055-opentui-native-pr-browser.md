# Native provider PR browsing and metadata navigation

## Owning ADRs

- `../../adr/0020-opentui-rust-stdio-workspace.md` (AC 3–5, 9).

## Scope

Second bounded #357 delivery after the native read bridge: native OpenTUI
repository/author/state filters, grouped PR lists with per-repository pagination,
actual metadata/reviewers/revisions, selection and refresh. Use generated DTOs
and the Rust bridge, including GitHub/Bitbucket semantics and UUID aliases.
Preserve full provider identity and saved-run selection when opening a remote
PR in the existing workspace. Pending pinned pre-review files/diffs and explicit
at-most-once approval are subsequent required #357 deliveries; do not close
#357/#306 or claim those actions already work.

## Exit Criteria

1. A discoverable native shortcut opens the PR browser. Repository selection
   includes All/configured scopes and applicable imported provider scopes;
   identical names on different providers remain distinct. Open includes drafts,
   Draft/Merged/All retain native meaning. Author filtering uses actual native
   identity (including current user), never display-name/provider guessing.
2. Pages and errors are independent by repository; raw empty filtered pages keep
   a reachable next-page action. Per-repo continuation, duplicate IDs, request
   generations and explicit refresh preserve selection while bounding pages,
   total loaded results/bytes/concurrency and native mounts. Exceeding a bound
   surfaces a useful filter choice instead of claiming complete results.
3. Detail displays actual title/description, author, branches, head/base, state,
   draft, reviewer and approval evidence. Metadata refresh does not reset or
   alter an unsaved canonical draft/reference/prompt/reply/fix approval.
4. Explicit workspace selection merges only missing provider/PR identities;
   saved run IDs are preserved and canonical scopes deduplicated without fake
   runs. PRs without stored reviewed ranges show the current explicit limitation,
   never a working-tree diff. Subsequent pinned-diff delivery removes that gap.
5. Late repository/filter/page/detail/current-user responses cannot alter a new
   selection or closed panel. Escape/quit/resize work while reads are pending.
   Synthetic transports enforce read-only requests and owned restart/reconnect,
   wrong-provider IDs, malformed/oversized errors and rate-limit states.
6. Wide/narrow component stories and native input cover actual cross-provider
   browsing, filters/pages/detail/workspace selection, refresh and settings/auth
   recovery. Inspect new native frames and verify existing layout/color changes.
7. Root typecheck/tests/lint/Archgate, protocol, UI typecheck/tests/frame checks,
   React Doctor, Rust core/stdio/isolated CLI/no-GUI and strict all-feature/all-
   target Clippy pass. Merge the exact CI-verified head and update parity evidence;
   ADR 0020 remains Accepted until full cutover.

## Dependencies

- Plan 0054 / PR #365 merged at `2f2cd50d3314b09528f323f40778c0eb45cc4879` after exact-head UI/verify CI success.
- Canonical saved scopes, real saved runs and settings/authentication already
  merged; only owned fixtures/fake agents for tests, no private credential/profile
  access, Norn review, live provider mutation or release publication.

## Status

- Claimed by: default-agent, 2026-10-12, `issue-357-native-pr-browser`.
- Blockers: implementation, local gates and exact-head PR integration.
- Stopped:
