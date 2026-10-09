# Current Focus

This file is the live snapshot of any in-flight session. It is short on
purpose; the durable record lives in git, `_agent/WORKLOG.md`, and
`plan/done/`. Queued work lives in `plan/todo/`.

If status files and git disagree, git is authoritative; correct this file.

## Active state

- **Branch:** `issue-300-opentui-frames` (to be created).
- **Active item:** epic #294 step 06/15, issue #300 - native OpenTUI frame
  previews and Storybook visual regression.
- **Change:** `ui/scripts/render-frames.tsx` captures native OpenTUI frames
  (spans, colors, attributes) into `ui/storybook/generated/frames.json`;
  `ui/scripts/check-frames.ts` is the golden-frame visual lane (regenerate to a
  temp file, compare to the committed baseline, fail with
  `storybook/visual-artifacts/{expected,current,diff}`). `ui/storybook/`
  (`frame.ts`, `TerminalPreview.tsx`, `preview.css`, `stories/Shell.stories.tsx`)
  and `ui/.storybook/` render the frames in Storybook. Storybook deps were added
  to the `ui` package; the CI `ui` job now runs `frames:check`; task-runner
  recipes `ui-frames-check` added.
- **Plan items:** `.docflow/plan/todo/0021-opentui-frames.md` (this item);
  `plan/done/2026-10-09-opentui-{rust-architecture,engine-extraction,protocol,backend}.md`.

## Next item

- After #300 merges, proceed to #301 (real file -> diff -> finding vertical
  slice, measured).

## Epic queue

#295 -> #296/#297 -> #298 -> #299 -> #300 -> #301 -> #302/#303/#305 ->
#304 -> #306 -> #307 -> #308 -> #309 -> close #294.
