---
adr: 0006
title: Support a terminal UI as a second local review interface
status: Implemented
date: 2026-07-23
owner: default-agent
supersedes:
superseded-by:
depends-on: [0002, 0003, 0004]
tags: [tui, cli, rust, review-ui]
---

# ADR 0006 - Support a terminal UI as a second local review interface

## Context

Lachesi is currently a Tauri desktop app with a React webview and Rust native
services. The product is also a local-first pull request review workspace, so a
terminal interface can serve users who already live in terminals and want a
fast review surface close to their local repository tools.

The terminal interface must not become a second provider implementation. The
existing decisions keep external-provider HTTP, credential lookup, local
repository operations, and review publication in Rust, with secrets outside the
webview. Those boundaries are still the right ones for a TUI: the TUI should
share the same native services and product semantics instead of duplicating
network clients, storing secrets differently, or publishing comments
immediately.

Keep terminal rendering and layout testable, keep terminal raw-mode lifecycle
small and robust, and restore terminal state on normal exit, interruption, and
panic. Pull request diffs should remain native to the review workspace instead
of requiring branch checkout or delegation to a separate git TUI.

## Capability statement

Lachesi will support a terminal UI as a second local review interface in this
repository. The TUI runs as a separate Rust entrypoint, reuses the same native
configuration, credential, provider, local-repository, and review services as
the desktop app, and preserves Lachesi's staged review workflow.

## User stories / scenarios

- As a reviewer, I can browse configured repositories and open pull requests
  from a terminal without launching the desktop webview.
- As a reviewer, I can inspect pull request details, comments, and unified
  diffs using the same provider data and credentials as the desktop app.
- As a reviewer, I can stage review comments locally and explicitly publish
  them in a batch, matching the desktop review model.
- As a reviewer, I can inspect provider pull request diffs in a native terminal
  view without checking out the source branch or changing my local worktree.
- As a maintainer, I can test TUI layout and view state without a real terminal
  session or provider network calls.

## Acceptance criteria

1. The TUI is implemented in this repository as a separate Rust entrypoint or
   workspace crate, not as a separate repository.
2. Provider HTTP, credential lookup, configuration, local repository
   resolution, and review storage are reused from Rust native modules rather
   than reimplemented for the TUI.
3. Tauri command names and mock IPC contracts remain stable unless a command
   contract intentionally changes in the same implementation change.
4. The first TUI release supports configured repositories, open pull request
   listing, selected pull request details, comments, and unified diff viewing.
5. TUI review comments are staged locally first and published only through an
   explicit batch publish action.
6. Terminal rendering and layout have focused tests using a terminal test
   backend or equivalent non-interactive renderer.
7. The native diff workflow does not check out pull request branches or widen
   shipped Tauri capabilities, and the TUI restores terminal state on normal
   exit, interruption, and panic.
8. Starting AI review from the TUI skips local evidence analyzers because
   repository validation belongs to the development flow before Lachesi review.

## Out of scope

- Splitting the TUI into a separate repository before shared Rust boundaries are
  stable.
- Replacing the Tauri desktop app or React webview.
- Rebuilding the full feature set of `lazygit` inside Lachesi.
- Adding new provider credentials or token stores for the TUI.

## Open questions

- None.

## References

- ../../.archgate/adrs/ARCH-001-tauri-react-rust-bitbucket-boundary.md
- ../../.archgate/adrs/ARCH-002-stage-review-comments-locally-and-publish-in-batches.md
- ../../.archgate/adrs/ARCH-003-keep-tauri-command-and-mock-ipc-surfaces-in-sync.md
- ../../.archgate/adrs/ARCH-004-keep-tauri-commands-thin-and-delegate-to-native-service-modules.md
- ../../.archgate/adrs/ARCH-006-tauri-native-capability-scope.md
- ./0002-http-in-rust.md
- ./0003-credentials-keychain.md
- ./0004-diff-rendering.md
- https://github.com/lachesi-hq/lachesi/issues/80
- https://github.com/lachesi-hq/lachesi/issues/81
- https://github.com/lachesi-hq/lachesi/issues/82
- https://github.com/lachesi-hq/lachesi/issues/83
- https://github.com/lachesi-hq/lachesi/issues/84
- `../../src-tauri/src/tui/terminal.rs`

## Revision History

| Date | Revision | Author | Change |
|------|----------|--------|--------|
| 2026-07-23 | r1 | default-agent | Accepted the terminal UI as a second local review interface. |
| 2026-07-24 | r2 | default-agent | Made TUI AI review skip duplicate local analyzers. |
| 2026-07-30 | r3 | default-agent | Aligned native and split diff workflows with the shipped implementation and marked the terminal UI capability implemented after PR #146. |

## Approvals

| Role | Name | Date | Signature |
|------|------|------|-----------|
| Maintainer | fdg | 2026-07-23 | approved in chat |
| Maintainer | fdg | 2026-07-24 | approved analyzer skip revision in chat |
| Maintainer | fdg | 2026-07-30 | approved shipping through PR #146 |
