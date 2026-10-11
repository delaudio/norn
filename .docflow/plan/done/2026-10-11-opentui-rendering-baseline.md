# Matched rendering baseline and workload budgets

## Owning ADRs

- `../../adr/0020-opentui-rust-stdio-workspace.md` (AC 4, 9).

## Scope

Record same-host Ratatui/OpenTUI rendering measurements using shared deterministic
Git diff inputs and terminal dimensions. Include 10/100/1,000-file reviews,
1,000/10,000-line diffs and a real tracked-source Norn patch pinned to published
commits. Separate engine/IPC/connect latency from direct rendering measurements;
do not mislabel fake transport connect timing as OS startup or compare unequal
loads. Keep a repeatable baseline artifact and a documented invocation of the
archived Ratatui harness before removal. No review engine/provider execution,
credentials, HOME configuration or live publication. Add a read-only real IPC
measurement using an explicit backend environment and isolated empty config;
record existing EOF/exit-code limitations rather than claiming lifecycle parity.
Streaming-load correctness
and lifecycle remain #303/#307 gates; record them as pending if not measured here.

## Exit Criteria

1. Both renderers consume identical selected-file patch inputs with an identical
   120x30 terminal; output records runtime/build/host, frame/scroll p95, workload
   size and limitations. Real source input is generated only from pinned tracked
   files, never local user/config data.
2. Budgets distinguish proportional regression thresholds from absolute usability
   limits; rendering, parsing, transport and OS startup are not conflated.
3. Repeatability and passing/failing gates are recorded honestly; synthetic tests
   prove both harnesses draw their actual native diff UI and reject missing panels.
4. Required existing root/UI/Rust/Archgate gates pass. Pending broader performance
   and platform checks remain explicit in the parent plans/issues.

## Dependencies

- Plan 0040 / PR #342 must merge before integration.

## Status

- Claimed by: default-agent, 2026-10-11, `issue-302-rendering-baseline`.
- Blockers: None; PR #342 merged at `2a2ed72` with both CI gates green. Real IPC EOF status and writer latency are explicitly tracked for #307.
- Stopped:

## Delivery

Effective on merge to main of the matched-baseline PR. Six shared workloads,
two native modes, 100 scroll samples and three separate process runs pass
the recorded budgets. Shared input hashes and selected patch identity/bytes
match; native bodies are asserted. Raw artifacts and repeatable scripts are
committed. Root lint/typecheck/123 tests/tooling, Archgate 17/17, 75 UI tests
and 44 native frames, strict all-feature/all-target Clippy and the opt-in
release Ratatui test pass. The subprocess test proves explicit environment
isolation; read-only real Rust IPC is measured with empty temporary settings.
Its ACK/EOF exit-code limitation and 25 ms writer wait remain #307 follow-ups,
not claimed lifecycle completion. Streaming/installed/platform gates remain
open; #302 and ADR 0020 are not closed by this rendering delivery.
