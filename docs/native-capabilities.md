# Native capability ownership

Norn keeps its Tauri permission surface intentionally small. The desktop
webview talks to the backend through typed commands centralized in
`src/lib/tauri.ts`; it does not receive broad native access. Provider HTTP and
credential handling stay in Rust behind those commands.

This document records which permissions are currently enabled, which product
capability owns each one, and the criteria reviewers apply before expanding the
surface.

## Current capabilities

The capability set lives in `src-tauri/capabilities/default.json` and applies
to the `main` window. Plugins are registered in `src-tauri/src/lib.rs`.

| Permission | Owner | Rationale |
|---|---|---|
| `core:default` | Desktop shell and IPC | Baseline window, event, path, and IPC access every window needs. It grants no direct filesystem, shell, or process execution. |
| `opener:default` | External link opening (`openExternal` in `src/lib/tauri.ts`) | Opens review and documentation links in the user's default browser. It is not used to open arbitrary local files. |
| `notification:default` | Desktop notifications (`src/App.tsx`, `src/hooks/useMenuBarPrSync.ts`) | Surfaces review-sync notifications. The app checks and requests the OS permission at runtime and degrades silently when denied. |

If a permission is not listed here, treat it as intentionally absent.

## Adding a new permission

Before adding a permission, document all of the following in the change or the
requesting issue:

1. **Product need** — the user-visible capability that requires it, with the
   owning issue or spec.
2. **Scope** — the narrowest permission set that satisfies the need. Prefer a
   specific plugin permission over a bundle, keep it scoped to the window that
   needs it, and avoid wildcard paths or hosts.
3. **Failure behavior** — what happens when the permission is denied, missing,
   or unsupported on a platform. Native failures must degrade to an actionable
   message, not a silent no-op or a crash.
4. **Verification** — the smoke test or manual procedure that proves the
   capability works, and which platform it runs on. Add automated coverage when
   the behavior can be exercised without a real OS prompt.
5. **Ownership** — the module or service that owns the capability and would
   remove the permission if the feature is dropped.

A permission that cannot name a concrete owner and a failure path is not ready
to add.

## Security notes by permission category

- **Filesystem**: the reviewed repository, local config, and app data live in
  Rust. Only add scoped filesystem access when a feature genuinely needs it,
  and never grant broad roots.
- **Shell / process**: executed analyzers run through Rust commands with
  explicit, repository-provided commands. Do not expose shell execution to the
  webview, and keep output free of secrets.
- **External URL**: `openExternal` is the only intended external-navigation
  path. Validate or construct URLs from trusted data before opening them, and
  never place provider tokens in a URL.
- **Notifications**: treat bodies as user-visible. Do not include secrets,
  tokens, private paths, or customer identifiers in notification text.

## Provider data and credentials

Provider HTTP, tokens, and credentials never cross into webview permissions.
They stay behind Rust commands and the OS credential store. Adding a permission
must not create a webview path to provider APIs or to persisted secrets.

## Reference

- Capability config: `src-tauri/capabilities/default.json`
- Plugin registration: `src-tauri/src/lib.rs`
- Typed IPC entry point: `src/lib/tauri.ts`
