# Local Review Target in the Desktop App

## Owning ADRs

- `../../adr/0017-review-local-changes-before-publication.md`

## Scope

Expose Local alongside provider pull-request states in the desktop app after
the terminal-first native capability ships. Consume the same snapshot command,
domain model, and shared React diff renderer rather than recreating Git or diff
logic in TypeScript. Present repository, branch, upstream, ahead, empty, warning,
and error states accurately.

Support desktop AI review against the immutable local snapshot while omitting
provider-only approval, remote comments, publication, and branch-sync controls.

Out of scope: changing the snapshot semantics established by the TUI item,
continuous filesystem watching, provider publication, and repository mutations.

## Exit Criteria

- ADR 0017 AC7: the desktop app lists and opens Local targets using the shared
  native snapshot and existing shared diff viewer.
- ADR 0017 AC6-7: desktop AI review consumes the immutable snapshot and local
  targets expose no provider-only actions.
- ADR 0017 AC8: loading, refresh, empty, warning, error, cancellation, and stale
  asynchronous result states have automated coverage.
- ADR 0017 AC9: existing GitHub and Bitbucket pull-request workflows remain
  backward compatible.
- `pnpm run typecheck`, `pnpm run test`, and `archgate check` pass.

## Dependencies

- `../../adr/0017-review-local-changes-before-publication.md`
- `./0013-local-review-target-tui.md`
