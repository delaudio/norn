# Native repository tree, preview and find presentation

## Owning ADRs

- ../../adr/0020-opentui-rust-stdio-workspace.md, AC3–5/8–10.
- ../../adr/0002-http-in-rust.md.
- ../../adr/0003-credentials-keychain.md.

## Scope

After actual plan0068 source merge, reconcile its proven completion receipt
and claim the next bounded #359 native tree/content/diff/blame/find UI slice.
External-editor platform adapters and real suspend/resume terminal proof remain
an explicit following slice before #359 closure. No branch/worktree mutations,
arbitrary explorer finding anchors or provider writes.

## Implementation direction

- Independent ExplorerStore/panel over the real six repository.explore methods.
  Full provider/repository/root context and selected file/mode/generation fence
  every asynchronous application. F8 opens configured selected repo; Escape/F8
  returns to the preserved review target/scroll/finding/thread selection. Busy,
  unconfigured/moved clone, malformed/stale and empty states are visible and
  recoverable by refresh/settings, not guessed alternate sources.
- Preserve the shipped configured clone-directory fallback deliberately: the
  first readonly API requires explicit localPath, while the retained desktop
  adapter still accepts its proven configured fallback. Extend only explorer
  source resolution to a fully provider-qualified unique fallback whose Git
  root/origin are reverified; never guess a different provider/current clone,
  weaken immutable local review's explicit source policy or rewrite settings.
- Bounded32-row paged tree, expandable folder groups, explicit all/changed and
  trimmed lowercase path filter; preserve unchanged, rename old source, deleted,
  untracked and conflict statuses. Clip tree/preview independently at narrow
  sizes, with real keyboard focus and selected row visible after paging.
- Content/diff use32KiB windows and pinned complete hashes. Store only bounded
  visible window/history metadata, not the entire repo or8MiB patch. UTF8 byte
  offsets and page boundaries are explicit. Present binary/large/missing/deleted
  states, unborn HEAD, errors and refresh without blank/false-success output.
  Shipped Markdown source/preview toggle uses the installed native Markdown
  renderer where feasible, with actual frame proof and no browser dependency.
- Blame32-line page retains all author/commit details; selected row exposes
  complete bounded commit message with scrolling. Native Ctrl/Cmd-F searches
  selected content/rawdiff with explicit apply/next/previous/result paging and
  highlight/jump. Extend find result with original byte start/end alongside
  UTF16 coordinates as needed to jump directly into a bounded preview window;
  schema/sharedfixtures/native/clienttests stay strict and matched.
- Escape all repository filenames/content/metadata control sequences for
  terminal presentation while preserving original bytes/positions for transport,
  hashing/search and argv. Paste contributes only bounded text to active input;
  pasted shortcuts never launch review/fix/editor/publish or close the panel.
- Reuse actual OpenTUI dimensions/render declarations; no DOM assumptions,
  diagnostic suppression or fake production fallback. Keep fixture transport
  only in tests/Storybook. Public help/footer adds F8 without internal IDs.

## Exit Criteria

1. Claim only after actual predecessor merge; reconcile0068 todo-to-done using
   actual final-head checks and squash hash, never a prepared/unmerged receipt.
2. Meaningful fake/native input tests for wide/narrow tree/folders/status/filter/
   paging/content/diff/blame/Markdown/find/focus/return; both providers, stale
   roots/generations/late responses, paste, malformed/unavailable states.
3. Owned production transport/Rust/native input workflow opens F8, selects a
   literal file, navigates modes/find and returns to the exact review selection
   on all3OS; no private profiles/credentials/provider HTTP/actualagent/write.
4. Native Storybook frames cover wide/narrow loading/empty/error/tree/status/
   content/binary/deleted/blame/Markdown/find and preserve supported rendering.
5. Root/UI/Rust/protocol/Archgate/audit/Doctor/Clippy/fmt/noGUI gates; focused PR
   own exact-head platform CI and actual squash. #359/#294 remain open until
   final editor proof. ADR0020 stays Accepted until finalcutover.

## Dependencies

- Plan0068/PR379 actual verified squashb15d80bf2c2f275e22c68a100fd1ca2124c4542d
  at2026-10-12T11:48:19Z; all5CI38243490575 SUCCESS on
  final766fa6c084f24079580eacf0fa91cf948f37838c, including real3OS runtime.

## Status

- Claimed by: default-agent,2026-10-12, single writer after verified379merge.
- Branch: issue-359-native-explorer-ui.
- Location: isolated /private/tmp/norn-repository-explorer-ui on actual379main.
- Blockers: implementation/local gates/own exactheadCI/actualverifiedmerge.
- Stopped:
