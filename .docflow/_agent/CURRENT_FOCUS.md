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
  deadline, superseded TUI loads cancel their process trees, and every local
  review Git process uses the platform's trusted executable resolver. Preview
  fingerprints are calculated from bounded bytes read through an already-open,
  capability-protected file handle and verified in memory with the repository's
  SHA-1 or SHA-256 object format. Local repository eligibility is computed off
  the render thread and fenced by stable repository identities plus repository
  generation. Norn branch review `run-1788595690018618000` identified three
  release-blocking findings; commit `4cc7318` resolves all three. The bounded
  remediation review is pending. Three earlier low-severity follow-ups about
  bounded/cancellable eligibility, structured eligibility errors, and
  event-driven invalidation remain intentionally out of scope. Implementation
  commit `26b4ee6` and release metadata commit `8460524` are prepared, all
  version sources are aligned at `0.3.0`, and the release guard accepts
  candidate tag `v0.3.0`; branch publication and PR creation are pending the
  final pre-push Norn branch review.

## Last shipped

`1e08045` - release Norn v0.2.9 with the shared desktop/browser diff UI.

## Next item

- Implement the shared local snapshot and terminal Local review target.
