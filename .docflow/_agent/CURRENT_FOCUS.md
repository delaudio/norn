# Current Focus

Git is authoritative. Durable history lives in WORKLOG and plan/done.

## Active state — 2026-10-14

Single writer; no delegation or Norn review per maintainer.
PR #394 remains open on fix/current-repo-first in
/private/tmp/norn-current-repo-first. Plan 0074 remains pending.
Merged current origin/main (including #395), preserving both conflicting tests.
Repairing Windows repository-operation reconciliation: complete post-write
snapshot uses the same bounded 20-second budget as preview instead of five.
Unverified state remains uncertain; failed-fetch fixture also asserts actual HEAD.
Local UI 318 tests, root 127 tests/tooling, six focused Rust tests pass.
Exact-head three-platform CI and actual merge are required before shipping.
Next: #308 matched installable artifacts, then #309 Tauri/Ratatui cutover.
Do not publish releases or touch stored user data. Keep ADR0020 Accepted until
full cutover. Preserve root untracked .norn.yaml and user stash.
