# Norn Repository Configuration Migration

## Owning ADRs

- `../../adr/0011-norn-naming-and-compatibility.md`

## Scope

Implement GitHub issue #175 by making `.norn.yaml`, `.norn/`, and
`.norn.local.yaml` canonical repository policy sources. Legacy Lachesi names
remain fallback-only during the compatibility window, ambiguous coexistence is
rejected, and the CLI can preview or execute a no-overwrite migration.

## Exit Criteria

- ADR 0011 AC3: canonical repository inputs are preferred and equivalent
  legacy inputs are read only when the canonical source is absent.
- Coexisting canonical and legacy sources fail with actionable guidance and
  never merge implicitly.
- `norn config migrate --dry-run` previews file/directory renames and YAML path
  rewrites without mutating the repository.
- Executed migration never overwrites a canonical target and new metadata uses
  canonical paths.
- Validation tests cover canonical-only, legacy-only, coexistence, local
  override safety, and migration idempotence.
- GitHub issue #175 acceptance criteria pass.

## Dependencies

- `../../adr/0011-norn-naming-and-compatibility.md`
- GitHub issue #175
