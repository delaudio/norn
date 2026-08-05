# Norn Readiness Probe

## Owning ADRs

- `../../adr/0013-norn-readiness-probe.md`
- `../../adr/0012-norn-onboarding-contract.md`
- `../../adr/0011-norn-naming-and-compatibility.md`

## Scope

Implement GitHub issue #187 by adding `norn doctor` as a shared, read-only
readiness probe used by CLI, TUI, and desktop flows before onboarding writes.

## Exit Criteria

- `norn doctor` inspects machine state, repository state, provider capabilities,
  and config precedence without writing files or mutating credentials.
- JSON output includes deterministic status and issue objects with severity and
  remediation text.
- Missing remote/credential/config edges are explicit warnings or errors, never
  silent success conditions.
- Fixture coverage includes GitHub and Bitbucket, no remote, dirty tree, and legacy/
  canonical config precedence.
- ADRs 0012 and 0013 are implemented after acceptance and referenced in completion docs.

## Dependencies

- `../../adr/0013-norn-readiness-probe.md`
- `../../adr/0012-norn-onboarding-contract.md`
- GitHub issue #187
