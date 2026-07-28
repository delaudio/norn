# Terminal Image Diffs

## Owning ADRs

- `../../adr/0009-terminal-image-diffs.md`

## Scope

- detect PNG, JPEG, GIF, and WebP file changes in TUI diff entries;
- retrieve bounded base and changed blobs through shared Rust provider services;
- decode bounded metadata and image content without persisting remote blobs;
- render supported terminal protocols through `ratatui-image`;
- let modified images switch between base and changed versions;
- retain useful metadata and error fallbacks when inline graphics are
  unavailable;
- document protocol detection, limits, controls, and fallback behavior.

## Exit Criteria

- ADR 0009 AC1: supported image paths and decoded formats are validated.
- ADR 0009 AC2: Kitty, Sixel, and iTerm2 protocols can render the selected
  image inside the TUI diff pane.
- ADR 0009 AC3: added, modified, and deleted image versions select and switch
  deterministically.
- ADR 0009 AC4-AC6: metadata, failure fallback, and resource limits are
  implemented and documented.
- ADR 0009 AC7: text and unsupported binary rendering remains unchanged.
- ADR 0009 AC8: focused non-interactive tests cover all required states.

## Dependencies

- `../../adr/0009-terminal-image-diffs.md`
- GitHub issue #127
- `.docflow/plan/todo/0002-terminal-ui-foundation.md`

Shipped at HEAD `f50e5c88c9bfdcc61a51448bf6d9bb25f946889e` through PR #129.
