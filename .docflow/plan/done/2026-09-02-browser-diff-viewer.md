# Browser Diff Viewer

## Owning ADRs

- `../../adr/0016-browser-diff-viewer.md`

## Scope

Complete and harden the terminal-launched browser diff viewer. Bind cached diff
content to the loaded pull request identity, stop automatic provider retries
after a failed population attempt, restrict browser previews to inert raster
formats, avoid copying unchanged diff bodies during browser polling, parse
Git-quoted paths, normalize provider file statuses, and add focused regression
coverage.

Repair the audit-adjacent command test contract so desktop-default Cargo tests
do not execute CLI end-to-end assertions against the GUI binary, while retaining
an explicit command-distribution test script.

## Exit Criteria

- ADR 0016 AC1-6 are implemented and covered by focused tests.
- Selecting a second pull request without loading it cannot reuse the first pull
  request's diff or detail.
- Provider population failure remains stable across browser polling and an
  explicit reopen permits a retry.
- SVG responses are not emitted as `image/svg+xml` by the browser server.
- Unchanged browser polling returns no state body and does not clone the cached
  diff.
- Git-quoted paths render as their decoded repository paths.
- Provider file statuses are normalized before the browser renders them.
- Default Cargo tests skip command-only end-to-end journeys and
  `pnpm run test:rust:cli` runs them against a command-distribution build.
- `pnpm run typecheck`, `pnpm run test`, `cargo clippy`, and `archgate check`
  pass.

## Dependencies

- `../../adr/0016-browser-diff-viewer.md`
- `../../adr/0006-terminal-ui.md`

## Shipped

Shipped at HEAD `32c51e633ba10a5bdbefd6fedf1a3aef7df4a10e` through
[PR #230](https://github.com/delaudio/norn/pull/230).
