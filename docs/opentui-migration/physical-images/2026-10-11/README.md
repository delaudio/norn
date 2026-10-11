# Observed native image smoke

Observed 2026-10-11 on macOS 26.2, iTerm2 3.6.11, Bun 1.3.8,
OpenTUI 0.5.17. Only newly created owned terminal windows were controlled or
captured. Existing sessions were not inspected, changed or captured. The demo
used an empty isolated environment/HOME, synthetic 2×2 PNGs and FakeBackendTransport;
no Rust backend, provider, agent, keychain or application data was accessed.

| Observation | Evidence | Result |
| --- | --- | --- |
| Wide, 120×30 cells | `wide.png` | Brown BEFORE and blue AFTER physically visible, side by side; Sixel caption. |
| Narrow, 50×20 cells | `narrow.png` | Both colors visible, stacked; no stale old layout. |
| Return to 120×30 | `wide-after-resize.png` | Both colors visible again in separate panes. |
| Explicit `q` | `quit-restored.png` | Exit code 0; `stty -g` before/after identical; normal screen displays the synthetic shell receipt. |

`capabilities.json` is the actual terminal report, not an image-display assertion.
Both Kitty and Sixel report supported. Initially the app image occupied one
cell; the component now fills its allocated viewport. Automatic Kitty then
produced blank physical placements on this exact iTerm/OpenTUI combination.
The same fixture forced to Sixel displayed both colors. Automatic selection now
prefers Sixel for iTerm with a pixel resolution; without Sixel/resolution it
uses labeled block previews. Other terminals retain SDK selection; explicit
protocol overrides remain honored outside tmux. Tmux retains safe block previews.

The screenshot shows a remaining Retina pixel-scale limitation: Sixel colors
occupy roughly half the allocated image rectangle in each axis, rather than
filling it. Image contents, aspect and before/after identity are visible; this
is **not** evidence of pixel-perfect scaling. Track the resolution conversion
in the hardening step, and retain the authenticated browser comparison for
precise image review. No physical Kitty, Windows or Linux result is claimed.
All seven owned test windows were closed after recording the result.

Reproduce with `bun run ui/scripts/image-terminal-smoke.ts`; optional
`--report NEW_PATH` writes only terminal dimensions/capabilities/resolution and
refuses to overwrite an existing file. The committed PNGs contain synthetic
content only. The decorative key icon is terminal chrome, not a credential.
