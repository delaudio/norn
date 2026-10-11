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
- Blockers: PR #340 awaiting CI; prepare isolated image work meanwhile.
- Stopped:
