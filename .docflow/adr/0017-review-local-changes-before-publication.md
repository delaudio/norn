---
adr: 0017
title: Review local changes before publication
status: Accepted
date: 2026-09-03
owner: codex
supersedes:
superseded-by:
depends-on: [0004, 0006, 0007, 0016]
tags: [local, review, git, tui, desktop, diff]
---

# ADR 0017 - Review local changes before publication

## Context

Norn currently presents provider pull requests as its primary interactive
review targets. Developers also need to review work before it becomes a pull
request: commits that exist only on the current branch, staged and unstaged
changes, and eligible untracked files. The headless CLI can already review a
working tree, and the repository explorer can inspect individual local files,
but neither the terminal nor desktop review workspace presents the complete
unpublished state as a first-class target.

Local review cannot rely on a provider pull-request identifier, remote comment
threads, or provider publication actions. It must remain read-only with respect
to the repository while producing a stable snapshot that both interactive
interfaces can render. The terminal UI is the primary delivery surface; the
desktop app follows as a second consumer of the same native capability.

## Capability statement

Norn exposes unpublished local repository work as a first-class review target,
combining commits ahead of the configured upstream with staged, unstaged, and
eligible untracked changes into one bounded snapshot that the terminal UI and
desktop app can inspect and submit to the existing local AI-review workflow.

## User stories / scenarios

- As a developer, I can inspect everything on my current branch that has not
  reached its upstream before I commit, push, or open a pull request.
- As a terminal user, I can select Local alongside provider pull-request states
  and use Norn's native or browser diff views without leaving the TUI.
- As a desktop user, I can inspect the same local snapshot through the shared
  diff viewer after the terminal-first capability is available.
- As a reviewer, I can run an AI review against a stable local snapshot without
  exposing provider-only actions that cannot apply to unpublished work.

## Acceptance criteria

1. A shared native local-review snapshot reports repository identity, current
   branch, upstream identity when configured, commits ahead of upstream, a
   unified diff, changed-file metadata, and explicit warnings without modifying
   the repository.
2. With an upstream configured, the snapshot contains tracked changes between
   the upstream tree and the current working tree exactly once, including
   committed-but-unpushed, staged, and unstaged changes; eligible untracked text
   files are appended under the existing bounded and sensitive-path rules.
3. Without an upstream, Norn reports the condition explicitly and still shows
   staged, unstaged, and eligible untracked working-tree changes relative to
   `HEAD`; repositories without commits receive a deterministic supported
   fallback.
4. The terminal UI presents Local alongside Open, Draft, and Merged, supports
   keyboard and mouse selection, lists only configured local repositories with
   usable paths, and refreshes the selected snapshot on demand.
5. A terminal local target supports the native unified/split diff and the
   authenticated browser diff viewer, with repository and branch context that
   does not pretend the target is a provider pull request.
6. The terminal AI-review action can review the loaded immutable local snapshot
   through the shared local execution policy, while provider approval, remote
   comment loading, staged provider-comment publication, and branch-sync actions
   are unavailable for that target.
7. The desktop app consumes the same native snapshot and shared diff renderer,
   exposes Local alongside provider states, and applies the same provider-action
   restrictions and empty/error states.
8. Snapshot size, file count, binary handling, path validation, sensitive
   untracked-file exclusion, cancellation, and stale asynchronous result
   fencing remain bounded and covered by automated tests.
9. Existing provider pull-request review behavior, browser-diff authentication,
   headless working-tree and branch scopes, and GitHub/Bitbucket integrations
   remain backward compatible.

## Out of scope

- Committing, pushing, stashing, discarding, or otherwise modifying local
  changes from the Local review target.
- Publishing comments or approvals before a provider pull request exists.
- Treating the shared or self-hosted review service as an execution target for
  an uncommitted working tree.
- Watching the filesystem continuously; explicit refresh is sufficient for the
  initial capability.

## Open questions

- None.

## References

- [Diff rendering](./0004-diff-rendering.md)
- [Terminal UI](./0006-terminal-ui.md)
- [Headless review CLI](./0007-headless-review-cli.md)
- [Shared review service boundary](./0008-shared-review-service-boundary.md)
- [Browser diff viewer](./0016-browser-diff-viewer.md)

## Revision History

| Date | Revision | Author | Change |
|------|----------|--------|--------|
| 2026-09-03 | r1 | codex | Initial draft. |
| 2026-09-03 | r2 | codex | Accepted the TUI-first local review target contract after maintainer approval. |

## Approvals

| Role | Name | Date | Signature |
|------|------|------|-----------|
| Maintainer | delaudio | 2026-09-03 | approved implementation in chat |
