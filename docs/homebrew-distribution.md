# Homebrew distribution runbook

This document is the canonical onboarding guide for operating Norn release artifacts
through Homebrew on clean machines.

## Supported environments

- macOS 13 and 14
- Intel (`x86_64`) and Apple Silicon (`arm64`)
- Package versions from GitHub tags (`v<semver>`)

## One-command onboarding

From a clean machine (no source checkout required):

```sh
brew tap lachesi-hq/tap
brew install norn

norn --version
norn --help
norn doctor
```

The `norn` formula installs both the CLI (`norn`) and terminal UI (`norn-tui`) and
does not require Node.js, Rust, or the source repository.

If `norn` is not found immediately after install, restart the shell or source your
shell startup file (`~/.zshrc`, `~/.bash_profile`, etc.).

## First-run requirements

Before first review run:

1. Configure required credentials from the app onboarding screens.
2. Validate provider access:
   - `norn doctor` should report configured GitHub and/or Bitbucket connectivity
     checks.
3. Confirm local policy config presence:
   - `.norn.yaml` or migration-compatible `.lachesi.yaml` is discovered.

## Upgrade

```sh
brew update
brew upgrade norn
```

After upgrade:

```sh
norn --version
norn doctor
norn-tui --version
```

## Rollback and repair

If a release must be reverted, uninstall and reinstall a known-good formula
revision from Homebrew history.

```sh
brew uninstall norn
brew install delaudio/tap/norn
```

If a specific prior version is required, pin the tap to the older formula commit
and reinstall.

```sh
brew tap --repair
brew tap reset lachesi-hq/tap <tap-commit-sha>
brew install delaudio/tap/norn
```

## Desktop cask path

Once notarized desktop artifacts are available:

```sh
brew install --cask norn
```

To remove the desktop app:

```sh
brew uninstall --cask norn
```

## Troubleshooting

- **PATH still points to old binary:** ensure your shell `PATH` resolves `norn`
  from Homebrew and restart the terminal.
- **Provider auth fails:** re-run the onboarding flow in-app and recheck env/secret
  values.
- **`brew install` is pulling build toolchain dependencies:** release artifacts are
  binary-only; if dependencies are requested, the install path is likely mis-pointing
  to a source-only tap entry.
- **Upgrade fails:** rerun with verbose logs and capture output.

```sh
brew upgrade --verbose norn
```

- **App is quarantined:** do not strip signatures locally. File a release integrity
  ticket with the failing release tag.

## Release operations

Before publishing each stable tag:

- Run local gates (`pnpm run typecheck`, `pnpm run test`, `pnpm run test:tauri`) on a
  clean machine.
- Publish immutable assets for both supported architectures:
  - `norn-<version>-macos-arm64.tar.gz`
  - `norn-<version>-macos-x86_64.tar.gz`
- Attach `.sha256` sidecar files and metadata.
- Update formula/cask metadata to immutable release URLs and version.
- Keep the lifecycle smoke workflow green on macOS 13 and 14.

Every release must include:

- tag → commit provenance mapping,
- workflow run identifier,
- checksums, and
- architecture marker in filename.

You can manually trigger lifecycle smoke validation from this repo:

```sh
gh workflow run homebrew-lifecycle-smoke.yml
```
