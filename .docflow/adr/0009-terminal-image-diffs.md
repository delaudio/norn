---
adr: 0009
title: Render bounded image diffs in supported terminals
status: Implemented
date: 2026-07-28
owner: default-agent
supersedes:
superseded-by:
depends-on: [0002, 0004, 0006]
tags: [tui, diff, images, terminal]
---

# ADR 0009 - Render bounded image diffs in supported terminals

## Context

Provider unified diffs describe raster image changes as binary files. The TUI
therefore cannot show the visual state that a reviewer must inspect, even
though the Rust provider services already retrieve authenticated file previews
for the desktop review interface.

Terminal graphics support is not uniform. Kitty, Sixel, and iTerm2 protocols
can place images inside a terminal layout, while unsupported terminals still
need a stable, useful review surface. Remote image data is untrusted and may be
large or malformed, so fetching and decoding must have explicit limits.

## Capability statement

Lachesi will render bounded PNG, JPEG, GIF, and WebP pull-request image versions
inside the TUI when the active terminal supports Kitty, Sixel, or iTerm2
graphics. The TUI will reuse Rust provider services for base and changed blobs,
allow modified images to switch between those versions, and always expose a
metadata fallback when graphics are unsupported or decoding fails.

## User stories / scenarios

- As a reviewer, I can select an added image and inspect it without leaving the
  TUI.
- As a reviewer, I can switch a modified image between its base and changed
  versions.
- As a reviewer in an unsupported terminal, I can still see the image path,
  side, format, dimensions, and byte size.
- As a maintainer, I can reject oversized or corrupt image data before it can
  destabilize the terminal process.

## Acceptance criteria

1. PNG, JPEG, GIF, and WebP changes are detected from diff paths and verified
   from decoded content before rendering.
2. Kitty, Sixel, and iTerm2 terminals render the selected image version through
   a ratatui-compatible image widget.
3. Added images expose the changed version, deleted images expose the base
   version, and modified images provide a deterministic base/changed toggle.
4. Every image view includes path, side, format, dimensions, byte size, and
   change status when that metadata can be decoded.
5. Unsupported terminals, unsupported or corrupt content, provider failures,
   and oversized content render a non-crashing metadata or error fallback.
6. Provider reads reject payloads over 8 MiB; decoders reject dimensions over
   8192 pixels per side or 40 megapixels before full decode.
7. Text diffs and non-image binary diffs retain their existing rendering path.
8. Tests cover added, modified, deleted, oversized, corrupt, and unsupported
   terminal states without requiring an interactive terminal.

## Out of scope

- Animated playback for GIF or WebP; the first decoded frame is sufficient.
- Pixel-level visual comparison, overlays, or heat maps.
- SVG rendering in the TUI.
- Persisting image blobs outside the active in-memory review session.
- Adding Tauri permissions or exposing provider credentials to the webview.

## Open questions

- None.

## References

- ./0002-http-in-rust.md
- ./0004-diff-rendering.md
- ./0006-terminal-ui.md
- ../../.archgate/adrs/ARCH-001-tauri-react-rust-bitbucket-boundary.md
- https://github.com/lachesi-hq/lachesi/issues/127
- https://github.com/benjajaja/ratatui-image

## Revision History

| Date | Revision | Author | Change |
|------|----------|--------|--------|
| 2026-07-28 | r1 | default-agent | Accepted bounded terminal image diff rendering. |
| 2026-07-28 | r2 | default-agent | Marked implemented after the feature shipped through PR #129. |

## Approvals

| Role | Name | Date | Signature |
|------|------|------|-----------|
| Maintainer | fdg | 2026-07-28 | approved through issue prioritization and implementation instruction |
| Maintainer | fdg | 2026-07-28 | implementation verified through merged PR #129 |
