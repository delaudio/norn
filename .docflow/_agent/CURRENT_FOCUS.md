# Current Focus

This file is the live snapshot of any in-flight session. It is short on
purpose; the durable record lives in git, `_agent/WORKLOG.md`, and
`plan/done/`. Queued work lives in `plan/todo/`.

If status files and git disagree, git is authoritative; correct this file.

## Active state

- **Branch:** `docs/readme-refresh`
- **Active item:** GitHub issue #179 (Lachesi-to-Norn compatibility and upgrade
  regression coverage) by hardening `norn doctor` legacy-name gating.
- **Completed predecessor:** plan items `0006`, `0007`, and `0008` for issues
  #174, #176, and #175 are shipped via PR #196 (`36353fb`), and recorded in
  `plan/done` with the same commit.
- **Additional completed predecessor:** issue #177 is now closed and includes
  user-facing docs/website/storybook UI rename completion in commit
  `2de66ed`.
- **Additional completed predecessor:** issue #179 is now functionally covered by
  `norn doctor` legacy-name gates and tests in this branch.
- **Blockers:** external redirect/rename validation for issue #178 still depends on
  GitHub, DNS, and CDN ownership changes outside this repository.
- **Branch-local work:** issue #179 adds repository scanning for legacy-name
  occurrences and explicit allowlisting of intentional migration artifacts.

## Last shipped

`2de66ed` - rename user-facing docs/UI/website naming to Norn.

## Next item

Issue #178 remains open for external infrastructure/redirect validation, then
issues #181–#185 for Homebrew/release slices.
