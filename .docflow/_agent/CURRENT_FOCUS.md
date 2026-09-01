# Current Focus

This file is the live snapshot of any in-flight session. It is short on
purpose; the durable record lives in git, `_agent/WORKLOG.md`, and
`plan/done/`. Queued work lives in `plan/todo/`.

If status files and git disagree, git is authoritative; correct this file.

## Active state

- **Branch:** `fix/agent-review-permissions`.
- **Active item:** `.docflow/plan/todo/0006-agent-review-permission-handoff.md`.
- **Plan items:** add explicit headless diff-sharing authorization, proactive
  Codex and Claude Code host-permission guidance, restricted-host diagnostics,
  and bounded provider execution.
- **Verification:** typecheck, 104 frontend tests plus tooling suites, 17/17
  Archgate checks, Clippy, 4 Tauri IPC tests, skill validation, 101 focused Rust
  tests, and fail-fast consent/sandbox CLI probes pass. The complete Rust suite
  reaches the pre-existing `doctor` provider-version probes, which remain
  unbounded and hang under concurrent execution. The implementation review
  found one valid setup-state bug, fixed before its bounded rerun. The pre-push
  review then found that restricted-host detection followed the consent gate;
  host detection now runs first, with a focused regression and runtime CLI
  probe passing. The bounded pre-push rerun is required before publication.

## Last shipped

`3b78900` - release Norn v0.2.6 after the TUI settings refinement.

## Next item

- `.docflow/plan/todo/0001-agentic-code-policy-pack.md` is the lowest-numbered
  remaining queue item.
