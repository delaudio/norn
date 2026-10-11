# Review Engine Seam in the Stdio Backend

## Owning ADRs

- `../../adr/0020-opentui-rust-stdio-workspace.md`

## Scope

Introduce a review-engine seam in the stdio backend so `review.start` can run
the shared headless engine. `headless_review::run` (assembling the request from configuration) is the
**default**; `NORN_REVIEW_ENGINE=simulated` keeps the deterministic placeholder
and `fake` is a fast deterministic engine used by tests. Cancellation is honoured before start; a running agent is
not killed mid-run.

GitHub issue: #303 (epic #294 step 09/15).

Pending (tracked): making the real engine the default for the desktop/TUI,
progress streaming from the running agent, cooperative mid-run cancellation,
`review.reply`/`review.fix` IPC, the conversation composer and finding grouping.

## Exit Criteria

- `review.start` dispatches to the selected engine; the **real** engine is the
  default and uses the shared headless engine with no React-side agent shelling.
- A deterministic fake engine streams running → progress → succeeded and is
  covered by a subprocess test.
- `cargo clippy -D warnings`, `cargo test`, `pnpm run lint/typecheck/test`, the
  `ui` job and `archgate check` pass.

## Dependencies

- `../../adr/0020-opentui-rust-stdio-workspace.md`
- `../../adr/0008-shared-review-service-boundary.md`
