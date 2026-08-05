# Norn task runner (macOS/Linux).
# Windows contributors use the parallel `justfile`; recipe names are kept in
# parity across both files per ADR ARCH-007. Recipes delegate to the canonical
# package.json / cargo / tauri commands so package.json stays authoritative.

.DEFAULT_GOAL := help
.PHONY: help dev tauri-dev cli-build cli-install tui tui-build tui-install build typecheck lint test test-tauri evaluate check bundle-windows

# List available recipes (runs by default).
help:
	@echo "Norn recipes: dev tauri-dev cli-build cli-install tui tui-build tui-install build typecheck lint test test-tauri evaluate check bundle-windows"

# Start the Vite dev server (browser mock IPC).
dev:
	pnpm run dev

# Start the full Tauri app (real IPC).
# Uses credentials from the OS keychain; if none are stored, BITBUCKET_USERNAME and BITBUCKET_TOKEN env vars are used as a dev fallback.
tauri-dev:
	pnpm tauri dev

# Build the canonical headless CLI and its deprecated compatibility alias.
cli-build:
	pnpm run cli:build

# Install `norn` and the deprecated `lachesi` alias in ~/.local/bin.
cli-install:
	pnpm run cli:install

# Start the terminal UI.
tui:
	pnpm run tui

# Build the terminal UI release binary.
tui-build:
	pnpm run tui:build

# Build and install the terminal UI as `norn-tui` in ~/.local/bin.
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
	pnpm run version:verify
	archgate check

# Alias for the release version alignment check.
check-versions:
	pnpm run version:verify

# The Windows NSIS installer must be built on Windows (ARCH-008).
# This target exists for recipe parity; run `just bundle-windows` on Windows.
bundle-windows:
	@echo "Windows NSIS installer must be built on Windows: run 'just bundle-windows' (see ADR ARCH-008)."
