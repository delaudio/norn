# Before/after terminal image comparison

## Owning ADRs

- `../../adr/0020-opentui-rust-stdio-workspace.md` (AC 3, 4, 5, 7).

## Scope

Complete the image part of #302: explicit old/new sides for modified, added,
deleted and renamed files, pinned to the reviewed range or bounded working-tree
comparison. Preserve the 2 MiB image bound through chunks below the protocol frame
limit, fence chunks against content drift, and render before/after with clear
missing-side and unsupported-protocol/format fallbacks. Keep browser security and
headless dependencies unchanged. Text anchor/performance parity remains a separate
bounded delivery under #302 before final cutover.

## Exit Criteria

1. Typed side/range/content-version parameters and bounded responses never exceed
   the 1 MiB protocol frame. Invalid ranges, paths outside changed files, symlinks,
   unsupported formats, oversize and changed content fail clearly.
2. UI previews both sides where present, the surviving side for additions/deletions,
   handles renames correctly, decodes bounded content once and fences late responses.
3. Isolated Git/provider fixtures cover old/new/rename/delete/untracked and chunk
   size/drift. Native UI states and protocol/frames/Rust/root gates pass.
4. Native image protocol smoke is recorded honestly on available terminals; text
   frames do not stand in for protocol support. Unsupported terminals have an
   explicit authenticated browser fallback. Remaining platform smoke stays a final
   parity gate if the runtime environment cannot display images.

## Dependencies

- #301 merged. Integrate after PR #340; #302 stays open for text/performance gates.

## Status

- Claimed by: default-agent, 2026-10-11, `issue-302-image-comparison`.
- Blockers: None; PR #340 merged at `72440ea`. Physical terminal smoke remains an explicit final platform gate.
- Stopped:

## Delivery

Effective on merge to main of the image comparison PR. Root 123 tests/tooling,
Archgate 17/17, 72 terminal tests/42 native frames, 616 core tests (2 ignored)
and the no-GUI guard, 39 backend subprocess tests, strict all-feature/all-target
Clippy, protocol drift and React Doctor pass. Decoder/capability tests cover
Kitty/Sixel/tmux selection; physical-terminal display is not claimed. Text
anchor/windowing/performance remains in #302; ADR 0020 remains Accepted until
complete migration.
