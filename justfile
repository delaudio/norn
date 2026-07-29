# Lachesi task runner (Windows).
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

# Start the Vite dev server (browser mock IPC).
dev:
    pnpm run dev

# Start the full Tauri app (real IPC).
# Requires BITBUCKET_USERNAME and BITBUCKET_TOKEN in the environment.
tauri-dev:
    pnpm tauri dev

# Start the terminal UI.
tui:
    pnpm run tui

# Build the terminal UI release binary.
tui-build:
    pnpm run tui:build

# Build and install the terminal UI as `lac` in ~/.local/bin.
tui-install:
    pnpm run tui:install

# Typecheck + Vite production build.
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

# Offline review-quality corpus gate.
evaluate:
    pnpm run evaluate

# Archgate ADR compliance check.
check:
    archgate check

# Build the Windows distributable: the NSIS setup .exe (ARCH-008).
# Output: src-tauri/target/release/bundle/nsis/Lachesi_<version>_x64-setup.exe
bundle-windows:
    pnpm tauri build --bundles nsis
