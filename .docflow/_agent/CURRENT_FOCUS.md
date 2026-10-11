# Current Focus

This file is the live snapshot of any in-flight session. It is short on
purpose; the durable record lives in git, `_agent/WORKLOG.md`, and
`plan/done/`. Queued work lives in `plan/todo/`.

If status files and git disagree, git is authoritative; correct this file.

## Active state

- **Branch:** `issue-302-diff-navigation-streaming` (isolated preparation after PR #344).
- **Active item:** complete epic #294 through verified PRs and squash merges.
- **Current change:** plan 0043, native diff navigation and streaming evidence; plan 0042 is PR #344, CI pending; plan 0041 / PR #343 merged at `ccd133d`, both CI gates green. PR #342 merged at `2a2ed72` with both CI gates green. PR #341 merged with both CI gates green. PR #340 merged at `72440ea` with both CI gates green. PR #339 merged at `76ba213` with both CI gates green. PR #338 merged at `3168dcd` with all checks green.
- **Review instruction:** the maintainer explicitly requested no Norn reviews
  for this migration session; use code inspection and deterministic checks.
- **Merged baseline:** main at `ccd133d`, including PRs #316–#343.
- **Remaining:** follow-up commit/push/conflict/sync parity (#306/#307; #303 already closed on GitHub); remaining image/split/performance parity (#302); settings
  and data migration (#305); staged publication (#304); parity/hardening/
  packaging/cutover (#306–#309), then close the epic with merge evidence.

## Next item

- Merge canonical workspace records after CI, then complete staged publication and review/data workflow parity. Preserve `.norn.yaml` as unrelated user data.

## Epic queue

#295 -> #296/#297 -> #298 -> #299 -> #300 -> #301 -> #302/#303/#305 ->
#304 -> #306 -> #307 -> #308 -> #309 -> close #294.
