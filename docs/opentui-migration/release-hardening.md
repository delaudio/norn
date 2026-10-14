# Terminal release hardening report

Internal delivery evidence for #307 / ADR0020. Packaging is #308 and final
adapter removal is #309; this report does not claim installed artifacts or
release publication. The terminal is the reference; desktop-only parity is retired.

## Correctness and lifecycle

Incoming protocol bytes are bounded before concatenation, including unterminated
frames and multibyte UTF-8. The client validates against the generated JSON
schema, rejects malformed/wrong-direction messages, fails pending requests and
reaps only its own backend on protocol error/handshake timeout. Diagnostics
never include the invalid payload. Valid fragmented messages remain accepted.
Shared fixtures now include previously missing settings/auth/data/fix/thread
responses and the nullable operation error already emitted by Rust. No provider
retry, mutation replay, weaker approval or credential policy is introduced.

Coverage uses real owned subprocesses for invalid output, pipes closing while
the process remains alive, delayed requests and handshake timeout; native store
tests cover target-switch races, event gaps/bursts, cancellation, failed agents,
uncertain publication and restart. Rust stdio tests cover owned review/reply/fix,
staged data persistence/publication, EOF/cancel and immutable local evidence.
The native journey tests use temporary repositories and a compiled fake agent,
not actual agents or user profiles. Provider publication fixtures use bounded
owned HTTP peers; there are no live provider writes.

The inherited fixture intermittency was traced to accepted nonblocking macOS
sockets in PR367, with fragmented-header regressions. No production deadlines
or provider retries were changed for it. Post-operation checkout reconciliation
uses the same20-second read budget as preview (PR394); failed/incomplete reads
remain uncertain, and no write is retried.

## Local verification — 2026-10-14

- Complete no-default-feature Rust:776core passed,2ignored benchmark tests;
  80stdio,6CLI and5onboarding tests passed.
- Current-repository baseline PR394:318UI tests and223 native frames; updated
  framing coverage adds11tests, including every shared valid/invalid fixture.
  A single native UI journey now traverses diff/finding/conversation, stages a
  pinned finding, restores its exact draft in a new session and explicitly
  previews/approves one simulated publication; pasted actions never send.
- Real production OpenTUI/Rust local journey passed: qualified local sources,
  layered diff, findings, review, cancellation, restart and missing checkout.
- Six actual POSIX PTY cases passed: editor success, failure, Ctrl+C and backend
  death, plus actual resize and raw UI Ctrl+C (the keyboard quit action),
  with foreground restoration and owned process reaping. Equivalent
  Windows ConPTY cases are enforced by the native CI lane.
- Root typecheck/tests/tooling, UI typecheck/tests, generated-contract drift,
  lint and Archgate are required gates. Native frames are compared, never
  automatically accepted. CI uploads failing frame artifacts.
- Complete core/CLI/stdio suites use at most four concurrent tests and
  run on Linux/macOS/Windows in addition to
  the real local/Git/editor/terminal workflows. Exact-head results must pass
  before integration; a compiled artifact alone is not proof.

## Measurements

Raw evidence: `benchmarks/2026-10-14/{streaming,ipc}.json`. On macOSarm64/Bun1.4.3,
10,000lines and10,000events per mode yielded batch/frame p95 8.86ms unified /
6.89ms split,32/22maximum mounted rows,100retained logs and154MiB maximum
current RSS. Limits remain20ms /64rows /100logs /512MiB. First measurement
while full tests ran concurrently exceeded timing (20.98ms); timing must run
separately from compiler/test load. Deterministic bounds were not relaxed.
CI runs this benchmark after UI tests, preserving measurements.

100real read-only `repository.status` requests against an owned empty config
using the debug Rust backend yielded55.03ms handshake,23.90ms median and25.05ms
p95, with acknowledged shutdown and reaped exit0. Limits remain100/50ms.
An initial measurement under simultaneous tests exceeded timing; the recorded
measurement ran without competing test load. No provider/agent call occurred.
Unlike the old IPC receipt, a missing/null exit code now fails the gate.

These are native renderer/synthetic event measurements, not physical-terminal
latency or live-provider performance. RSS is cumulative current process memory.
Prior matched Ratatui/rendering evidence remains in rendering-baseline.md;
installation, minimal-PATH startup, upgrade and rollback remain required in #308.

## Reproduction

```sh
cargo test --locked --manifest-path src-tauri/Cargo.toml --no-default-features
bun run --cwd ui typecheck
bun test --cwd ui
bun run --cwd ui frames:check
bun run ui/scripts/check-local-workflow.tsx
bun run ui/scripts/check-repository-operations-ipc.ts
bun run ui/scripts/check-editor-workflow.ts
bun run ui/scripts/streaming-diff-bench.ts /tmp/norn-streaming.json
bun run ui/scripts/ipc-bench.ts src-tauri/target/debug/norn-backend /tmp/norn-ipc.json
pnpm run typecheck
pnpm run protocol:check
pnpm run test
pnpm run lint
archgate check
```

Native scripts require the no-default-feature `norn` and `norn-backend` debug
binaries and owned Git/Rust tooling. Run timing separately from other gates.

A second full stdio run competing with root/UI tests timed out in the owned
local image/browser case (79 passed /1 failed); the earlier full suite passed 80.
The final stdio suite with four concurrent tests passed 80 without changing
production deadlines or fixture assertions. CI caps native suite concurrency
to four, while dedicated race/concurrency tests still exercise their owned
concurrent requests. This records the observed failure rather than claiming
that a passing rerun alone proves a production fix.

The first expanded Linux CI lane reported missing browser assets in two stdio
tests. It previously built only Rust/UI dependencies. Native CI now installs
web dependencies and builds the actual retained browser bundle before these
tests on every OS; fixtures and production asset resolution remain unchanged.

The first complete Windows stdio compile exposed two inherited agent tests
using Unix-only PermissionsExt and shell executables. Both use a native
compiled std-only fixture now, retaining streaming/cancel/no-late-completion
assertions on every OS. No test is skipped to bypass Windows compilation.
