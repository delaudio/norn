# Approved Commit and Push for Terminal Fixes

## Owning ADRs

- `../../adr/0020-opentui-rust-stdio-workspace.md` (AC 3, 4, 5).

## Scope

Port explicit commit and push controls for native fix sessions. Reuse the
snapshot-bound preview/approval flow and native Rust fix state. Commit previews
bind the exact message and selected files; unrelated staged files remain staged
and are excluded from the commit. Push previews bind the source branch, origin
and committed HEAD. Both operations remain explicit and cancellable.

## Exit Criteria

1. Typed preview/execution requests use short-lived, one-use approval bound to
   the action as well as the loaded run, thread, clone and content identity.
2. Commit requires a completed fix and a nonempty message; selected paths are
   validated and unrelated staged files are excluded from the new commit.
3. Push requires a successful commit for the same session/branch/HEAD and does
   not force push. The UI displays the resulting commit and operation outcome.
4. Stale, wrong-action, expired and replayed approvals are rejected; cancellation
   or timeout never causes an automatic retry and ambiguous remote outcomes are
   reported for inspection.
5. Fake-agent/local-origin subprocess tests, UI composer/input tests, native
   frames, protocol and root/UI/Rust gates pass.

## Dependencies

- PR #335: native approved fix pipeline. Merge this delivery after #335.

## Status

- Claimed by: default-agent, 2026-10-11, `issue-303-approved-commit-push`.
- Blockers: PR #335 must pass CI and merge before this delivery is opened.
- Stopped:
