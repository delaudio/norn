---
id: ARCH-007
title: Drive repository commands through platform-native task runners
domain: architecture
rules: true
files:
  - "Makefile"
  - "justfile"
  - "package.json"
---

# Drive repository commands through platform-native task runners

## Context

Lachesi is a Tauri desktop app that is developed on both Unix (macOS/Linux) and Windows. The real command surface lives in `package.json` scripts (`dev`, `build`, `typecheck`, `lint`, `test`, `test:tauri`, `tauri`) plus a handful of native commands (`cargo test --manifest-path src-tauri/Cargo.toml ...`, `tauri build`, credential env setup). Contributors currently memorize and retype these long invocations, and several of them differ by platform:

- setting credential environment variables uses `export VAR=...` on Unix but a different mechanism on Windows
- launching the full Tauri app requires `BITBUCKET_USERNAME`/`BITBUCKET_TOKEN` to be present in the shell environment first (see `CLAUDE.md`)
- native build output (bundling) is platform-specific (see [ARCH-008: Build and distribute the Windows app as an NSIS installer](./ARCH-008-build-and-distribute-the-windows-app-as-an-nsis-installer.md))

Without a task-runner convention, three problems appear:

1. **Command drift** — developers copy-paste long `pnpm`/`cargo`/`tauri` invocations from chat history or memory, and typos silently run the wrong lane.
2. **Platform divergence** — a single shell-script entrypoint cannot express both `export VAR=...` (POSIX `sh`/`zsh`) and Windows-native shell syntax cleanly; scripts that assume one shell break on the other.
3. **Onboarding cost** — new contributors have no single, discoverable list of "what can I run in this repo."

Alternatives considered:

