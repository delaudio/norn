# Norn Onboarding Contract and Shared Readiness Surface

## Owning ADRs

- `../../adr/0012-norn-onboarding-contract.md`
- `../../adr/0011-norn-naming-and-compatibility.md`

## Scope

Implement GitHub issue #186 by defining and documenting the first-run onboarding
contract for machine setup, repository initialization, deterministic proposals,
`--dry-run`, JSON output, `--yes` fast path boundaries, and a read-only
readiness probe.

## Exit Criteria

- The issue defines machine setup, repository initialization, and readiness behavior
  as explicit commands and state transitions.
- Proposal generation is deterministic for the same repository evidence.
- `--dry-run` and JSON output describe all intended filesystem writes.
- No secrets or personal environment values are persisted in committed repository files.
- ADR 0012 is implemented as Accepted before implementation work starts.
- Issue #186 has a clearly documented exit reference in docflow after PR merge.

## Dependencies

- `../../adr/0012-norn-onboarding-contract.md`
- GitHub issue #186
