---
title: Getting Started
description: Install dependencies and run Norn locally.
---

## Requirements

- Node.js and pnpm.
- Rust toolchain.
- Tauri v2 prerequisites for your operating system.
- Bitbucket Cloud API token and/or GitHub token for real provider usage.
- Claude CLI and/or Codex CLI for AI review flows.

## Install

```sh
pnpm install
```

## Run In Browser Mode

```sh
pnpm dev
```

This starts the Vite app with mock IPC handlers.

## Run The Desktop App

```sh
pnpm tauri dev --features desktop-bundle
```

Use the settings screen to configure repositories, credentials, local clone paths, and AI provider options.

## Build

```sh
pnpm build
pnpm tauri build --features custom-protocol,desktop-bundle
```

## Test

```sh
pnpm test
pnpm test:tauri
```

`pnpm test` runs the browser/jsdom Vitest suite. `pnpm test:tauri` runs the separate Tauri IPC smoke
lane against a mock Tauri webview and low-risk Rust commands, without publishing comments or
mutating remote providers.
