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
credentials, HOME configuration or live publication. Streaming-load correctness
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
- Blockers: PR #342 CI pending; isolated local preparation only.
- Stopped:
