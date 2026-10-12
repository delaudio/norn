# Automatic Refresh Schedule in the Workspace

## Owning ADRs

- `../../adr/0020-opentui-rust-stdio-workspace.md`

## Scope

Restore the shipped automatic-refresh semantics in the OpenTUI workspace. Port
the retained `automaticSyncIntervalSeconds` values (Off, 30s, 60s, 5m, 10m) to a
client scheduler that performs at most one in-flight bounded read-only refresh
(global review history), clears its timer on interval/settings change,
disconnect and unmount, and never resets the selected target, files, findings or
unsaved drafts. Surface syncing/error status in the shell.

GitHub issue: #363 (epic #294 step 12/15).

Out of scope (tracked): the offline timer fencing proofs on fake clocks and
background operation completion/recovery panels.

## Exit Criteria

- `syncIntervalMs` maps only the retained intervals and treats everything else
  as off; unit-tested.
- The shell schedules refresh from the configured interval, and `refresh`
  updates the global history without touching the selection or drafts.
- `?` opens a keyboard-help overlay built from a single binding catalogue that
  matches the real handlers; `Esc`/`q`/`?` closes it.
- `pnpm run lint/typecheck/test`, `cargo clippy -D warnings`, `cargo test`, the
  `ui` job and `archgate check` pass.

## Dependencies

- `../../adr/0020-opentui-rust-stdio-workspace.md`
