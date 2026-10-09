# Vertical slice: file -> diff -> finding

This records how the real file -> diff -> finding slice is exercised and
measured, per the migration contract and ADR 0020. It covers the shipped
OpenTUI path and the still-pending Ratatui baseline.

## What the slice does

1. `review.targets` lists real, offline review targets: recent stored review
   jobs whose workspace/repo match a configured repository, each carrying its
   `prId`. The target picker shows a working-tree target per configured
   repository as well as these stored PR targets, so a reviewer can reach a
   stored PR review without a webview and without starting a new AI review,
   while still inspecting local working-tree changes.
2. `review.files` lists changed files for the selected target
   (Rust backend `changed_file_entries`; NUL-delimited git name-status +
   numstat, including untracked files, renames and unusual filenames).
3. Selecting a file issues `diff.file`; the diff is fenced by a monotonic
   request id so a late response never overwrites a newer selection.
4. `review.findings` lists findings with optional source anchors. Jumping to a
   finding selects its file — resolving renamed files through `oldPath` — and
   highlight line/side; a finding whose anchor line is not present in the diff
   surfaces a notice instead of highlighting a guessed line.

## Commands

```bash
# Rust backend integration (real git repo, real process)
cargo test --manifest-path src-tauri/Cargo.toml --test backend_stdio

# OpenTUI state/transport/shell
bun run --cwd ui typecheck
bun run --cwd ui test

# Deterministic native frames for the slice scenes
bun run --cwd ui frames:generate
bun run --cwd ui frames:check

# OpenTUI benchmark (small / 100 / 1000 files)
bun run --cwd ui bench
```

## OpenTUI measurements

`ui/scripts/bench.ts` renders the shell on the fake transport with generated
fixtures and reports connect-to-frame latency, the time to apply 200 selection
moves and render once, and per-file selection-to-diff-resolved latency.

Reference run (Apple silicon, Bun 1.3.8, 120x30 terminal):

| files | connect-to-frame ms | 200 moves ms | per-file diff ms | process RSS MB |
| ----- | ---------- | ------------ | ---------------- | ------ |
| 10    | 20.54      | 5.93         | 3.30             | 122    |
| 100   | 3.91       | 0.78         | 2.18             | 135    |
| 1000  | 3.98       | 3.08         | 3.08             | 150    |

`processRssMb` is process-wide RSS read before teardown; fixtures share one
process, so it is a cumulative reading, not an isolated per-fixture footprint.
`connectToFrameMs` ends at the first usable frame (client connect, store
construction, mount, and one render); it is not OS-level cold start.
`bulkMovesMs` applies 200 selection moves and renders once without waiting on
diffs. `diffLatencyMsPerFile` selects a file, waits for its diff to resolve, and
renders, averaged over up to 20 files. Numbers are JIT-warmed after the first
fixture. Absolute values are machine dependent; the script is the stable
artifact.

## Ratatui baseline (pending)

The comparison baseline is the shipping Ratatui TUI on the same fixtures. It
requires a separate harness run on the same host and has not been produced here,
so no comparative threshold in the migration contract has been satisfied yet.
This is the remaining gate for the "deliver and measure" work item; the
functional slice above is delivered and independently tested, but the measured
comparison is not.

## Exit criteria

- The slice is driven end to end against the real Rust backend (`backend_stdio`).
- The OpenTUI benchmark command is committed and repeatable.
- Ratatui-vs-OpenTUI numbers exist on the same host and the contract thresholds
  are checked. Until then this item stays open.
