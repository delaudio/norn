# Build the OpenTUI React Shell and Typed Backend Client

## Owning ADRs

- `../../adr/0020-opentui-rust-stdio-workspace.md`

## Scope

Add the OpenTUI + React workspace package (`ui/`, Bun) and a typed backend
client that talks to the `norn-backend` subprocess over the versioned protocol.
Provide a production subprocess transport and a deterministic fake transport,
shell panels with focus navigation, help/keybindings, status/error/loading/empty
views, responsive layout, and terminal restoration on quit/Ctrl+C/backend death.
UI state owns selection/focus/scroll and fences async results by target
generation and operation id; the backend owns persistent data and operations.

GitHub issue: #299 (epic #294 step 05/15).

Out of scope: migrating every screen, importing DOM/Radix components into
OpenTUI, or performing provider calls from TypeScript.

## Exit Criteria

- The shell starts against both the fake transport and the real `norn-backend`
  handshake.
- Keyboard focus/navigation/quit work at compact and wide terminal sizes; text
  editing does not capture shortcuts.
- Backend errors produce actionable UI state with no unhandled rejections.
- Switching targets cannot show stale data; remounting does not duplicate
  events.
- `bun --cwd ui run typecheck` and `bun --cwd ui test` pass; the CI `ui` job
  runs them. The existing pnpm/browser/docs workspace keeps working.
- `pnpm run typecheck`, `pnpm run test`, and `archgate check` pass.

## Known limitation

- A PTY smoke for terminal restoration is deferred to #307 (terminal lifecycle
  hardening); renderer/input tests cover focus, navigation, resize and cleanup.

## Dependencies

- `../../adr/0020-opentui-rust-stdio-workspace.md`
- `../../../docs/opentui-migration/migration-contract.md`

---

Shipped in `25c7caf` (PR #314, issue #299).
