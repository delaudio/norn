# Current Focus

This file is the live snapshot of any in-flight session. It is short on
purpose; the durable record lives in git, `_agent/WORKLOG.md`, and
`plan/done/`. Queued work lives in `plan/todo/`.

If status files and git disagree, git is authoritative; correct this file.

## Active state

- **Branch:** `docs/readme-refresh`
- **Active item:** closing the Homebrew/Ecosystem ship chain by wrapping up issues
  #170–#185 and validating branch-local release/readiness artifacts.
- **Completed predecessor:** plan items `0006`, `0007`, and `0008` for issues
  #174, #176, and #175 are shipped via PR #196 (`36353fb`) and recorded in
  `plan/done`.
- **Additional completed predecessors:**
  - issue #177 for user-facing docs/website/storybook UI rename completion in
    `2de66ed`.
  - issue #179 by hardening `norn doctor` with legacy-name gates and repository
    allowlist tests in `fcfc508`.
  - Homebrew/Cask release distribution and lifecycle scaffolding now added under
    `homebrew-tap/`, `.github/workflows/`, and `scripts/`.
- **Current status:** issues #170, #171, #182, #183, #184, and #185 are now closed.

## Last shipped

`fcfc508` - harden `norn doctor` compatibility checks for legacy-name migration
artifacts.

## Next item

- Continue from the current branch with next-priority open work (`#169`) once
  this closeout is accepted.
