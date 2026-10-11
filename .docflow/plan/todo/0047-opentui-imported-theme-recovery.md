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
