# Current Focus

This file is the live snapshot of any in-flight session. It is short on
purpose; the durable record lives in git, `_agent/WORKLOG.md`, and
`plan/done/`. Queued work lives in `plan/todo/`.

If status files and git disagree, git is authoritative; correct this file.

## Active state

- **Branch:** `issue-303-review-reply-ipc` (PR #333).
- **Active item:** complete epic #294 through verified PRs and squash merges.
- **Current change:** complete review.reply and the reply composer, including
  bracketed paste and failed-draft preservation; prepare plan 0031 completion.
- **Review instruction:** the maintainer explicitly requested no Norn reviews
  for this migration session; use code inspection and deterministic checks.
- **Merged baseline:** main at `5b82ac6`, including PRs #316–#332.
- **Remaining:** review.fix approval/IPC, finding filters/grouping and thread
  selection (#303); remaining image/split/performance parity (#302); settings
  and data migration (#305); staged publication (#304); parity/hardening/
  packaging/cutover (#306–#309), then close the epic with merge evidence.

## Next item

- Merge #333 after the updated checks pass, then implement the remaining #303
  operations and UI navigation. Preserve `.norn.yaml` as unrelated user data.

## Epic queue

#295 -> #296/#297 -> #298 -> #299 -> #300 -> #301 -> #302/#303/#305 ->
#304 -> #306 -> #307 -> #308 -> #309 -> close #294.