- **`package.json` scripts alone**: They already exist and stay cross-platform for pure Node tooling, but they cannot cleanly express platform-native shell steps (credential exports, chained native builds, OS-specific bundling) without brittle cross-env shims, and they are not the idiomatic home for native `cargo`/`tauri` orchestration.
- **A single `Makefile` for everyone**: `make` is not installed by default on Windows and its recipes assume a POSIX shell; forcing Windows developers onto `make` via WSL or MSYS adds a heavy, non-native dependency.
- **A single `justfile` for everyone**: [`just`](https://github.com/casey/just) is cross-platform and pleasant, but it is an extra install that the existing Unix contributors do not currently need, and `make` is already ubiquitous on macOS/Linux developer machines.
- **A shell-script `scripts/` directory**: Duplicates logic per platform with no shared vocabulary and no discoverability (`make`/`just` both list recipes; loose scripts do not).

For Lachesi, the pragmatic decision is to give each platform its **native** task runner while keeping a **single shared vocabulary of recipe names**. Unix contributors use the `make` they already have; Windows contributors use `just`, which installs cleanly via `winget`/`scoop`/`cargo` and does not depend on a POSIX shell. Both wrap the same underlying `pnpm`/`cargo`/`tauri` commands so behavior is identical regardless of which runner a contributor invokes.

## Decision

Repository commands MUST be driven through a platform-native task runner, and the two runners MUST expose the same set of recipe names:

- **`just` (a root `justfile`) is the task runner for Windows** and expresses Windows-native shell/command steps.
- **`make` (a root `Makefile`) is the task runner for macOS/Linux** and expresses POSIX-shell steps.
- Both files live at the repository root.
- Both files MUST define the **same set of recipe/target names** — the recipe name is the stable, cross-platform contract; the body may differ per platform.
- Recipes MUST delegate to the canonical commands (`pnpm run <script>`, `cargo ...`, `pnpm tauri ...`) rather than re-implementing their logic, so `package.json` remains the source of truth for the underlying tooling.
- The `justfile` MUST configure a **native Windows shell** with `set windows-shell := ["powershell.exe", "-NoLogo", "-NoProfile", "-ExecutionPolicy", "Bypass", "-Command"]` at the top of the file. It MUST NOT rely on `just`'s default `sh` shell, which is absent on a standard Windows machine and causes every recipe to fail with `could not find the shell 'sh'`. Windows recipes therefore run as native PowerShell, not POSIX-`sh`-wrapped commands. `-ExecutionPolicy Bypass` applies to the spawned process only (no machine-wide change) so PowerShell script shims such as `pnpm.ps1` run even under a `Restricted`/`AllSigned` policy; `-NoProfile` keeps the runner hermetic.

Scope: this ADR governs the **developer-facing command entrypoints** for the main repository. It does not replace `package.json` scripts (which remain the canonical definition of Node tooling), and it does not govern CI pipeline definitions.

The shared recipe surface MUST cover at least the everyday lanes:

- `dev` — start the Vite dev server (browser mock IPC)
- `tauri-dev` — start the full Tauri app (sets/loads the Bitbucket credential env first)
- `build` — typecheck + Vite build
- `typecheck` — `tsc --noEmit`
- `lint` — Biome check
- `test` — Vitest run
- `test-tauri` — the Rust IPC smoke lane (see [ARCH-005](./ARCH-005-tauri-ipc-smoke-and-parity-test-lane.md))
- `check` — run `archgate check`

## Do's and Don'ts

### Do

- **DO** add every new developer command as a recipe in **both** the `Makefile` and the `justfile`, using the identical recipe name.
- **DO** keep both runners at the repository root so `make <recipe>` and `just <recipe>` are discoverable.
- **DO** have each recipe delegate to the canonical command (`pnpm run <script>`, `cargo ...`, `pnpm tauri ...`) so `package.json` stays authoritative.
- **DO** express platform-specific steps (credential env exports, native bundling) inside the platform's own runner using that platform's native shell.
- **DO** configure `set windows-shell := ["powershell.exe", "-NoLogo", "-NoProfile", "-ExecutionPolicy", "Bypass", "-Command"]` at the top of the `justfile` so recipes run in native PowerShell rather than the missing `sh`.
- **DO** keep recipe bodies shell-agnostic where possible (calling `pnpm`/`cargo`/`pnpm tauri`) so both PowerShell (Windows `just`) and `sh` (Unix `make`) execute them correctly.
- **DO** update both files in the same change whenever a recipe is added, renamed, or removed.
- **DO** name recipes with lowercase kebab-case identifiers that start at column 0 (e.g. `tauri-dev:`) so they list cleanly and parse consistently across both runners.

### Don't

- **DON'T** add a recipe to only one runner — a `make` target with no matching `just` recipe (or vice versa) is a parity violation.
- **DON'T** mandate `make` on Windows or `just` on macOS/Linux; each platform uses its native runner.
- **DON'T** re-implement command logic inside a recipe when a `package.json` script already defines it — call the script instead.
- **DON'T** introduce a `scripts/*.sh` or `scripts/*.ps1` entrypoint as an alternative command surface; consolidate on the two task runners.
- **DON'T** use divergent recipe names for the same lane across platforms (e.g. `tauri-dev` in one file and `dev-tauri` in the other).
- **DON'T** hardcode secrets in either runner — read credentials from the environment or keychain per [ARCH-001](./ARCH-001-tauri-react-rust-bitbucket-boundary.md).
- **DON'T** rely on `just`'s default `sh` shell in the `justfile` — `sh` is not installed on a standard Windows machine and recipes will fail with `could not find the shell 'sh'`.
- **DON'T** write POSIX-`sh`-specific syntax (e.g. `$$VAR` expansion, `&&` chains assuming `sh`, backtick command substitution) in `justfile` recipe bodies; use PowerShell-native or shell-agnostic commands.

## Consequences

### Positive

- **Native experience:** Each platform uses a task runner that works with its native shell — no WSL/MSYS requirement on Windows and no extra install for existing Unix contributors.
- **Discoverability:** `make` and `just` both list available recipes, giving new contributors a single catalogue of repository commands.
- **Stable vocabulary:** The shared recipe names form a platform-agnostic contract; documentation and muscle memory transfer across platforms.
- **Single source of truth preserved:** Recipes delegate to `package.json`/`cargo`/`tauri`, so the underlying command definitions are not duplicated or forked.
- **Cleaner platform steps:** Credential env setup and native bundling live in the runner idiomatic for that OS.

### Negative

- **Two files to maintain:** Every command change must be mirrored in both the `Makefile` and the `justfile`.
- **Two tools to learn:** Contributors who switch platforms must know both `make` and `just`, even if they only use one at a time.
- **Extra install on Windows:** Windows contributors must install `just` (via `winget`, `scoop`, or `cargo install just`).

### Risks

- **Recipe drift between runners:** The two files can fall out of sync. **Mitigation:** an automated `archgate` rule enforces that both files expose the identical set of recipe names, failing the check on any mismatch.
- **Logic divergence in recipe bodies:** A recipe could behave differently per platform if bodies re-implement logic. **Mitigation:** the ADR mandates delegation to canonical `package.json`/`cargo`/`tauri` commands so bodies stay thin, and code review verifies parity of behavior.
- **Runner files missing entirely:** A fresh clone may lack one or both files before this decision is implemented. **Mitigation:** the rule warns (non-blocking) when neither file exists and escalates to a hard violation once one exists without its counterpart.

## Implementation Pattern

Both runners expose the same recipe names; only the bodies differ by platform.

`Makefile` (macOS/Linux):

```makefile
.PHONY: dev tauri-dev build typecheck lint test test-tauri check

dev:
	pnpm run dev

tauri-dev:
	pnpm tauri dev --features desktop-bundle

build:
	pnpm run build

typecheck:
	pnpm run typecheck

lint:
	pnpm run lint

test:
	pnpm run test

test-tauri:
	pnpm run test:tauri

check:
	archgate check
```

`justfile` (Windows):

```just
# Native Windows shell — recipes run in PowerShell, not the missing `sh`.
set windows-shell := ["powershell.exe", "-NoLogo", "-NoProfile", "-ExecutionPolicy", "Bypass", "-Command"]

dev:
    pnpm run dev

tauri-dev:
    pnpm tauri dev --features desktop-bundle

build:
    pnpm run build

typecheck:
    pnpm run typecheck

lint:
    pnpm run lint

test:
    pnpm run test

test-tauri:
    pnpm run test:tauri

check:
    archgate check
```

The recipe **names** (`dev`, `tauri-dev`, `build`, `typecheck`, `lint`, `test`, `test-tauri`, `check`) are identical across both files — that parity is the enforced contract.

## Compliance and Enforcement

Automated enforcement (companion `ARCH-007-...rules.ts`):

- **`task-runner-parity`** verifies that the root `Makefile` and root `justfile` expose the identical set of recipe/target names. Any name present in one but not the other fails the check.
- **`task-runner-files-present`** warns (non-blocking) when neither runner exists yet, and reports a hard violation when exactly one exists without its counterpart.
- **`justfile-uses-native-windows-shell`** verifies that the root `justfile` declares a `set windows-shell` (or `set shell`) directive, so Windows recipes never fall back to the missing default `sh` shell.

Manual enforcement (code review):

- Reviewers MUST confirm any command change touches both the `Makefile` and the `justfile` with matching recipe names.
- Reviewers MUST reject new `scripts/*.sh` / `scripts/*.ps1` command entrypoints that bypass the task runners.
- Reviewers MUST confirm recipe bodies delegate to canonical commands rather than re-implementing logic.

Exceptions MUST be approved by the lead architect and documented as a separate ADR.

## References

- [Use a Tauri desktop shell with a React webview and a Rust Bitbucket client](./ARCH-001-tauri-react-rust-bitbucket-boundary.md)
- [Maintain a Tauri IPC smoke and parity test lane separate from jsdom tests](./ARCH-005-tauri-ipc-smoke-and-parity-test-lane.md)
- [Build and distribute the Windows app as an NSIS installer](./ARCH-008-build-and-distribute-the-windows-app-as-an-nsis-installer.md)
- `package.json` (canonical script definitions)
- `CLAUDE.md` (credential env requirements for `pnpm tauri dev`)
- [just — a command runner](https://github.com/casey/just)
- [GNU Make](https://www.gnu.org/software/make/)
