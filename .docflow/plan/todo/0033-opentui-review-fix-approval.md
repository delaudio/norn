# Approved Native Review Fix Operations

## Owning ADRs

- `../../adr/0020-opentui-rust-stdio-workspace.md` (AC 3, 4, 5).

## Scope

Expose the existing Rust fix pipeline through typed stdio requests. The
terminal requests a preview for the loaded review run and explicitly confirms
its filesystem/network effects. A short-lived single-use approval is bound to
the target, conversation, canonical clone and its content snapshot. Rust builds
the fix payload, runs the agent and owns cancellation and result inspection.
Commit/push/conflict/sync parity follows in separate bounded deliveries.

## Exit Criteria

1. Preview is read-only and describes checkout, stash, fetch/pull and agent edits;
   approval replay, expiry, different targets and changed snapshots are rejected.
2. A native fix seam pins the approved clone and reuses existing orchestration;
   the UI cannot send arbitrary shell commands or start a fix without approval.
3. Operations stream bounded progress, remain cancellable, and report partial
   edits honestly. Cancellation reaps the owned agent before reporting terminal
   status; failure never commits, pushes or publishes automatically.
4. Fake agents and temporary Git repositories prove approval, stale state,
   successful edits, failure and cancellation; UI input and frames cover preview.
5. Root and UI gates, generated protocol checks and Rust core tests pass.

## Dependencies

- PR #334: stored thread/run selection. Merge this delivery after #334.

## Status

- Claimed by: default-agent, 2026-10-11, `issue-303-review-fix-approval`.
- Blockers: PR #334 must pass CI and merge before this delivery is opened.
- Stopped:
