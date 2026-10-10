# Browser Diff Server Under Rust Ownership

## Owning ADRs

- `../../adr/0020-opentui-rust-stdio-workspace.md`

## Scope

Retain the authenticated browser diff viewer under Rust ownership independent of
the terminal UI, and (follow-up) expose browser-open through the typed backend.
This change extracts the server from the feature-gated `tui` module into a
shared, always-compiled `browser_diff` module so the headless backend can reuse
it. Loopback binding, per-route unpredictable session authentication, MIME
allowlisting, target-identity caching, bounded retries and security headers are
preserved unchanged.

GitHub issue: #302 (epic #294 step 08/15).

Pending (tracked): exposing browser-open through the typed backend and a shell
action. This change does not add IPC or open a browser.

## Exit Criteria

- `browser_diff` compiles in the headless (`--no-default-features`) engine build
  and does not pull Tauri or Ratatui (`pnpm run test:rust:core`).
- The existing browser-server security tests pass unchanged.
- The TUI imports the server from its new home; no behavior change.
- `pnpm run typecheck`, `pnpm run test`, and `archgate check` pass.

## Dependencies

- `../../adr/0020-opentui-rust-stdio-workspace.md`
- `../../../docs/opentui-migration/migration-contract.md`
