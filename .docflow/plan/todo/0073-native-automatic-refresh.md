# Automatic Refresh Schedule in the Workspace

## Owning ADRs

- `../../adr/0020-opentui-rust-stdio-workspace.md`

## Maintainer scope revision (2026-10-13)

The TUI is the baseline; desktop polling parity is no longer a migration
requirement. Keep already merged useful scheduler/help behavior, but scope any
remaining refresh work to the current terminal target and preserve unsaved
state. It must not turn global desktop history into the startup screen or
trigger captures, Git writes, agent runs or publication. Plan 0074 owns the
current-repository entry correction. Original criteria below record prior
intent; they do not authorize new desktop-only work.

## Scope

Restore the shipped automatic-refresh semantics in the OpenTUI workspace. Port
the retained `automaticSyncIntervalSeconds` values (Off, 30s, 60s, 5m, 10m) to a
client scheduler that performs at most one in-flight bounded read-only refresh
(global review history), clears its timer on interval/settings change,
disconnect and unmount, and never resets the selected target, files, findings or
unsaved drafts. Surface syncing/error status in the shell.

GitHub issue: #363 (epic #294 step 12/15).

Out of scope (tracked): background operation completion/recovery panels beyond
the existing shells.

## Exit Criteria

- `syncIntervalMs` maps only the retained intervals and treats everything else
  as off; the `startSyncScheduler` timer is unit-tested with fake timers (off
  starts nothing; each retained interval fires; stop clears it), and `refresh`
  is proven single-in-flight.
- The shell schedules refresh from the configured interval, and `refresh`
  updates the global history without touching the selection or drafts.
- `?` opens a keyboard-help overlay built from a single binding catalogue that
  matches the real handlers; `Esc`/`q`/`?` closes it.
- `pnpm run lint/typecheck/test`, `cargo clippy -D warnings`, `cargo test`, the
  `ui` job and `archgate check` pass.

## Dependencies

- `../../adr/0020-opentui-rust-stdio-workspace.md`
