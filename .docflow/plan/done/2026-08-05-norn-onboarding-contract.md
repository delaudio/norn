# Norn Onboarding Contract and Shared Readiness Surface

## Owning ADRs

- `../../adr/0012-norn-onboarding-contract.md`
- `../../adr/0011-norn-naming-and-compatibility.md`

## Scope

Implement GitHub issue #186 by separating machine setup (`norn setup`) from
repository initialization (`norn init`) and adding deterministic onboarding
proposals, `--dry-run` + JSON output, non-interactive mode boundaries, and
read-only readiness validation through `norn doctor`.

## Exit criteria

- `norn setup` and `norn init` are independent commands with distinct success
  semantics.
- `--dry-run` and JSON output describe proposed configuration mutations without
  mutating repository files or secrets.
- `--yes` is limited to documented quick paths and explicitly rejected in
  interactive/restricted `norn init` modes.
- Legacy namespace names remain supported only where explicitly compatible, with
  canonical Norn identifiers taking precedence.
- `norn doctor` provides a read-only machine/repo/protocol surface with pass/warn/fail
  machine parseable output.

## Shipped

- Issue #186 acceptance criteria pass for onboarding command behavior and onboarding
  readiness checks.
- GitHub issue #186

## Implementation

- `f461755`: add setup and init onboarding commands.
- `af07a34`: reject conflicting `--yes`/`--dry-run` combinations for setup and init.
- `3887756`: require `--yes` for guided init at execution time and add regression coverage.

