# Native terminal frame previews

Every migrated terminal component and shell state must have a Storybook story
and a captured native frame before its migration PR merges. Stories render
frames produced by the real OpenTUI renderer; they are frame viewers, not a live
OpenTUI runtime.

## Convention

- Add a scene in `scripts/render-frames.tsx` for each new component or state, at
  **50x15, 80x24 and 120x30** where the layout changes with size.
- Provide a story in `storybook/stories/*.stories.tsx` referencing the frame id.
- Use fixed fixtures only. Never read `$HOME`, repository config, credentials or
  the network.
- Run `bun run frames:generate` to refresh `storybook/generated/frames.json`, and
  commit it as the visual baseline.

## Commands

- `bun run frames:generate` - render native frames to the committed baseline.
- `bun run frames:check` - regenerate and compare against the baseline; fails and
  writes `storybook/visual-artifacts/{expected,current,diff}` on regression.
- `bun run storybook` - Storybook dev server on port 6007.
- `bun run build-storybook` - regenerate frames and build the static Storybook.

Baseline updates require explicit intent: run `frames:generate` and commit the
result. Normal test runs never accept new frames automatically.
