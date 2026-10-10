# Browser Diff Server Under Rust Ownership

## Owning ADRs

- `../../adr/0020-opentui-rust-stdio-workspace.md`

## Scope

Retain the authenticated browser diff viewer under Rust ownership and expose
browser-open through the typed backend. The server lives in a shared,
always-compiled `browser_diff` module, and the stdio backend adds a
`browser.open` method that builds the viewer state offline from the selected
target (reviewed base/head for stored reviews, or the working tree), starts the
session server, returns its URL, and opens the OS browser (disabled by
`NORN_BROWSER_OPEN=0`). Loopback binding, per-route unpredictable session
authentication, MIME allowlisting, target-identity caching, bounded retries and
security headers are preserved; the packaged-asset file cap is raised because
the web viewer now emits one lazy chunk per Shiki language.

GitHub issue: #302 (epic #294 step 08/15).

The `browser.open` request and the shell `b` action surface the session URL in
the notice line.

## Exit Criteria

- `browser_diff` compiles in the headless (`--no-default-features`) engine build
  and does not pull Tauri or Ratatui (`pnpm run test:rust:core`).
- `browser.open` returns a session URL of the form
  `http://127.0.0.1:<port>/session/<64 hex>/` and does not open a browser under
  `NORN_BROWSER_OPEN=0`.
- The existing browser-server security tests pass.
- The TUI imports the server from its new home; no behavior change.
- `pnpm run typecheck`, `pnpm run test`, and `archgate check` pass.

## Dependencies

- `../../adr/0020-opentui-rust-stdio-workspace.md`
- `../../../docs/opentui-migration/migration-contract.md`
