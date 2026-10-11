# Windowed text diffs and exact wrapped anchors

## Owning ADRs

- `../../adr/0020-opentui-rust-stdio-workspace.md` (AC 3, 4, 7).

## Scope

Replace full React row mounting with a visible row window and overscan in both
unified and split modes. Preserve existing 2,000-row collapse/expand semantics,
word wrap, old/new labels, metadata and annotation placement. Compute split row
heights with the same native width/wrap implementation as rendered text, including
Unicode and tabs; anchor positions use cumulative visual heights, independent of
whether an anchored child is mounted. Resize recomputes geometry, and keyboard,
mouse/scrollbar movement refreshes the window. Keep side/mode/file fencing and
unified horizontal scrolling. Benchmark expanded 1,000/10,000-row diffs and record
mount/repaint/scroll cost and bounded mounted-row counts honestly.

## Exit Criteria

1. Expanded large diffs mount only viewport plus bounded overscan rows; top/bottom
   spacers preserve complete scroll extent and end-of-file access.
2. Native-render tests cover offscreen old/new anchors, wrapped rows, Unicode/tabs,
   resize and scrolling near both ends, with no missing or duplicate lines.
3. Collapse and expand remain accessible and preserve correct source anchors.
4. Expanded large-diff benchmarks record host/runtime and repeated scroll timing;
   thresholds and limits are explicit. Remaining matched Ratatui/platform evidence
   stays a separate final gate rather than being claimed by this partial delivery.
5. Root lint/typecheck/test, Archgate, terminal typecheck/tests/frames and React
   Doctor pass. No provider publication or Norn review.

## Dependencies

- Plan 0039 / PR #341 must merge before integration.

## Status

- Claimed by: default-agent, 2026-10-11, `issue-302-windowed-diff`.
- Blockers: PR #341 CI pending; isolated local preparation only.
- Stopped:
