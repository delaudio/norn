# Current Focus

This file is the live snapshot of any in-flight session. It is short on
purpose; the durable record lives in git, `_agent/WORKLOG.md`, and
`plan/done/`. Queued work lives in `plan/todo/`.

If status files and git disagree, git is authoritative; correct this file.

## Active state

- **Branch:** `main` (uncommitted working tree).
- **Active item:** ship the OpenCode AI review provider (ADR 0018) in release
  0.3.1 across all surfaces. Authored
  `.docflow/adr/0018-opencode-ai-review-provider.md` and
  `.docflow/plan/todo/0015-opencode-ai-review-provider.md`; regenerated
  `.docflow/INDEX.md`. Bumped `package.json`, `src-tauri/tauri.conf.json`,
  `src-tauri/Cargo.toml`, and the locked `norn` entry in
  `src-tauri/Cargo.lock` to `0.3.1`; `pnpm run version:verify` reports the
  sources aligned at `0.3.1`. `v0.3.0` is already tagged.
- **Plan items:** `.docflow/plan/todo/0015-opencode-ai-review-provider.md` is
  active. `.docflow/plan/todo/0014-local-review-target-desktop.md` remains
  queued; `.docflow/plan/todo/0001` and `0004` remain queued.
- **Verification:** `pnpm run typecheck`, `pnpm run test` (frontend + tooling),
  `pnpm run test:tauri` (4 IPC smoke tests), `pnpm run lint`, all-target
  `cargo clippy -D warnings`, `cargo fmt --check`, the full Rust library suite
  (664 passed, 2 ignored), and Archgate 17/17 all pass on the current tree.
- **Not yet done:** the change is not committed; ADR 0018 stays `Accepted`
  until the work ships and the plan item moves to `plan/done/`.

## What changed

Norn now supports a third AI review provider, OpenCode, executed through the
installed `opencode` CLI. Provider selection, model, and effort flow through
config, the headless `norn review` CLI, desktop settings, and the terminal UI.
DeepSeek models (`deepseek/deepseek-flash`, `deepseek/deepseek-v4-pro`) are the
default catalog. Execution delivers the payload on stdin, constrains tools
through an inline runtime config for read-only review, parses the JSON event
stream, reuses the bounded/cancellable provider path, and reports through the
existing sanitized error taxonomy. Claude and Codex behavior is unchanged.

## Next item

- Commit and open the PR for ADR 0018 / plan item 0015, then move the plan item
  to `plan/done/` and advance ADR 0018 to `Implemented` on merge.
- Implement the desktop Local review target without changing the shipped TUI
  snapshot contract.
