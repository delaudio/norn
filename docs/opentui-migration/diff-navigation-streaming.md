# Native diff navigation and streaming evidence

Press `/` to edit a case-insensitive, literal search in the currently loaded
file's diff. Enter applies; Esc keeps the previous query; Ctrl+U clears the
input. Bracketed paste stays inside the editor and never starts reviews/fixes.
Queries are limited to 512 UTF-8 bytes without control characters. Empty input
clears search; no matches produce a notice. `]`/`[` navigate matching rows with
wrapping. Results include headers/metadata and both old/new source rows.
At most 10,000 matching rows are retained, with a visible bounded-result notice.
Changing target/file/content invalidates matches. No regex execution or provider
request is involved in searching.

Search/finding requests take precedence over retained scroll positions. Switching
mode with an active anchor requests that anchor in the new layout. Without an
active anchor, each target/run/file/content/mode/collapse combination has its own
vertical/horizontal position. The transient cache retains at most 32 positions
and is cleared when the shell unmounts. A changed patch/run cannot inherit an
old offset. Deferred layout applies positions only after the full spacer extent
exists; resize continues to use native Unicode/tab/word-wrap geometry.

Split geometry now recognizes two-column rows even when full-width metadata
comes first. Metadata occupies one row; paired code uses native per-column word
wrapping. Text boxes are constrained to the measured column dimensions and fill
the reserved row height. Tests inspect actual offscreen search navigation,
Unicode/wrapped content and stable listener counts after unmount/remount and
mode/identity changes.

The OpenTUI 0.5.17 React `createRoot().render()` helper creates a new reconciler
container on each call. The app calls it once, then uses store/state updates.
The native test harness now does the same rather than accumulating independent
roots and stale listeners when requesting a new anchor. No dependency patch or
warning suppression is used.

## Repeatable stream workload

Run from the repository root:

```sh
bun run ui/scripts/streaming-diff-bench.ts /tmp/norn-streaming-diff.json
```

The actual mounted Shell displays a 10,000-line selected-file patch at 120x30.
Each mode receives 10,000 normalized operation events in 100 batches of 100.
Every batch also attempts a wrong-target event, changes the actual native scroll
position, and renders two frames with a task yield between them. Assertions
require visible diff body content, correct event ownership/sequence, no more
than 64 mounted rows, a 100-entry retained log ring, batch+frame p95 at most
20 ms and current RSS at most 512 MiB. Missing panels/content and budget failures
exit nonzero. No backend/agent/provider is launched and no credentials are read.

Three separate-process artifacts are committed under
`benchmarks/2026-10-11/streaming-{1,2,3}.json`. All selected-patch hashes match:
`0b33d13a40636687e85424118f3af7a204f797434d8178a38d6ecbf2f33609ff`.
All gates pass on macOS arm64/Bun 1.3.8:

| Mode | Batch+frame p95 range | Maximum mounted rows | Current RSS range |
| --- | --- | --- | --- |
| Unified | 4.73–6.86 ms | 33 | 217–269 MiB |
| Split | 4.72–6.53 ms | 23 | 267–314 MiB |

These measurements include event-ring updates and navigation, not agent execution,
Rust IPC parsing or physical terminal display latency. RSS is cumulative current
process memory across modes. The matched Ratatui/native rendering baseline remains
in `rendering-baseline.md`; real IPC lifecycle/stream queues are #307 gates.

## Verification and remaining #302 scope

Native tests exercise search apply/cancel/no-match/paste, previous/next wrapping,
headers and offscreen source rows in both modes, file/mode horizontal/vertical
restore, changed-content/run invalidation, anchor precedence and listener cleanup.
Storybook adds editing, no-match and split-match frames at ordinary/narrow sizes.
Root typecheck/test/lint/protocol and Archgate gates remain in force.
React Doctor reports one high-confidence false-positive DOM-property warning
for native `wrapMode` in `windowed-diff.test.tsx`; the installed OpenTUI types
and real renderer tests support that property. No rule suppression is added.

Syntax highlighting and clipboard parity remain #302 work, followed by the
supported physical-terminal image smoke described in `image-comparison.md`.
This increment does not close #302 or remove either old renderer.
