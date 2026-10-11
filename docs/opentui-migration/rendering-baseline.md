# Same-host native rendering baseline

Internal evidence for #302/#301, plan 0041 and ADR 0020. This records the
rendering comparison; streaming-agent and installed-artifact measurements are
separate remaining #303/#307/#308 gates.

## Workloads and timing

The shared input generator reads only the published tracked backend source
between `cdf6bdf0e8f2532578db47b4a05de9521e2e24ee` and
`e9c1ec78a225a9a9c7b55127c65a09b294afc192`. It never reads local configuration,
credentials or the working tree. Synthetic inputs cover 10/100/1,000 files and
1,000/10,000-line patches. Both native renderers consume the same selected-file
patch, review file identities and 120x30 terminal. The input SHA256 in every
result is identical, as are selected path/byte count and full review byte count.

Ratatui runs its shipping diff render function with a TestBackend in a release
Rust test binary. OpenTUI runs the shipping shell with its native test renderer
and fake transport. Renderer construction and obtaining the source are outside
first-frame timing; actual diff body presence is asserted in both. OpenTUI's
first usable frame includes React mount, native layout and the follow-up frame
that reconciles the visible window. Each scroll sample includes the viewport
move, React window reconciliation and the follow-up frame (with one zero-delay
task yield). Ratatui is synchronous and requires one draw. Both measure 100
positions covering the actual visual scroll extent. Native panel layouts differ;
this is the existing product workflow on equal terminals, not equal pixels or
an isolated microbenchmark of one text widget.

The old renderer reparses the full review during a draw; the new client already
has its selected-file patch and caches row geometry. That is an intended
architecture difference. Separate fake connect/select timings are recorded and
are not OS startup or real IPC latency. No review/provider/agent process runs.

## Budgets

For every shared case and mode, the checker enforces:

- First usable frame <=100 ms, including the first cold runtime case.
- Scroll p95 <=20 ms and <=(2 x matched Ratatui p95 +8 ms). The fixed allowance
  reserves half a 60 Hz frame for React presentation reconciliation; both limits
  must pass, so a slow baseline does not excuse unusable new scrolling.
- At most 64 mounted diff rows at 120x30, and at least one real mounted row.
- OpenTUI process-wide current RSS <=512 MiB across the fixed six-case run.

Timing is opt-in, not a flaky shared-runner CI assertion. Structural bounds and
correctness are exercised in native tests. These are measured rendering budgets,
not proof that streaming, cold installed launch, backend lifecycle or physical
image display gates passed.

## Recorded results

2026-10-11, macOS arm64, Bun 1.3.8/OpenTUI 0.5.17, Rust 1.94.0 release.
Three separate process runs; all six cases and both modes pass in every run.
The table gives medians across runs in milliseconds. Raw artifacts are in
`benchmarks/2026-10-11/`; do not infer statistical certainty from three runs.

| Case | Mode | Ratatui first frame | Ratatui scroll p95 | OpenTUI first frame | OpenTUI scroll p95 |
|---|---|---:|---:|---:|---:|
| files-10 | unified | 0.47 | 0.09 | 20.68 | 0.76 |
| files-10 | split | 0.12 | 0.10 | 6.18 | 0.41 |
| files-100 | unified | 0.17 | 0.14 | 4.04 | 0.37 |
| files-100 | split | 0.15 | 0.15 | 4.11 | 0.28 |
| files-1000 | unified | 0.58 | 0.54 | 2.74 | 0.29 |
| files-1000 | split | 0.54 | 0.54 | 2.96 | 0.16 |
| lines-1000 | unified | 0.18 | 0.43 | 3.43 | 1.35 |
| lines-1000 | split | 0.29 | 0.82 | 4.23 | 1.90 |
| lines-10000 | unified | 0.90 | 3.82 | 9.94 | 2.51 |
| lines-10000 | split | 1.97 | 7.32 | 11.62 | 3.32 |
| real-norn-backend | unified | 0.33 | 0.92 | 2.83 | 1.19 |
| real-norn-backend | split | 0.56 | 1.90 | 5.67 | 3.53 |

Ratatui remains faster in initial rendering and uses substantially less memory.
OpenTUI trades runtime/mount overhead for the richer component workflow while
remaining inside the absolute usability bounds. Large split scrolling benefits
from cached viewport geometry. Ratatui peak RSS uses getrusage (macOS bytes,
other Unix KiB); OpenTUI RSS is current process memory. Both are cumulative across
cases and are not isolated per-fixture measurements or equivalent memory metrics.

## Reproduction

From the repository root, with Bun/pnpm and the current Rust toolchain:

```sh
bun run --cwd ui scripts/render-bench-inputs.ts /tmp/norn-render-inputs.json
NORN_RENDER_BENCH_INPUT=/tmp/norn-render-inputs.json \
NORN_RENDER_BENCH_OUTPUT=/tmp/norn-ratatui.json \
cargo test --manifest-path src-tauri/Cargo.toml --no-default-features \
  --features tui --release --lib matched_rendering_benchmark -- --ignored --nocapture
bun run --cwd ui scripts/render-bench.ts /tmp/norn-render-inputs.json /tmp/norn-opentui.json
bun run --cwd ui scripts/check-render-bench.ts /tmp/norn-ratatui.json /tmp/norn-opentui.json
```

For repeated timing after compilation, run the produced test executable directly
with the same environment and test filter so Cargo/compiler memory is excluded.
Windows uses its native environment assignment syntax; Ratatui peak RSS is null
there rather than claiming unsupported getrusage measurements. Installed and
cross-platform results are not claimed by the macOS evidence here. At cutover,
pin a published pre-removal commit for reproducing the old harness in an isolated
checkout; no runtime product dependency on Ratatui is introduced.

## Real read-only IPC latency and open lifecycle evidence

The no-default-features release backend was also measured through the production
process transport using an explicit environment pointing to empty temporary
settings/data directories. `repository.status` was requested 100 times; the
empty result assertion proves isolation. No auth, review, provider or agent
request runs. The subprocess handshake measured 6.66 ms, median round trip
25.93 ms, p95 30.01 ms; current read-only budgets are <=100 ms handshake and
<=50 ms p95. The writer's 25 ms event wait explains the latency floor and is a
#307 responsiveness follow-up, not a hidden rendering measurement.

Shutdown is acknowledged, then EOF arrives, but the current transport reports
`null` before reaping the real exit status. The artifact explicitly records
that limitation; it does not claim a verified exit code or lifecycle parity.
Fix EOF/reaping/graceful close in #307 and rerun this measurement before cutover.
The optional explicit transport environment preserves existing launch behavior
when omitted and makes subprocess test isolation reliable; mutating Node-style
`process.env` after Bun startup does not change its default child environment.
A fixture subprocess test proves the override is received.

```sh
cargo build --manifest-path src-tauri/Cargo.toml --no-default-features \
  --release --bin norn-backend
bun run --cwd ui scripts/ipc-bench.ts ../src-tauri/target/release/norn-backend /tmp/norn-ipc.json
```

The raw IPC artifact is `benchmarks/2026-10-11/ipc.json`. Latency and EOF status
are separate from the native-rendering comparison and installed-startup gates.
