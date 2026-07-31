---
adr: 0010
title: Compare modified image versions side by side in the terminal UI
status: Proposed
date: 2026-07-31
owner: default-agent
supersedes:
superseded-by:
depends-on: [0006, 0009]
tags: [tui, diff, images, terminal]
---

# ADR 0010 - Compare modified image versions side by side in the terminal UI

## Context

The terminal image-diff view currently loads both base and changed versions of
a modified raster image, but presents one version at a time. Switching is
useful in narrow terminals, yet makes visual comparison of small or subtle
changes slower because the reviewer cannot see both states at once.

The existing terminal image-diff capability already bounds provider reads and
decoding, detects the active graphics protocol, and has a metadata fallback.
The comparative layout must reuse those loaded versions and retain a useful
single-image experience when the terminal is too narrow or graphics are
unavailable.

## Capability statement

Lachesi will present the base and changed versions of a modified supported
raster image side by side in a sufficiently wide graphics-capable terminal.
The existing single-image selection remains available as a fallback and for
added or deleted images.

## User stories / scenarios

- As a reviewer, I can compare an image's base and changed versions at the
  same time, so that I can spot visual changes without repeatedly switching.
- As a reviewer in a narrow or unsupported terminal, I can still inspect the
  selected image version and its metadata without a broken layout.

## Acceptance criteria

1. A modified PNG, JPEG, GIF, or WebP image renders base and changed versions
   simultaneously when both decode successfully and the image pane is wide
   enough for two readable columns.
2. Each comparative column identifies its side and keeps the existing decoded
   metadata and error reporting for its version.
3. Added and deleted images retain their existing single-version rendering.
4. Narrow panes, unsupported terminal graphics, and one-sided decode or fetch
   failures retain a non-crashing selected-version fallback.
5. The terminal image-diff state and renderer have focused non-interactive
   tests for the comparative and fallback layouts.

## Out of scope

- Pixel-level visual comparison, overlays, heat maps, or image-difference
  generation.
- Changing desktop image-diff rendering.
- Persisting image blobs beyond the active review session.

## Open questions

- None.

## References

- ./0006-terminal-ui.md
- ./0009-terminal-image-diffs.md
- ../../.archgate/adrs/ARCH-001-tauri-react-rust-bitbucket-boundary.md

## Revision History

| Date | Revision | Author | Change |
|------|----------|--------|--------|
| 2026-07-31 | r1 | default-agent | Initial draft. |

## Approvals

| Role | Name | Date | Signature |
|------|------|------|-----------|
