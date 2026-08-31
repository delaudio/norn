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
brew tap delaudio/tap
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
brew tap reset delaudio/tap <tap-commit-sha>
brew install delaudio/tap/norn
```

## Desktop cask

When a release enables the desktop channel, its cask installs the signed and
notarized app bundle without adding another `norn` executable, so it can coexist
with the command formula:

```sh
brew install --cask norn
open -a Norn
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
- When the desktop channel is enabled, also publish:
  - `Norn-<version>-macos-arm64.dmg`
  - `Norn-<version>-macos-x86_64.dmg`
- Attach `.sha256` sidecar files and metadata.
- Render the formula from `packaging/homebrew/norn.rb.template` using the two
  archives and checksum sidecars produced by that same workflow run.
- When the desktop channel is enabled, render the cask from
  `packaging/homebrew/norn-cask.rb.template` using the two notarized DMGs and
  checksum sidecars produced by that same workflow run.
- Publish the rendered `norn.rb` as an immutable release asset. Publish
  `norn-cask.rb` only for a desktop-enabled release.
- Advance the formula in the public tap only after its clean install, true
  prior-version upgrade, uninstall, and reinstall tests pass on both supported
  macOS architectures. A desktop-enabled release also gates cask publication on
  those lifecycle checks, the app signature, Gatekeeper, the stapled
  notarization ticket, and a real launch/quit cycle.
- Keep the lifecycle smoke workflow green on the current GitHub-hosted Intel and
  Apple Silicon macOS runners.

Every release must include:

- tag → commit provenance mapping,
- workflow run identifier,
- checksums, and
- architecture marker in filename.

Every release requires this repository Actions secret:

- `HOMEBREW_TAP_TOKEN`, scoped to update `delaudio/homebrew-tap`;

The command-only channel is the default when the repository Actions variable
`NORN_DESKTOP_RELEASE_ENABLED` is absent or is not exactly `true`. To publish a
desktop release, set that variable to `true` and configure all of these
additional repository Actions secrets:

- `APPLE_CERTIFICATE`, a base64-encoded Developer ID Application certificate;
- `APPLE_CERTIFICATE_PASSWORD` and `KEYCHAIN_PASSWORD`;
- `APPLE_SIGNING_IDENTITY`;
- `APPLE_ID`, `APPLE_PASSWORD`, and `APPLE_TEAM_ID` for notarization.

When a channel has no earlier complete stable release for both architectures,
set the repository Actions variable `NORN_HOMEBREW_BOOTSTRAP_TAG` to the exact
candidate tag. The command channel still checks clean install, uninstall, and
reinstall. An enabled desktop channel additionally checks app launch,
Gatekeeper, and local data and Keychain preservation. Remove the variable after
publication. Later releases fail closed unless they can perform a real upgrade
from the newest complete stable release; a stale bootstrap value cannot
authorize a different tag.

Signing material is imported into an ephemeral CI keychain and removed after the
build. Credentials must not be written to manifests, release assets, metadata,
or logs. When desktop is enabled, a missing secret or any unsigned, unnotarized,
or incomplete desktop architecture fails the release. Any command or enabled
desktop lifecycle failure leaves the candidate as a prerelease and keeps the
existing public tap entries unchanged. A command-only release does not create or
modify a cask entry.

Release workflows are serialized. Tap publication deliberately uses a normal
fast-forward push and never rebases a generated manifest commit over a concurrent
tap update. An unexpected non-fast-forward push therefore fails closed; inspect
the intervening tap change and rerun the failed job only if the candidate is
still the intended latest stable release.

You can manually trigger lifecycle smoke validation from this repo:

```sh
gh workflow run homebrew-lifecycle-smoke.yml
```
