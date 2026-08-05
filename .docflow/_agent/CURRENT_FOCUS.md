# Current Focus

This file is the live snapshot of any in-flight session. It is short on
purpose; the durable record lives in git, `_agent/WORKLOG.md`, and
`plan/done/`. Queued work lives in `plan/todo/`.

If status files and git disagree, git is authoritative; correct this file.

## Active state

- **Branch:** `codex/norn-runtime-migration`
- **Active items:** GitHub issues #174, #175, and #176, tracked by
  `.docflow/plan/todo/0006-norn-runtime-identifiers.md` and
  `.docflow/plan/todo/0007-norn-local-data-migration.md`, and
  `.docflow/plan/todo/0008-norn-repository-config.md`.
- **Completed predecessor:** plan item `0005` and GitHub issue #173 shipped as
  commit `79a90ece` through PR #195.
- **Blockers:** none for the runtime migration; external DNS, Homebrew, and
  signing operations remain separate delivery prerequisites.
- **Branch-local work:** canonical Norn runtime and repository identifiers,
  bounded legacy aliases, recoverable local-data migration, and safe
  repository-config migration are in validation.

## Last shipped

`e149569` - compare image diffs side by side in the terminal UI through PR #148.

## Next item

Validate, review, and merge the Norn runtime, repository-config, and local-data
migration slice, then continue with the next open issue.
