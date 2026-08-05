# Side-by-side terminal image diffs

## Owning ADRs

- `../../adr/0010-side-by-side-terminal-image-diffs.md`

## Scope

- render decoded base and changed image versions in two columns for modified
  image diffs when the terminal image pane has sufficient width;
- retain the existing single-version and metadata fallbacks for narrow panes,
  unsupported graphics, and image-version failures;
- add focused TUI state and renderer tests.

## Exit Criteria

- ADR 0010 AC1-AC2: both decoded versions render with clear labels and
  per-version metadata in an adequately wide image pane.
- ADR 0010 AC3-AC4: single-version and metadata/error fallback behavior stays
  available for non-comparative states.
- ADR 0010 AC5: non-interactive tests cover comparative and fallback layouts.

## Dependencies

- `../../adr/0010-side-by-side-terminal-image-diffs.md`
- `../../adr/0009-terminal-image-diffs.md`

Shipped as commit `e149569` through PR #148.
