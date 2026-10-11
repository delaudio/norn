# Terminal image comparison delivery (internal)

Owning decision: [0020](../../.docflow/adr/0020-opentui-rust-stdio-workspace.md).
Owning issue: #302. Text anchor/performance and installed terminal smoke remain
separate gates before cutover; this delivery completes image comparison behavior.

## Identity, limits and protocol

The selected changed-file entry controls both sides. Stored PR reviews use their
recorded base/head revisions and verify provider/repository/PR identity. Working
checkout comparisons use HEAD for before and the bounded current file for after.
Renames resolve the old path from Git; added/untracked files have no before image,
deletions have no after image. Paths outside the target's changed files, escapes,
symlinks, unavailable revisions and mismatched formats return clear failures.
No provider token or arbitrary filesystem path is returned to the browser.

`file.preview` accepts optional `side`, `offset`, `maxBytes`, `contentSha256`.
Missing side means new, preserving old small-preview callers. Each chunk is at
most 192 KiB, keeping base64 plus envelope comfortably under the 1 MiB frame.
The full image remains limited to 2 MiB; callers must use chunks above 192 KiB.
Responses carry side/availability, total size, offset/nextOffset, full content hash
and dimensions. Nonzero offsets require the preceding hash. A changed file during
loading fails stale; the client validates progression, metadata, total bounds and
final SHA-256 before rendering. Late selection stops subsequent chunks and is
fenced from workspace state.

PNG, JPEG, GIF, WebP and BMP headers are validated in Rust through the existing
pure Rust image library, now an engine dependency (no terminal/GUI toolkit).
Dimensions retain the prior 8,192-per-axis and 40-million-pixel limits; decoder
allocation limits are applied before metadata extraction. Unsupported native
formats still report a per-side decoder error. SVG previews use the browser
fallback and are never sent to the native raster decoder.

## Presentation and verification

Wide terminals show before/after columns; narrow ones stack the sides. A missing
side is explicit. A failed decode on one side preserves the other side and is
request-fenced. Native protocol selection follows OpenTUI's pinned capability
resolver: Kitty where supported, Sixel with pixel resolution, otherwise block
previews. The product labels the block fallback and offers `b` for the existing
authenticated browser comparison. No browser server security policy is relaxed.

The Git subprocess tests use synthetic BMP/PNG fixtures and temporary repositories:
large multi-chunk images with frame bounds, old/new byte equality, content drift,
unchanged-path refusal, rename sources, additions, deletions, untracked images,
malformed data, oversize and symlinks. Terminal tests cover assembly/version fences,
non-progressing chunks, stopping requests after selection changes, missing sides
and stale renderer errors. Both deterministic colored PNG sides are decoded by
OpenTUI's native decoder; protocol selection tests cover Kitty/Sixel/tmux/no-capability
fallbacks. Six native frame scenes cover modified/wide/narrow, added/deleted,
renamed and unsupported states.

Native frame snapshots and decoder/capability tests are **not** a manual physical
terminal protocol smoke. This workspace's tool PTY cannot verify that an external
Kitty/Sixel terminal displayed pixels. Installed supported-terminal image display,
resize and cleanup remain in the #307/#308 final platform smoke gate; no physical
terminal display proof is claimed here. #302 remains open for that gate and precise
split scrolling/performance measurements.
