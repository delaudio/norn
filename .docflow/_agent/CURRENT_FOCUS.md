# Current Focus

This file is the live snapshot of any in-flight session. It is short on
purpose; the durable record lives in git, `_agent/WORKLOG.md`, and
`plan/done/`. Queued work lives in `plan/todo/`.

If status files and git disagree, git is authoritative; correct this file.

## Active state

- **Branch:** `issue-301-opentui-vertical-slice` (to be created).
- **Active item:** epic #294 step 07/15, issue #301 - deliver and measure the
  real file -> diff -> finding vertical slice.
- **Change:** protocol gains `review.files` and `review.findings` (schema,
  generated TypeScript, Rust `protocol.rs`/`backend.rs`). The OpenTUI shell lists
  changed files, lazily loads the selected diff with stale-response fencing, and
  navigates findings with anchor resolution plus a missing-anchor notice.
  `ui/scripts/bench.ts` measures the OpenTUI slice; method and numbers live in
  `docs/opentui-migration/vertical-slice.md`.
- **Pending gate:** the Ratatui-vs-OpenTUI comparison and contract thresholds are
  not produced yet, so #301 stays open (PR uses `Refs`, not `Fixes`).
- **Plan items:** `.docflow/plan/todo/0022-opentui-vertical-slice.md` (this item)
  and `plan/todo/00{20,21}-opentui-{shell,frames}.md` (shipped in #300).

## Next item

- Finish the #301 PR; then #302/#303/#305.

## Epic queue

#295 -> #296/#297 -> #298 -> #299 -> #300 -> #301 -> #302/#303/#305 ->
#304 -> #306 -> #307 -> #308 -> #309 -> close #294.
