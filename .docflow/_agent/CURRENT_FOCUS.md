# Current Focus

This file is the live snapshot of any in-flight session. It is short on
purpose; the durable record lives in git, `_agent/WORKLOG.md`, and
`plan/done/`. Queued work lives in `plan/todo/`.

If status files and git disagree, git is authoritative; correct this file.

## Active state

- **Branch:** `main`.
- **Active item:** publish the prepared command-only `v0.3.0` release.
- **Plan items:** `.docflow/plan/todo/0014-local-review-target-desktop.md`
  remains queued because the desktop acceptance criterion is not part of the
  terminal-first release.
- **Verification:** 625 Rust library tests pass with 2 ignored and 32
  keychain-dependent tests filtered after the macOS Keychain blocked the full
  local run; all 17 affected Git, retention, migration, and active-session
  regressions pass. This is alongside 110 frontend tests plus tooling,
  all-target Clippy, typecheck, lint, and Archgate 17/17.
  The local Windows cross-target check reaches native dependency compilation
  but currently lacks the MinGW C compiler. Local snapshots share one end-to-end
  deadline, superseded TUI loads cancel their process trees, and every local
  review Git process uses the platform's trusted executable resolver. Preview
  fingerprints come from immutable Git index object IDs, and preview bytes are
  served from the matching bounded `git cat-file` object rather than read from
  the working tree. Images with unstaged content remain in the diff but omit
  their optional preview with an explicit warning. Local repository eligibility
  is computed off the render thread, fenced by stable repository identities and
  repository generation, and now shares one cancellable five-second Git
  deadline with bounded output. Git commands bind their canonical configured
  repository as the worktree, and an empty Local eligibility result settles the
  TUI out of its loading state. Git input is written by a supervised worker, so
  blocked writes remain subject to process-tree cancellation and the shared
  deadline. Eligibility failures are propagated without accepting partial
  repository lists, and trusted Unix Git discovery is restricted to exact
  validated system, Homebrew, MacPorts, Linuxbrew, and Nix entrypoints. Review
  persistence now records the target kind explicitly, and local retention
  preserves stores referenced by running reviews before pruning inactive
  snapshots. Repository changes now clear all previous local review state
  before asynchronous loading begins, and untracked-file detection terminates
  its contained Git process immediately after the first output byte. Norn
  snapshots also ignore global and system Git configuration and neutralize
  every repository-defined clean/process filter before any working-tree
  comparison, including required and long-running process drivers. Local review
  snapshots now preserve base-to-index and index-to-worktree layers separately,
  expose their file counts in the TUI, and bind both layer identities into the
  snapshot hash. Canceled eligibility requests are invalidated before leaving
  Local mode. Norn branch reviews through `run-1788699087691905000` drove these
  remediations. Two final pre-push attempts timed out at the configured AI
  provider boundary; the maintainer explicitly authorised publication without
  a completed Norn gate. PR #247 passed its `verify` workflow and was squash
  merged as `64b9f36`. All version sources are aligned at `0.3.0`, and the
  release guard accepts candidate tag `v0.3.0`. `HOMEBREW_TAP_TOKEN` is
  configured, the desktop release variable is absent, and the release workflow
  will therefore skip Apple signing, notarisation, DMG, and cask jobs.

## Last shipped

`64b9f36` - ship the terminal-first Local review target through PR #247.

## Next item

- Implement the desktop Local review target without changing the shipped TUI
  snapshot contract.
