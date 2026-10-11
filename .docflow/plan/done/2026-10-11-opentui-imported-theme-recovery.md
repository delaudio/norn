# Explicit imported theme consumption and recovery route verification

## Owning ADRs

- `../../adr/0020-opentui-rust-stdio-workspace.md` (AC 5, 9).

## Scope

Add an explicit terminal action to inspect the canonical imported theme and
preview its effect on the settings draft, then approve staging only that
preference. The existing explicit settings save retains its version fence. Do not silently replace newer settings on startup or
mutate the archived imported record. Preserve unrelated preferences and fence
pending results on disposal/navigation. Coordinate with unsaved settings edits.
Pin the recovery guide to a merged, complete desktop exporter revision and
verify its source/build contract with isolated synthetic fixtures; document
exactly which native recovery observations remain for packaging/cutover.

## Exit Criteria

1. Imported dark/light theme is read from Rust-owned canonical records; preview
   shows imported/current values and requires a separate explicit staging confirmation and existing explicit save.
   Missing/wrong/changed data and stale settings versions fail clearly, keeping
   existing preferences and source records intact.
2. Settings/editor cancellation and pending theme actions do not silently discard
   edits, run credential operations, or apply stale terminal state. Tests cover
   duplicate input, disposal, conflict and other preference preservation.
3. Recovery docs use a complete merged immutable source revision and original
   production origin/application identity; synthetic source/profile tests do not
   claim actual cross-platform native recovery observation.
4. Root typecheck/test/lint/Archgate and terminal typecheck/test/frame gates pass;
   sequential PR integrates after #348. #305 remains open for staged-draft and
   reference consumption coordinated with #304/#306 and final recovery gates.

## Dependencies

- PR #347 supplies status-only onboarding and isolated CLI doctor regression.
- PR #340 supplies exported desktop/canonical user data; PR #344 record reads.
- Integrate after PR #348; only independent preparation while its CI runs.

## Status

- Claimed by: default-agent, 2026-10-11, `issue-305-imported-theme-recovery`.
- Blockers: local validation, PR #348 merge and this PR CI/merge.
- Stopped:

## Delivery

Effective on merge to main of this settings/recovery PR. Implementation
`c128831`, prepared above physical image PR #348 (its merge remains required
before integrating). Imported themes use `workspace.read`, a separate modal
preview/staging action, record-version/content revalidation and the existing
explicit settings save/CAS. Other draft fields and the archived record are
retained; missing data/conflicts/disposal/duplicate input fail or fence safely.
Opening user data now retains an open settings session instead of disposing its
unsaved draft; returning restores it without credential or settings reload.

Root typecheck, 123 tests/tooling (including the new archive workflow contract),
lint and Archgate 17/17 pass. UI typecheck, 109 tests/648 assertions and
61 native frames pass. Native input covers modal approval, no premature save,
draft retention across the user-data overlay and explicit save. React Doctor
reports 12 unknown-DOM-property warnings for native `flexShrink` props in
SettingsPanel; these are accepted by the installed OpenTUI types and exercised
by actual native layout/input/frame tests. No suppression or global-clean claim.

Pinned archive `72440eaef4177b809664bf6b563647d7659f3e52` is the complete merged
desktop export bridge. Its actual exporter executes against six bounded
synthetic canonical/legacy DOM fixtures with source/cache/credential checks.
The archived production frontend and embedded desktop compile on local macOS
26.2; two isolated directory-copy/symlink-rejection tests pass. No desktop was
launched and no native profile/keychain/provider/agent was accessed. The new
three-platform recovery CI must pass before merge; it compiles the same archive
and tests isolated profile copying, without claiming actual native profile
recovery. Cross-platform run evidence remains pending until that CI completes.

Draft/reference consumption and remaining native recovery/cutover gates stay
explicit. #305 and the migration decision remain open/Accepted; no Norn review.
