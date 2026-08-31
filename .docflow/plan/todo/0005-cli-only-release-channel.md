# CLI-Only Release Channel

## Owning ADRs

- `../../adr/0014-reproducible-release-and-upgrade-channels.md`

## Scope

Allow a governed Norn release to publish CLI and TUI archives plus the Homebrew
formula when Apple signing and notarization are intentionally unavailable.
Desktop DMG construction, cask rendering, desktop lifecycle tests, and cask tap
publication must remain disabled unless the release explicitly enables the
desktop channel and supplies every required Apple credential.

Preserve the existing fail-closed version, provenance, checksum, architecture,
installation, upgrade, uninstall, and reinstall gates for the command channel.
Keep the public tap update limited to the manifests produced and verified by
the same release run.

Out of scope: unsigned desktop distribution, ad-hoc signing, weakening
Gatekeeper or notarization checks, changing supported operating systems, or
adding another package manager.

## Exit Criteria

- ADR 0014 AC1-2: a stable tag publishes locked, traceable CLI/TUI artifacts
  only after the complete command-channel verification gate succeeds.
- ADR 0014 AC3-5: the formula is rendered from release artifacts, passes clean
  install and authorized bootstrap or upgrade lifecycle tests on both supported
  macOS architectures, and advances the public tap only after those tests pass.
- ADR 0014 AC7-8: the desktop cask and DMG assets are absent when the desktop
  channel is disabled; enabling it still requires complete signed, notarized,
  checksummed artifacts and all desktop lifecycle gates.
- Workflow contract tests cover both command-only and desktop-enabled release
  dependency graphs, manifest sets, and tap publication paths.
- The repository verification gate passes: `pnpm run typecheck`,
  `pnpm run test`, and `archgate check`.

## Dependencies

- `../../adr/0014-reproducible-release-and-upgrade-channels.md`
- No pending plan item must land first.
