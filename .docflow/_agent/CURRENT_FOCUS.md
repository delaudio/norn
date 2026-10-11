# Current Focus

This file is the live snapshot of any in-flight session. It is short on
purpose; the durable record lives in git, `_agent/WORKLOG.md`, and
`plan/done/`. Queued work lives in `plan/todo/`.

If status files and git disagree, git is authoritative; correct this file.

## Active state

- **Branch:** `issue-303-review-fix-approval` (prepared after PR #334).
- **Active item:** complete epic #294 through verified PRs and squash merges.
- **Current change:** review.fix preview, snapshot-bound approval and native
  cancellation; plan 0033. PR #334 merged at `043dcf1` after both CI gates passed.
- **Review instruction:** the maintainer explicitly requested no Norn reviews
  for this migration session; use code inspection and deterministic checks.
- **Merged baseline:** main at `043dcf1`, including PRs #316–#334.
- **Remaining:** review.fix approval/IPC and commit/push/conflict/sync parity (#303); remaining image/split/performance parity (#302); settings
  and data migration (#305); staged publication (#304); parity/hardening/
  packaging/cutover (#306–#309), then close the epic with merge evidence.

## Next item

- Merge the approved-fix delivery after all checks pass, then complete
  remaining native mutation operations for #303. Preserve `.norn.yaml` as unrelated user data.

## Epic queue

#295 -> #296/#297 -> #298 -> #299 -> #300 -> #301 -> #302/#303/#305 ->
#304 -> #306 -> #307 -> #308 -> #309 -> close #294.
