# Current Focus

Git is authoritative. Durable history lives in WORKLOG and plan/done.

## Active state — 2026-10-14

Single writer, no delegation or Norn review. Default headless/OpenTUI transition
shipped in PR #397, squash 416610fe42c7b4b7c0453bbcb04b126f9c528c78.
Exact source b312fe9 passed main CI, UI, Linux/macOS/Windows installed and native
workflows, legacy adapters and release contract. Plan 0076 is reconciled to done
only after that actual merge; this branch contains the documentation receipt.

Ordinary Cargo defaults are empty. Dev/build/install use Rust CLI/backend,
standalone OpenTUI and browser diff. Tauri/Ratatui remain explicit legacy
features with separate CI. Final adapter deletion and complete upgrade proof
remain later migration work; keep ADR0020 Accepted until full cutover.

Hardening #396/plan0075 remains paused as a draft on fix/opentui-hardening;
its three-file followup is in an owned stash. Do not merge it over current main
without rebasing and resolving its remaining Windows fixture failures. Plan0074
was already merged in #394; its receipt is prepared in that paused branch.

Do not publish releases, read the root untracked .norn.yaml or modify stored user
data. Preserve the user's original stash and the owned hardening followup.
Pre-existing audit coverage/coordination findings are recorded in plan/done.
