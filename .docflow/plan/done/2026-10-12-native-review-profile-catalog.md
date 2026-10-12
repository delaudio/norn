# Native review profile catalog and effective default selection

## Owning ADRs

- `../../adr/0020-opentui-rust-stdio-workspace.md` (AC 3–5, 8–10).
- `../../adr/0002-http-in-rust.md`.
- `../../adr/0003-credentials-keychain.md`.

## Scope

Port the actual shipped desktop repository-policy profile catalog/default picker
into OpenTUI, extending the delivered bounded F6 manual name selection. Rust owns
policy parsing, organization/repository/local policy resolution and effective
default; TypeScript displays and selects declared names. Selection must reach the
same native review-start/reply resolution used by existing reviews. Preserve the
already delivered canonical repository/user prompt editor. The desktop profile
catalog is read-only; do not invent a policy-profile rewrite API. Explicit policy
file editing is part of retained repository/editor integration in #359.

## Exit Criteria

1. Generated bounded strict review.profiles request/result use the full selected
   target and reject unknown fields/invalid local/provider/run identity before
   policy/credential access. Rust returns only the bounded profile catalog,
   effective default and clear availability/validation state, not arbitrary raw
   configuration, analyzer commands, secrets or browser policy resolvers.
2. Provider targets use only their exact provider-qualified explicit configured
   checkout, canonical root and actual saved owner where relevant. No providerless
   path or backend cwd fallback. No-clone provider review keeps valid base/default
   operation, displays absent local-policy catalog honestly, and cannot invent
   named/org repository profiles. Local targets prove the actual selected local
   snapshot against the backend's canonical Git repository.
3. Catalog/default resolution reuses the native engine's actual policy seam and
   profile semantics, including organization/local/repository layers, declared
   names and invalid/unknown/default profiles. Bound returned names/collections/
   diagnostics. Fail before agent launch on invalid policy rather than silently
   choosing base. No agent, provider, policy-file, Git index or worktree writes from catalog/navigation. Existing verified organization-policy cache maintenance remains in the native resolver; do not bypass rollback checks or change policy layer semantics.
4. Native F6 exposes actual names and the effective repository default with
   keyboard selection, refresh/error/cancel and narrow/wide rendering. Selecting
   the default passes null (native effective default); selecting a named profile
   passes that exact name. Preserve the older manual input when the backend does
   not advertise the new capability, and existing user prompt editing.
5. Full-target/generation fencing discards late catalog data and prevents a
   profile from another repository/provider/run becoming selected after target
   changes. Paste/navigation/cancel cannot start a review or rewrite policy.
   Selection is reset/revalidated across target changes and changed catalogs.
6. Owned Rust/policy/stdio/client/native input fixtures cover both providers,
   same owner/repo across providers, explicit clones/no clone, local identity,
   missing/invalid/default/named/layered profiles, delayed reads, cancellation,
   stale selection, malformed/oversized responses and actual start handoff.
   Owned fake agents/policy only; no private profiles/credentials/config or root
   unrelated .norn.yaml reads. Native Storybook frames at wide/narrow sizes.
7. Root typecheck/tests/lint/protocol/Archgate; UI typecheck/tests/frames and
   ReactDoctor; Rust core/stdio/no-GUI, strict all-feature/all-target Clippy/fmt;
   docflow numbering/metadata/links/INDEX/main-collision audit all pass.
8. PR #373 must pass exact-head CI and be verified squash-merged before this
   item's PR; rebase onto its actual squash and audit collisions. Completion
   requires this PR's exact-head CI/verified merge. No #306/#294 closure,
   ADR0020 completion or parity/platform/native-profile/install waiver.

## Dependencies

- PR #373 fixed head `2db1dcb8dba87aafd48b16c1e5b42858319b31fb`,
  CI38202396806 passed both jobs; verified merged `ac077cd48ff96d6a6248f1ee941dd7904e2d9ce5` at 04:23:57 UTC. Rebased onto the actual squash.
- Shared native review/policy resolution, explicit provider clone ownership,
  target validation, F6 manual selection and canonical prompt editor.

## Prepared completion — effective only after verified merge

- Implementation commit: `8e57d2a97b43950e3fde99b4932684e6c57ac7d4`; exact remote-owner correction `57875cd2d5c1b2c93d682744ca609573bf3ea6d7`.
- Predecessor PR #373 passed exact-head CI38202396806 and was verified merged
  as `ac077cd48ff96d6a6248f1ee941dd7904e2d9ce5` at 04:23:57 UTC. Rebased
  onto the actual squash; main numbering/metadata/link audit passed.
- Root typecheck,123 Vitest tests plus tooling, lint (0 errors/3 existing infos),
  generated protocol and Archgate17 checks passed.
- UI typecheck,197 tests/1,225 assertions and135 native frame fixtures passed.
  Eight new frames cover catalog/base/error/loading at120x30/50x18; owned
  native input also proves full128-character wide Unicode names at50x18.
- Rust no-default full suite passed732 core tests +2 existing ignored,62 real
  stdio,6 CLI,5 onboarding and3 binary tests. Strict all-features/all-targets
  Clippy, fmt, no-default all-target compile and no-GUI dependencies passed.
- Owned native catalog unit/signed policy/Git/stdio tests prove strict targets,
  profile bounds/default/missing/invalid policy, actual signed precedence,
  exact checkout/remote/local snapshot, changed snapshots/ambiguous paths,
  same names across providers, no-clone base and continued control. The owned
  fake-agent native review tests fetch the catalog before proving selected and
  null/default profile payloads and pins for both providers.
- UI verifies full-target/generation late-result fences, reconnect/target reset,
  cancel, refresh errors, removed names, malformed/oversized/foreign DTOs,
  paste isolation, explicit choose/start handoff and old-backend fallback.
- ReactDoctor final64; remaining3 unknown-DOM-property warnings on OpenTUI
  native `text` properties are proved valid by renderer types/native frames.
  No rule suppression, dependency installation or unrelated cleanup.
- Native policy semantics remain unchanged: local override precedence applies
  through the configured organization resolver; verified-cache maintenance
  and rollback/signature checks are preserved. No invented profile rewrite API.
- No Norn review, private profiles/credentials, live provider writes or public
  release. This item does not close#306/#294, implementADR0020 or waive actual
  local-workflow/repository/editor/native-profile/install/platform gates.
- Completion requires this fixed head's required CI and verified squash merge;
  the prepared done file is not evidence that those have happened.

- Final exact-origin hardening uses native parsed provider/owner/repository
  equality, never a substring. Both-provider owned negatives cover repository
  suffix, foreign host and nested path; the full Rust gate and integration
  gate passed again. Supersedes the prior bd9a511 CI head; final CI must verify
  the new fixed head after this receipt.
