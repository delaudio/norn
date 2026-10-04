# Tauri Provider Boundary Basics

A small public example pack for repositories that use Tauri with a hosted code
review provider. It demonstrates prompt rules, a native path rule, a Rust AST
rule declaration, and analyzer defaults.

## Install Locally

```yaml
# .norn.yaml
version: 0.1

policy:
  packs:
    - ./examples/policy-packs/bitbucket-tauri-basic
```

## What It Covers

- `boundary.provider-http-in-rust`: provider REST calls from the webview.
- `boundary.no-secrets-in-frontend`: secrets reaching frontend state or config.
- `boundary.ipc-command-mock-parity`: command and mock-IPC drift.
- `boundary.rust-error-mapping`: native errors leaking sensitive detail.
- `boundary.rust-no-unwrap-in-commands`: `unwrap()` in command paths.

Analyzer defaults enable `typecheck` and `tests` and leave `lint` optional.

## Adapting It

Copy `pack.yaml` into a private repository or an internal bundle and keep the
rule ids stable. Replace analyzer commands with the checks your repository
already runs. Keep customer names, private paths, credentials, and model
transcripts out of committed rules.

## Files

- `pack.yaml` - loadable policy pack manifest.
