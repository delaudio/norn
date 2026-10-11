# Current Focus

This file is the live snapshot of any in-flight session. It is short on
purpose; the durable record lives in git, `_agent/WORKLOG.md`, and
`plan/done/`. Queued work lives in `plan/todo/`.

If status files and git disagree, git is authoritative; correct this file.

## Active state

- **Branch:** `issue-303-operation-resync` (PR #338).
- **Active item:** complete epic #294 through verified PRs and squash merges.
- **Current change:** plan 0036, operation identity and saved review resync; PR #338 awaits CI; PR #337 merged at `2f9b58e`. Previous delivery: approved commit and push controls for native fix sessions;
  plan 0034, prepared for merge in PR #336. PR #335 merged at `6367fa0` after both CI gates passed.
- **Review instruction:** the maintainer explicitly requested no Norn reviews
  for this migration session; use code inspection and deterministic checks.
- **Merged baseline:** main at `2f9b58e`, including PRs #316–#337.
- **Remaining:** commit/push/conflict/sync parity (#303); remaining image/split/performance parity (#302); settings
  and data migration (#305); staged publication (#304); parity/hardening/
  packaging/cutover (#306–#309), then close the epic with merge evidence.

## Next item

- Merge PR #338 after all checks pass, then deliver settings/onboarding and desktop-local data recovery for #305. Preserve `.norn.yaml` as unrelated user data.

## Epic queue

#295 -> #296/#297 -> #298 -> #299 -> #300 -> #301 -> #302/#303/#305 ->
#304 -> #306 -> #307 -> #308 -> #309 -> close #294.
