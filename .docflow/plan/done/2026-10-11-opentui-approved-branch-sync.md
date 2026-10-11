# Approved Branch Sync and Conflict Resolution

## Owning ADRs

- `../../adr/0020-opentui-rust-stdio-workspace.md` (AC 3, 4, 5).

## Scope

Expose branch sync and agent conflict resolution through snapshot-bound,
action-specific preview and approval. Use pinned clones, bounded cancellable
Git/agent commands and the existing native fix operation state. Never push
automatically. Preserve interrupted merges for explicit inspection. Require a
clean source checkout before starting a new merge; allow resolving an existing
merge only against its verified destination. Support an explicitly approved
complete merge commit without pulling unrelated staged paths into it.

## Exit Criteria

1. Typed sync/conflict preview and execution methods use one-use approvals
   bound to target, clone, branches, checkout and merge identity.
2. New sync refuses dirty worktrees and unmerged state. Fetch/fast-forward and
   destination merge are cancellable, bounded and leave edits for inspection.
3. Conflict resolution runs the native agent and verifies no unmerged entries
   remain; failures/cancellation preserve changes and never commit or push.
4. Merge commits approve every changed path and refuse unrelated stage/worktree
   changes. Commit and push remain separate approved steps.
5. Local bare-origin/fake-agent tests cover clean sync, conflicts, stale/wrong
   approvals, failed resolution and merge commit/push. UI input, protocol,
   native frames and repository gates pass.

## Dependencies

- PR #336 must merge before opening this delivery.

## Status

- Claimed by: default-agent, 2026-10-11, `issue-303-approved-branch-sync`.
- Blockers: None; PR #336 merged at `350103f`.
- Stopped:

## Delivery prepared for merge

- PR: https://github.com/delaudio/norn/pull/337
- Effective on merge to main; ADR 0020 remains Accepted until full cutover.
- Local evidence: 34 backend tests, 48 UI tests, 24 checked native frames,
  603 core tests (2 ignored), root gates, Archgate 17/17, protocol and Clippy.
- React Doctor: no issues after separating effects/result rendering.
- Dirty-checkout stash/restore parity and persistent recovery remain open.
