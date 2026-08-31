# Release, Installation, and Upgrade Hardening

## Owning ADRs

- `../../adr/0014-reproducible-release-and-upgrade-channels.md`

## Scope

Implement GitHub epic #204 and issues #199-#203 in dependency order. Harden the
release gate first, then generate and publish checksummed Homebrew metadata,
make clean installation and real upgrades release-blocking, provide durable
source installation, and gate the desktop cask on verified app artifacts.

Out of scope: an application-managed self-updater, new package managers, new
operating systems, or changes to review/provider semantics.

## Exit Criteria

- ADR 0014 AC1-2: issue #199 delivers the complete locked release gate and
  immutable CLI/TUI artifact provenance.
- ADR 0014 AC3: issue #200 renders exact formula checksums and advances the tap
  only after verification.
- ADR 0014 AC4-5: issue #201 proves clean installation and previous-to-candidate
  upgrades on supported macOS architectures.
- ADR 0014 AC6: issue #203 provides durable, configurable source installation.
- ADR 0014 AC7: issue #202 gates cask publication on signed and notarized app
  artifacts and lifecycle checks.
- ADR 0014 AC8: every delivery stage fails closed without leaking credentials or
  private machine paths.
- The repository verification gate passes: `pnpm run typecheck`, `pnpm run
  test`, and `archgate check`.

## Dependencies

- `../../adr/0014-reproducible-release-and-upgrade-channels.md`
- GitHub epic #204
- Delivery order: #199 -> #200 -> #201; #203 may follow #199 independently;
  #202 remains a separate desktop slice after the release foundation.
