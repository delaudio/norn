# Current Focus

This file is the live snapshot of any in-flight session. It is short on
purpose; the durable record lives in git, `_agent/WORKLOG.md`, and
`plan/done/`. Queued work lives in `plan/todo/`.

If status files and git disagree, git is authoritative; correct this file.

## Active state

- **Branch:** `issue-304-persistent-drafts` (independent preparation after PR #350).
- **Active item:** complete epic #294 through verified PRs and squash merges.
- **Current change:** plan 0049, persistent local draft editor verified locally; plan 0048 / PR #350 merged at `bf7c15c`, UI/verify green; plan 0047 / PR #349 merged at `da0b87f`, all gates including three-platform archive build/copy green; plan 0046 / PR #348 merged at `e9d3020`, UI/verify green; #302 closed; plan 0045 / PR #347 merged at `58247d3`, UI/verify green; plan 0044 / PR #346 merged at `066174e`, UI/verify green; plan 0043 / PR #345 merged at `565d081`, UI/verify green; plan 0042 / PR #344 merged at `5eb33fd`, UI/verify green; plan 0041 / PR #343 merged at `ccd133d`, both CI gates green. PR #342 merged at `2a2ed72` with both CI gates green. PR #341 merged with both CI gates green. PR #340 merged at `72440ea` with both CI gates green. PR #339 merged at `76ba213` with both CI gates green. PR #338 merged at `3168dcd` with all checks green.
- **Review instruction:** the maintainer explicitly requested no Norn reviews
  for this migration session; use code inspection and deterministic checks.
- **Merged baseline:** main at `bf7c15c`, including PRs #316–#350.
- **Remaining:** follow-up commit/push/conflict/sync parity (#306/#307; #303 already closed on GitHub); image/split/performance issue #302 closed; settings
  and data migration (#305); staged publication (#304); parity/hardening/
  packaging/cutover (#306–#309), then close the epic with merge evidence.

## Next item

- Merge verified canonical reference PR; implement persistent draft editing, then authoritative finding staging and approved publication/reconciliation (#304), completing draft consumption for #305. Physical Sixel wide/narrow/quit observed; Retina scaling remains hardening work. Preserve `.norn.yaml` as unrelated user data.

## Epic queue

#295 -> #296/#297 -> #298 -> #299 -> #300 -> #301 -> #302/#303/#305 ->
#304 -> #306 -> #307 -> #308 -> #309 -> close #294.
