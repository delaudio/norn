# Implement the Persistent Rust Stdio Backend

## Owning ADRs

- `../../adr/0020-opentui-rust-stdio-workspace.md`

## Scope

Add the private `norn-backend` binary (`src-tauri/src/backend.rs`) that hosts the
Rust engine over the typed stdio protocol: initialize shared state once, do the
versioned handshake, dispatch typed requests, run long operations on their own
threads, serialize output through one bounded writer queue, and shut down
gracefully on `shutdown` or EOF. First-slice methods: `repository.status`,
`diff.file` (credential-free local Git, including untracked files),
`review.start` (operation lifecycle), `operation.status`, `operation.cancel`,
`shutdown`.

GitHub issue: #298 (epic #294 step 04/15). Review/agent execution is connected in
later steps (#301, #303); this item drives the snapshot and lifecycle.

Out of scope: OpenTUI widgets, an HTTP replacement API, FFI, or rewrites of
review execution logic.

## Exit Criteria

- A real subprocess test client handshakes, lists repositories, fetches a file
  diff (tracked and untracked), starts and cancels an operation, and shuts down.
- Concurrent requests correlate correctly; blocking work never stalls
  cancel/status/shutdown.
- Cancellation is honoured after the last file and for empty change sets; the
  operation never reports success after a cancel.
- Provider targets resolve only a matching configured repository; local targets
  use the backend working directory.
- EOF/crash/error paths release resources; output queue and frame sizes are
  bounded; standard output stays protocol-only.
- The headless CLI/service remain independent of the backend and JS runtime.
- `pnpm run typecheck`, `pnpm run test`, and `archgate check` pass.

## Dependencies

- `../../adr/0020-opentui-rust-stdio-workspace.md`
- `../../../docs/opentui-migration/migration-contract.md`
