# Norn task runner (macOS/Linux).
# Windows contributors use the parallel `justfile`; recipe names are kept in
# parity across both files per ADR ARCH-007. Recipes delegate to the canonical
# package.json / cargo / tauri commands so package.json stays authoritative.

.DEFAULT_GOAL := help
.PHONY: help dev tauri-dev install-local cli-build cli-install tui tui-build tui-install build typecheck lint test test-tauri ui-typecheck ui-test ui-native-workflow ui-repository-operations ui-frames-check evaluate check bundle-windows legacy-desktop-build legacy-tui core-no-gui

# List available recipes (runs by default).
help:
	@echo "Norn recipes: dev tauri-dev install-local cli-build cli-install tui tui-build tui-install build typecheck lint test test-tauri ui-typecheck ui-test ui-native-workflow ui-repository-operations ui-frames-check evaluate check bundle-windows legacy-desktop-build legacy-tui core-no-gui"

# Start OpenTUI with the headless backend and browser diff assets.
dev:
	pnpm run dev

# Legacy only: start the retained Tauri app (explicit desktop feature).
# Uses credentials from the OS keychain; if none are stored, BITBUCKET_USERNAME and BITBUCKET_TOKEN env vars are used as a dev fallback.
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

# The Windows NSIS installer must be built on Windows (ARCH-008).
# This target exists for recipe parity; run `just bundle-windows` on Windows.
bundle-windows:
	@echo "Windows NSIS installer must be built on Windows: run 'just bundle-windows' (see ADR ARCH-008)."

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
