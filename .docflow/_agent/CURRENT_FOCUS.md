# Current Focus

This file is the live snapshot of any in-flight session. It is short on
purpose; the durable record lives in git, `_agent/WORKLOG.md`, and
`plan/done/`. Queued work lives in `plan/todo/`.

If status files and git disagree, git is authoritative; correct this file.

## Active state

- **Branch:** `issue-299-opentui-shell` (to be created).
- **Active item:** epic #294 step 05/15, issue #299 - OpenTUI React shell and
  typed backend client.
- **Change:** new `ui/` Bun package (`@norn/ui`) with OpenTUI 0.5.17 + React
  19.3.0: a typed `BackendClient` over a `BackendTransport` interface with a
  production `ProcessBackendTransport` (spawns `norn-backend` by resolved path,
  NDJSON over stdin, request correlation, bounded pending, timeouts, EOF and
  version-mismatch handling) and a deterministic `FakeBackendTransport`; a
  `ShellStore` that fences async results by target generation and operation id;
  an OpenTUI shell with panels, focus navigation, help, status/error/loading/
  empty views and responsive layout; and `src/main.ts` with terminal restore on
  quit/Ctrl+C/init failure. `ui/` has its own typecheck and `bun test` suites; a
  CI `ui` job and `ui-typecheck`/`ui-test` task-runner recipes were added.
- **Plan items:** `.docflow/plan/todo/0020-opentui-shell.md` (this item);
  `plan/done/2026-10-09-opentui-{rust-architecture,engine-extraction,protocol,backend}.md`.

## Known limitation

- PTY smoke for terminal restoration is deferred to #307; renderer/input tests
  cover focus, navigation, resize and cleanup.

## Next item

- After #299 merges, proceed to #300 (native OpenTUI frame previews and visual
  regression testing in Storybook).

## Epic queue

#295 -> #296/#297 -> #298 -> #299 -> #300 -> #301 -> #302/#303/#305 ->
#304 -> #306 -> #307 -> #308 -> #309 -> close #294.
