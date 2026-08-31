# Local source installation

Homebrew is the default end-user installation path. The local installer is for
developers who intentionally build Norn from a source checkout and want the
result to remain usable after build output or the checkout is removed.

## Install every command

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

- `norn`: canonical command-line review and repository interface.
- `norn-tui`: canonical terminal interface.
- `norn-app`: desktop launcher; `norn-app --version` is non-interactive.
- `lachesi`: deprecated CLI compatibility alias.
- `lac`: deprecated TUI compatibility alias.

The compatibility aliases remain through six stable Norn releases and are
removed no earlier than the following major release, after fresh-install and
upgrade coverage passes.

Use `make cli-install` to install only `norn`, `norn-app`, and `lachesi`, or
`make tui-install` to install only `norn-tui` and `lac`. The matching `just`
recipes have the same names.

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
