# Current Focus

This file is the live snapshot of any in-flight session. It is short on
purpose; the durable record lives in git, `_agent/WORKLOG.md`, and
`plan/done/`. Queued work lives in `plan/todo/`.

If status files and git disagree, git is authoritative; correct this file.

## Active state

- **Branch:** `issue-302-opentui-scalable-diff`.
- **Active item:** epic #294 step 08/15, issue #302 - scalable diff rendering.
- **Change:** split/unified diff modes with a toggle, viewport culling on the
  diff scrollbox, old/new line mapping preserved in both modes, and a large-diff
  benchmark fixture.
- **Pending gate:** windowed virtualization, image previews and browser-server
  extraction, so #302 stays open (PR uses `Refs`).
- **Plan items:** `.docflow/plan/todo/0023-opentui-scalable-diff.md`; moved
  `00{20,21}` to `plan/done/`.
- **Merged:** #301 in `cbc2808` (PR #316, `Refs #301`, benchmark gate pending).

## Next item

- Finish the #302 PR; remaining #302 work continues after.

## Epic queue

#295 -> #296/#297 -> #298 -> #299 -> #300 -> #301 -> #302/#303/#305 ->
#304 -> #306 -> #307 -> #308 -> #309 -> close #294.
