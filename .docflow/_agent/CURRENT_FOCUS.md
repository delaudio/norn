# Current Focus

This file is the live snapshot of any in-flight session. It is short on
purpose; the durable record lives in git, `_agent/WORKLOG.md`, and
`plan/done/`. Queued work lives in `plan/todo/`.

If status files and git disagree, git is authoritative; correct this file.

## Active state

- **Branch:** `issue-296-engine-extraction` (to be created).
- **Active item:** epic #294 step 02/15, issue #296 - extract a UI-independent
  Rust engine and isolate Tauri adapters.
- **Change:** `src-tauri/Cargo.toml` now gates the Tauri adapter behind the
  optional `desktop` feature and the Ratatui terminal UI behind the optional
  `tui` feature; `desktop-bundle` enables both. `build.rs`, `lib.rs`, `main.rs`,
  the Tauri-only binaries, and all `#[tauri::command]` wrappers are gated, so a
  `--no-default-features` build compiles the engine and headless commands with
  no Tauri, Ratatui, crossterm, or ratatui-image in the active dependency tree.
  `pnpm run test:rust:core` proves it; the CI `verify` lane and the release gate
  run it. Build scripts and the macOS release archive now pass
  `--features custom-protocol,tui` so every shipped distribution still builds.
- **Plan items:** `.docflow/plan/todo/0017-opentui-engine-extraction.md` (this
  item); `.docflow/plan/done/2026-10-09-opentui-rust-architecture.md` (#295).

## Next item

- After #296 merges, proceed to #297 (define and generate the versioned typed
  stdio protocol), which is blocked by #295 and independent of #298.

## Epic queue

#295 -> #296/#297 -> #298 -> #299 -> #300 -> #301 -> #302/#303/#305 ->
#304 -> #306 -> #307 -> #308 -> #309 -> close #294.
