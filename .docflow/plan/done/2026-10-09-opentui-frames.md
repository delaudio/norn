# Native Frames and Storybook Visual Regression

## Owning ADRs

- `../../adr/0020-opentui-rust-stdio-workspace.md`

## Scope

Make terminal components reviewable in Storybook and verifiable with
deterministic screenshots produced by the real OpenTUI renderer. Add
`ui/scripts/render-frames.tsx` (captures cell spans, colors and attributes into
bounded frame fixtures), `ui/storybook/{frame.ts,TerminalPreview.tsx,preview.css}`
and stories for the shell states at 50x15, 80x24 and 120x30, plus a
`.storybook/` configuration. Add the golden-frame visual lane
(`bun run frames:check`) that regenerates frames to a temporary file, compares
them with the committed baseline, and fails with inspectable artifacts.

GitHub issue: #300 (epic #294 step 06/15).

Out of scope: an external snapshot SaaS, real user data or credentials in
stories, and a live OpenTUI runtime in the browser.

## Exit Criteria

- A layout/color regression fails `bun run frames:check` and writes
  `ui/storybook/visual-artifacts/{expected,current,diff}`.
- A clean checkout can generate frames and build Storybook with documented
  commands (`bun run frames:generate`, `bun run storybook`,
  `bun run build-storybook`).
- Baseline updates require explicit intent (`frames:generate` + commit); normal
  test runs never accept new frames automatically.
- The preview handles Unicode, wide characters and styled spans and is a frame
  viewer, not a pixel-identity claim across terminal emulators.
- The CI `ui` job runs `frames:check`; keyboard/scroll assertions stay in the
  OpenTUI tests.
- `pnpm run typecheck`, `pnpm run test`, and `archgate check` pass.

## Dependencies

- `../../adr/0020-opentui-rust-stdio-workspace.md`
- `../../../docs/opentui-migration/migration-contract.md`

---

Shipped in `a47ab88` (PR #315, issue #300).
