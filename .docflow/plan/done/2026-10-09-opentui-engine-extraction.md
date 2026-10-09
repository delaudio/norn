# Extract a UI-Independent Rust Engine

## Owning ADRs

- `../../adr/0020-opentui-rust-stdio-workspace.md`

## Scope

Make the existing business services buildable and testable without Tauri or
Ratatui while preserving existing CLI, service, desktop, and terminal behavior
during the transition. Gate the Tauri desktop adapter behind an optional
`desktop` feature and the Ratatui terminal UI behind an optional `tui` feature,
so `--no-default-features` produces a Tauri- and Ratatui-free engine/headless
build. Keep Tauri command wrappers thin and the `*_native` operations as the
ordinary Rust API; do not duplicate provider or review implementations. Preserve
credential and user-storage locations and formats.

GitHub issue: #296 (epic #294 step 02/15).

Out of scope: a new transport, the OpenTUI UI rewrite, storage migration, or
deleting the old interfaces (those are later steps).

## Exit Criteria

- The engine and headless binaries compile with no Tauri, Ratatui, crossterm, or
  ratatui-image in their active dependency tree; `pnpm run test:rust:core`
  proves it and fails if the inspection itself fails.
- The default `desktop-bundle` build still compiles and runs the desktop GUI and
  the Ratatui terminal UI; `pnpm run test:tauri` and `pnpm run test:rust:cli`
  pass.
- Existing build scripts and the release workflow keep building all shipped
  distributions (CLI, TUI, desktop launcher) with the correct features.
- Credentials and user-storage locations/formats are unchanged.
- `pnpm run typecheck`, `pnpm run test`, and `archgate check` pass.

## Dependencies

- `../../adr/0020-opentui-rust-stdio-workspace.md`
- `../../../docs/opentui-migration/migration-contract.md`

Shipped through [PR #311](https://github.com/delaudio/norn/pull/311) (757709e).
