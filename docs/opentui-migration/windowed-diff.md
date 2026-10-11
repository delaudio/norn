# Windowed terminal text diffs

Internal migration evidence for #302, plan 0040 and ADR 0020.

## Behavior and limits

Both modes mount only viewport rows plus eight logical rows on each side. Native
spacers preserve the complete visual height, including expanded large diffs.
Collapsed diffs retain the existing 2,000-row source window and expand action.
Split columns still word-wrap. Geometry uses OpenTUI TextBuffer/TextBufferView
with the renderer width method, not JavaScript string length. Native buffers are
destroyed after measurement; cached cumulative offsets enable logarithmic window
lookup and offscreen anchor positioning without mounting the target first.

The native frame/resize subscription observes keyboard, mouse and scrollbar
movement. It cleans up on unmount. Width changes recompute wrapping and restore
a finding anchor; a repeated scroll request recenters that anchor. File/target
identity resets an unanchored viewport. Unified horizontal extent includes the
widest source line even when that line is offscreen. Split content width stays
fixed to the viewport so hidden metadata cannot change column geometry.

This is a text rendering delivery, not completion of all #302 platform gates.
Physical Kitty/Sixel smoke and the matched Ratatui baseline remain explicit
before cutover. Selection/copy behavior across a moving window belongs to the
final interaction parity/hardening checks.

## Repeatable checks

- `bun test --cwd ui test/windowed-diff.test.tsx`: real native renderer, 10,000
  rows, first/last access and bounded mounts; offscreen old/new split anchors;
  Unicode, combining marks, tabs, odd widths and resize; actual text height fits
  the reserved row; full offscreen horizontal extent.
- `pnpm --dir ui frames:check`: 44 native scenes, including wide/narrow wrapped
  anchors. Capture waits for layout/anchor reconciliation rather than recording
  the provisional first layout. Storybook consumes the same frames.
- `pnpm --dir ui bench`: expanded 1,000/10,000-line fixtures, 100 scroll samples,
  mount and split repaint, maximum mounted rows and process-wide RSS. A missing
  scroll panel invalidates the benchmark. Scroll samples include viewport frame,
  React reconciliation and a second frame, plus a zero-delay task yield.

Provisional same-host budgets at 120x30 for 10,000 expanded lines are: split
repaint <=100 ms, scroll p95 <=20 ms, mounted rows <=64. Structural bounds are
asserted in native tests; timing is measured rather than a flaky CI time limit.
These budgets are not a claim that the outstanding Ratatui comparison passed.

## Same-host observations

2026-10-11, macOS arm64, Bun 1.3.8, OpenTUI 0.5.17, 120x30. Synthetic fake
transport, no provider calls or OS cold-start measurement. Process-wide RSS is
cumulative in a single benchmark process and must not be treated as an isolated
per-scene memory figure.

| Expanded lines | Mount | Split repaint | Scroll p95 (100 samples) | Maximum mounted rows | RSS |
|---|---:|---:|---:|---:|---:|
| 1,000 | 6.33 ms | 4.27 ms | 3.49 ms | 39 | 253 MiB |
| 10,000 | 6.74 ms | 15.16 ms | 5.25 ms | 39 | 312 MiB |

Running the same expanded harness on parent `699878b` gave 17.50 ms mount,
41.13 ms split repaint, 2.97 ms scroll p95 and 502 mounted split rows for 1,000
lines. Existing mounted nodes make that small-case scrolling faster; windowing
adds reconciliation while bounding memory and initial construction.

The parent failed on expanded 10,000 lines with `Failed to create TextBuffer`
and the diff panel disappeared. Its subsequent zero mounted-row and timing
readings are invalid, not performance successes. The updated harness now refuses
that state. An earlier collapsed-only comparison observed ~143 ms split repaint
and 396 MiB RSS; it is a different workload and not the expanded baseline.

No Norn review was run, per the maintainer's instruction. Native tests and frame
capture use synthetic inputs without HOME/config/keychain/provider access.
