# Native repository explorer and bounded readonly consumers

## Owning ADRs

- ../../adr/0020-opentui-rust-stdio-workspace.md, AC3–5/8–10.
- ../../adr/0002-http-in-rust.md.
- ../../adr/0003-credentials-keychain.md.

## Scope

After actual plan0067 merge, begin #359 with native extraction of shipped
repository listing/content/diff/blame behavior and qualified bounded readonly
IPC. Preserve desktop adapters until cutover; do not mix #360 mutations or
approve arbitrary explorer paths as finding/fix authority. This bounded first
slice does not close #359 before its native tree/find/editor UI proof is merged.

## Starting points

- commands/repositories.rs has shipped status enums, porcelain-z parsing,
  listing tracked+untracked plus deleted/renamed/conflicted inventory; file512KiB
  UTF8 preview; HEAD/worktree diff and untracked no-index; line-porcelain blame
  including complete commit messages. Current helpers use unbounded Command.output
  and ambiguous workspace/repo configured lookup: preserve semantics through a
  no-GUI service with full provider identity, canonical root proof and bounded
  cancellation-aware Git argv/file handles; never copy unsafe lookup blindly.
- Desktop RepositoryExplorerPanel filters paths by trimmed lowercase substring
  and changed-only. Ctrl-F searches non-overlapping lowercase substring matches
  in selected file or raw diff; result indices refer to displayed text. Preserve
  these distinct shipped searches, not an invented whole-repo text search.
- Reuse local_snapshot source/options owner/root validation deliberately; explorer
  live reads are not saved review captures. Echo explicit provider/root and file
  content/version fences. State/generation/path changes invalidate late results.
- Use strict generated protocol and shared fixtures, bounded reader workers,
  inventory pages and continuation/version hashes. Handle option-like, Unicode,
  spaces/newlines filenames as literal pathspecs, never shell syntax or ANSI.
- Native readonly context should carry provider/repository/rootId; resolve the
  exact configured source with local_snapshot.resolve_source and compare the
  canonical root hash on every request, without exposing a caller-provided path.
  Inventory continuation pins complete bounded status/path/head/index state and
  rejects drift. Current/imported targets without a configured clone report the
  actual unavailable integration rather than guessing another checkout.
- Content states distinguish binary/UTF8/tooLarge/deleted/missing/unborn, never
  silently truncate canonical bytes. Diff pages retain complete validated UTF8
  SHA and original meaning. Blame supports bounded line pages with full commit/
  author metadata and complete bounded commit messages or explicit unavailable.
- Extract desktop readonly adapters with their current externally observable DTOs.
  Keep branch/worktree/mutation adapters separate for #360. Editor follow-up uses
  native validated argv/platform adapters (Windows cmd-start interpolation is not
  a safe template), explicit graphical/unavailable state and OpenTUI suspend/
  resume for interactive execution, with actual owned terminal smoke.

## Exit Criteria

1. Claim after predecessor actual merge/currentmain audit in isolated worktree.
2. Owned native temporary Git tests cover tracked/untracked/deleted/renamed/
   conflicted/unchanged, Unicode/spaces/option-like and hostile control text,
   exact same-name provider/clone distinction, outside symlinks, binary/large,
   unborn HEAD and bounded large inventory/diff/blame/find; cancellation/stale
   roots and continuation drift fail safely before credentials or writes.
3. Typed stdio shared fixtures and real subprocess tests enforce strict DTOs,
   bounds, root/path/generation, EOF/control responsiveness and no GUI deps.
4. Preserve existing desktop readonly tests via thin native adapters; no silent
   semantic loss or removal of retained editor/tree/status/search behavior.
5. Root/UI/Rust/protocol/Archgate/docflow/Doctor/strictClippy/fmt/noGUI gates pass,
   focused PR own exact-head CI and actual squash; update parity from actualmerge.
   #359/#294 remain open until complete UI/editor follow-up proof. Preserve
   maintainer closures; ADR0020 Accepted until full cutover.

## Dependencies

- Plan0067 PR378 verified squash8cdd00768eba9b162dd08d6b15717188e77dd455
  at2026-10-12T10:39:13Z. All five CI38234834238 jobs SUCCESS on final
  b906071101ba65347b3105358a7fb992a215bb97; all3 native workflow lanes
  actual runtime passed. Generalverify retried only on samehead after slow
  Ubuntu mirror. Root main ff synced before creating this checkout/claim.

## Status

- Claimed by: default-agent, 2026-10-12, single writer.
- Branch: issue-359-native-repository-readers.
- Location: isolated /private/tmp/norn-native-repository-readers on actual378main.
- Blockers: implementation/local gates/own exact-head CI/verified squash merge.
- Stopped:
