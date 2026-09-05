# Current Focus

This file is the live snapshot of any in-flight session. It is short on
purpose; the durable record lives in git, `_agent/WORKLOG.md`, and
`plan/done/`. Queued work lives in `plan/todo/`.

If status files and git disagree, git is authoritative; correct this file.

## Active state

- **Branch:** `feat/local-review-tui`.
- **Active item:** `.docflow/plan/todo/0013-local-review-target-tui.md`.
- **Plan items:** implement the shared local snapshot and TUI first, then
  `.docflow/plan/todo/0014-local-review-target-desktop.md`.
- **Verification:** 642 Rust tests pass with 2 ignored, alongside 110 frontend
  tests plus tooling, all-target Clippy, typecheck, lint, Archgate 17/17, and an
  isolated Windows target compile check. Local snapshots share one end-to-end
  deadline, superseded TUI loads cancel their process trees, and Windows Git
  starts suspended before Job Object assignment. Preview fingerprints now use
  bounded, killable Git object-ID processes with in-memory SHA-1/SHA-256
  verification; recoverable preview failures remain warnings. Local repository
  eligibility is computed off the render thread with single-flight, two-second
  invalidation. Eligibility results now carry stable repository identities and
  a repository-list generation, stale work is discarded, and the TUI renders
  the initial eligibility check as loading. Timing regressions cover the shared
  Git deadline and complete HTTP response reads. All repository gates pass; the
  working tree remains uncommitted. Norn review
  `run-1788528449536161000` confirmed the TUI generation/loading fixes and
  reported two new follow-ups in local repository eligibility. The high finding
  is now remediated: Windows Git discovery uses OS Known Folders, accepts only
  canonical executable paths contained by Program Files or Local AppData, and
  never falls back to ambient `PATH`; a Windows-target compile check and
  symlink-escape regressions pass. Norn review
  `run-1788535418566547000` returned exit 0 with no high-severity findings. It
  retained three low-severity follow-ups about bounded/cancellable eligibility,
  structured eligibility errors, and event-driven invalidation; those remain
  intentionally out of scope for this pass. Implementation commit `26b4ee6`
  is prepared, all version sources are aligned at `0.3.0`, and the release
  guard accepts candidate tag `v0.3.0`; branch publication and PR creation are
  pending the pre-push Norn branch review.

## Last shipped

`1e08045` - release Norn v0.2.9 with the shared desktop/browser diff UI.

## Next item

- Implement the shared local snapshot and terminal Local review target.
