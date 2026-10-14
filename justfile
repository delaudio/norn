# Norn task runner (Windows).
# macOS/Linux contributors use the parallel `Makefile`; recipe names are kept in
# parity across both files per ADR ARCH-007. Recipes delegate to the canonical
# package.json / cargo / tauri commands so package.json stays authoritative.

# Run recipes through the native Windows PowerShell instead of just's default
# `sh` (which is not present on a standard Windows machine). See ADR ARCH-007.
# -ExecutionPolicy Bypass applies to this spawned process only (no machine-wide
# change) so PowerShell script shims like pnpm.ps1 run under a Restricted policy.
set windows-shell := ["powershell.exe", "-NoLogo", "-NoProfile", "-ExecutionPolicy", "Bypass", "-Command"]

# List available recipes (runs by default).
help:
    @just --list

# Start OpenTUI with the headless backend and browser diff assets.
dev:
    pnpm run dev

# Legacy only: start the retained Tauri app (explicit desktop feature).
# Requires BITBUCKET_USERNAME and BITBUCKET_TOKEN in the environment.
tauri-dev:
    pnpm tauri dev --features desktop-bundle

# Build and install Rust CLI/backend, standalone OpenTUI and browser diff.
install-local:
    pnpm run install:local

# Build the canonical headless CLI and its deprecated compatibility alias.
cli-build:
    pnpm run cli:build

# Install the complete OpenTUI command distribution (no desktop launcher).
cli-install:
    pnpm run cli:install

# Start the terminal UI.
tui:
    pnpm run tui

# Build the terminal UI release binary.
tui-build:
    pnpm run tui:build

# Install the complete OpenTUI command distribution and compatibility aliases.
tui-install:
    pnpm run tui:install

# Build Rust CLI/backend, standalone OpenTUI and browser diff.
build:
    pnpm run build

# TypeScript typecheck only.
typecheck:
    pnpm run typecheck

# Biome lint.
lint:
    pnpm run lint

# Vitest run.
test:
    pnpm run test

# Rust IPC smoke / parity test lane (ARCH-005).
test-tauri:
    pnpm run test:tauri

# OpenTUI client typecheck (Bun).
ui-typecheck:
    pnpm run ui:typecheck

# OpenTUI client tests (Bun).
ui-test:
    pnpm run ui:test

# Native terminal frame visual regression (Bun).
ui-frames-check:
    pnpm run ui:frames:check

# Offline review-quality corpus gate.
evaluate:
    pnpm run evaluate

# Archgate ADR compliance check.
check:
    pnpm run version:verify
    archgate check

# Alias for the release version alignment check.
check-versions:
    pnpm run version:verify

# Build the Windows distributable: the NSIS setup .exe (ARCH-008).
# Output: src-tauri/target/release/bundle/nsis/Norn_<version>_x64-setup.exe
bundle-windows:
    pnpm tauri build --bundles nsis --features custom-protocol,desktop-bundle

# Exercise the native UI with the Rust backend and an isolated fake agent.
ui-native-workflow:
    pnpm run ui:workflow

# Approved Git operations using disposable clones and loopback-only bare remotes.
ui-repository-operations:
    pnpm run ui:repository-operations

# Explicit legacy desktop build; never part of ordinary build/install.
legacy-desktop-build:
    pnpm run legacy:desktop:build

# Explicit archived Ratatui terminal frontend.
legacy-tui:
    pnpm run legacy:tui

# Verify the default Rust dependency graph contains no GUI/terminal toolkit.
core-no-gui:
    node scripts/verify-core-no-gui-deps.mjs
