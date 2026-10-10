# OpenTUI Scalable Diff Rendering

## Owning ADRs

- `../../adr/0020-opentui-rust-stdio-workspace.md`

## Scope

Reach terminal diff parity for large reviews without freezing navigation. This
change delivers the text-diff increment: unified and split diff modes with a
mode toggle, viewport culling on the diff scrollbox so off-screen rows are not
rendered, large-diff collapse (diffs over 2,000 rows render a bounded window
centred on the highlighted region with an `e` expand toggle), old/new line
mapping preserved in both modes, and a large-diff performance fixture that
records mount and split-repaint cost.

GitHub issue: #302 (epic #294 step 08/15).

Still pending within #302 (tracked, not delivered here): full windowed
virtualization so React does not mount one node per line, native terminal image
preview/comparison with an explicit unsupported-terminal fallback, and
extracting the authenticated browser diff server under Rust ownership. This
change references #302 without closing it.

## Exit Criteria

- Unified and split modes render with correct old/new line numbers; findings and
  comments keep correct anchors in both.
- A mode toggle is exposed through the shell and documented.
- The diff scrollbox uses viewport culling; off-screen rows are not rendered.
- Diffs over 2,000 rows collapse to a bounded window centred on the highlight,
  with an `e` expand toggle and an expand hint.
- Large-diff benchmarks (1,000 and 10,000 lines) are recorded with commands.
- `pnpm run typecheck`, `pnpm run test`, `archgate check`, the `ui` job, Rust
  tests and `frames:check` pass.
- **Pending gate:** windowed virtualization, image previews and browser-server
  extraction. #302 stays open until these land.

## Dependencies

- `../../adr/0020-opentui-rust-stdio-workspace.md`
- `../../../docs/opentui-migration/migration-contract.md`
- `../../../docs/opentui-migration/vertical-slice.md`
