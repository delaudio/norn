# Local source installation

Homebrew is the default end-user installation path. The local installer is for
developers who intentionally build Norn from a source checkout and want the
result to remain usable after build output or the checkout is removed.

## Install the OpenTUI command distribution

Build prerequisites: Rust, pnpm and Bun 1.3.8. Run `pnpm install` and
`bun install --cwd ui --frozen-lockfile` once. The installed client embeds its
runtime; users do not need Bun or Node after installation.

On macOS or Linux:

```sh
make install-local
```

On Windows:

```powershell
just install-local
```

The default prefix is `~/.local`, so commands are installed as real executable
files in `~/.local/bin`. A different absolute prefix can be selected without
editing the repository:

```sh
NORN_INSTALL_PREFIX=/absolute/prefix make install-local
```

The complete install contains:

- `norn`: Rust CLI; a bare terminal invocation opens OpenTUI. Noninteractive
  zero-argument invocation prints help. CLI subcommands need no JS runtime.
- `norn-tui`: explicit OpenTUI launcher.
- `norn-backend`: private Rust stdio host, launched by the UI.
- `norn-ui`: standalone compiled OpenTUI client and owned syntax subprocess.
- `lachesi`: deprecated CLI compatibility alias.
- `lac`: deprecated TUI compatibility alias.

The compatibility aliases remain through six stable Norn releases and are
removed no earlier than the following major release, after fresh-install and
upgrade coverage passes.

`make cli-install` and `make tui-install` both install this complete runtime;
matching `just` recipes have the same names. The launcher resolves the UI and
backend beside its own executable, preserving the caller's repository directory.
Keep them together. The installer also copies `share/norn/browser-diff`, required
for the `g` → `b` browser viewer. Existing settings, profiles and stored data are
not removed or rewritten by the installer.

Ordinary scripts never enable Tauri, Ratatui or `custom-protocol`. The optional
legacy desktop and terminal adapters remain available via
`pnpm run legacy:desktop:build` and `pnpm run legacy:tui`; `norn-app` is excluded
from ordinary build/install. Existing separately installed legacy launchers are
not automatically deleted. Final source removal is a later migration step.

## Replacement guarantees

The installer copies every selected build output into a staging directory under
the destination `bin` directory and runs its bounded `--version` check before
changing the active installation. It then renames the staged files into place.
If a copy, verification, or replacement fails, it restores the prior selected
commands. A successful install therefore does not depend on checkout-relative
symbolic links and remains available after `src-tauri/target/release` is cleaned.

## Distinguish source and Homebrew installations

Check the command that a clean shell resolves:

```sh
command -v norn
brew --prefix
```

A default source install resolves to `~/.local/bin/norn`. A Homebrew formula
install resolves below `$(brew --prefix)/bin`. Avoid installing both into the
same prefix; remove the source-installed files before switching that prefix to
Homebrew ownership.
