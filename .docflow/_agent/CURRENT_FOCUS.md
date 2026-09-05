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
- **Verification:** 642 Rust library tests pass with 2 ignored, alongside 110 frontend
  tests plus tooling, all-target Clippy, typecheck, lint, Archgate 17/17, and an
  isolated Windows target compile check. Local snapshots share one end-to-end
  deadline, superseded TUI loads cancel their process trees, and every local
  review Git process uses the platform's trusted executable resolver. Preview
  fingerprints come from immutable Git index object IDs, and preview bytes are
  served from the matching bounded `git cat-file` object rather than read from
  the working tree. Images with unstaged content remain in the diff but omit
  their optional preview with an explicit warning. Local repository eligibility
  is computed off the render thread, fenced by stable repository identities and
  repository generation, and now shares one cancellable five-second Git
  deadline with bounded output. Norn branch reviews through
  `run-1788599625404235000` drove the trusted Git, executable-configuration,
  immutable-preview, and eligibility remediations. The next bounded review is
  pending. Structured eligibility errors remain a low-severity follow-up.
  Implementation commit `26b4ee6` and release metadata commit `8460524` are prepared, all
  version sources are aligned at `0.3.0`, and the release guard accepts
  candidate tag `v0.3.0`; branch publication and PR creation are pending the
  final pre-push Norn branch review.

## Last shipped

`1e08045` - release Norn v0.2.9 with the shared desktop/browser diff UI.

## Next item

- Implement the shared local snapshot and terminal Local review target.
