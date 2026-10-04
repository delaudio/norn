# Repository Health Audit

## Owning ADRs

- `../../adr/0019-repository-health-audit.md`

## Scope

Implement the read-only repository health audit defined by ADR 0019 and GitHub
epic #150: a deterministic, bounded inventory of a stable snapshot, deterministic
analyzer/structural evidence, a findings contract with provenance, an end-to-end
audit pipeline, persisted baselines and deltas, and a terminal backlog surface.
Audit and remediation stay separate; the audit never mutates the repository.

Child GitHub issues: #158 (contract), #159 (inventory), #160 (evidence),
#161 (findings/prompt contracts), #162 (pipeline), #163 (baselines/deltas),
#164 (TUI backlog). Bounded specialist orchestration (#151) is a later
enhancement.

Out of scope: automatic remediation, a full cross-language static-analysis
runtime, untracked content, and a hosted/shared audit target.

## Exit Criteria

- ADR 0019 AC1-2: snapshot identity and dirty-tree behavior are deterministic and
  explicit.
- ADR 0019 AC3: generated, vendored, binary, submodule, symlink, sensitive, and
  ignored paths are excluded and reported.
- ADR 0019 AC4-5: read-only guarantee and deterministic repeatability are tested.
- ADR 0019 AC6-7: limits fail or truncate with warnings; cancellation is clean.
- ADR 0019 AC8: findings carry category, severity, evidence references, and
  snapshot provenance.
- ADR 0019 AC9: persisted snapshots support retained baselines and deltas
  without colliding with PR/local-review storage.
- ADR 0019 AC10: the terminal UI presents the backlog and evidence.
- ADR 0019 AC11-12: no remediation side effects; existing workflows stay
  backward compatible.

## Dependencies

- `../../adr/0019-repository-health-audit.md`
- `../../adr/0017-review-local-changes-before-publication.md`
