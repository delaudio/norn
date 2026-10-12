# Provider-qualified inline review execution and profile handoff

## Owning ADRs

- `../../adr/0020-opentui-rust-stdio-workspace.md` (AC 3–5, 8–10).
- `../../adr/0007-headless-review-cli.md`.
- `../../adr/0008-shared-review-service-boundary.md`.
- `../../adr/0003-credentials-keychain.md`.

## Scope

Replace the temporary legacy inline handoff fence with explicit native execution
context for provider reviews, replies and approved fix operations. Reuse the Rust
review pipeline and provider CLI adapters. Native scope, clone and profile cannot
be inferred from providerless configuration or substituted by cwd/autodiscovery.
Preserve old CLI/desktop wrappers and legacy data. This is retained work under
#306; fresh immutable-preview review start and durable provider evidence follow
once this engine seam is verified. Fresh preview Enter remains unavailable here.

## Exit Criteria

1. Native initial/reply sessions persist new runs only into the qualified native
   owning row and use scoped runtime lookup/cancellation. Legacy runs remain
   byte-preserved. A legacy reply imports only its proved conversation, not old
   mutable runs. Ambiguous/unproved conversations and duplicate ownership fail.
2. Explicit native context supplies provider and optional canonical clone. A
   missing clone never invokes providerless lookup or cwd fallback. Agent review
   executes in isolated context; local evidence analyzers cannot claim pinned PR
   evidence from a dirty/different-head checkout or silently skip mandatory work.
3. review.start.profile is bounded/validated and applied by the shared Rust
   policy/profile resolution before agent execution. Persist actual selected
   explicit/default profile; reject unknown explicit profile. Canonical prompt
   overrides/references, organization policy and reply-resume behavior remain.
4. Keep destination approval pin in reviewed_base_sha. Optional actual diff base
   is distinct and used by diff readers; native replies retain both. Old run
   records and public headless JSON remain compatible when this field is absent.
5. Approved fix/commit/push/sync/conflict runtime identity and state lookup include
   native provider scope and thread identity. Preserve approved clone/content/
   cancellation fences and separate approvals; legacy desktop wrappers retain
   their old identity. No namespace hack in workspace/repository values.
6. Real stdio with owned fake CLI proves same names/PR IDs across providers,
   different clones, selected/default/unknown profiles, saved native/legacy
   reply inheritance, isolated execution, independent cancel/restart and existing
   approved filesystem workflows. No Norn review, live provider writes or private
   credentials/profiles. Policies/data/repos are owned temporary fixtures.
7. Root/UI/Rust/protocol/Archgate/docflow gates pass, exact-head CI and verified
   squash merge required. Integrate only after PR #369 is verified and merged;
   rebase isolated work onto the actual squash baseline before opening this PR.

## Dependencies

- Plan 0058 / PR #369 provider storage/session foundation: merged at
  `68c69399cc879b29873846a65fd7c79ad42baee9` after exact-head CI38190456486.
- Existing shared pipeline, prompt/reference and approved filesystem seams.
- Subsequent durable cached-evidence start before fresh provider review parity.

## Prepared completion receipt

Prepared 2026-10-12 for PR integration from implementation commit
`4db90405f7d80692c54ea491facc97de32ec8110` on
`issue-306-provider-inline-engine`. This path becomes a completion record only
after exact-head CI succeeds and the PR is verified squash-merged into main.

- Predecessor PR #369 verified merged at `68c69399cc879b29873846a65fd7c79ad42baee9`.
  This branch was rebased onto that actual squash baseline without tree changes.
- Local gate: root typecheck/123 tests plus tooling/lint/protocol; UI typecheck,
  172 tests/1057 assertions/115 unchanged native frames; Rust 706 core plus two
  existing ignored/58 stdio/six CLI/five onboarding/three bin tests; strict
  all-feature/all-target Clippy, fmt, no-GUI, Archgate and docflow audit.
- Criteria 1–4: owned initial/reply legacy-preservation fixtures, ambiguous
  conversation rejection, native conversation inheritance, configured clone
  selection across providers, selected/default/missing profiles and separate
  approval/diff-base pins; mandatory analyzer clean pinned checkout fences.
- Criteria 5–6: native fix state with identical thread IDs stays provider scoped;
  existing approved fix/commit/push/sync/conflict tests pass. Real simultaneous
  GitHub/Bitbucket sessions use different configured clones with identical
  repository/PR/run/thread identities; cancelling GitHub lets Bitbucket succeed.
- No React changes, Norn reviews, live provider writes, private profiles/data or
  public releases. Optional diff-base omission preserves old saved/public output.
- Fresh-preview review start still requires durable cached provider evidence.
  No full parity/epic completion or owning ADR status advancement is claimed.
