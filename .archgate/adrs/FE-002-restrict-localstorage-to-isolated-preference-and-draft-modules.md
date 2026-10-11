---
id: FE-002
title: Restrict localStorage to isolated preference and draft modules
domain: frontend
rules: true
---

# Restrict localStorage to isolated preference and draft modules

## Context

Lachesi uses browser storage for a small set of user-local concerns while running outside Tauri and inside the webview:

- theme preference in `src/hooks/useTheme.ts`
- per-repo AI review prompt overrides in `src/lib/reviewPrompt.ts`
- staged draft comments in `src/hooks/useDraftComments.ts`
- per-PR review references (Jira/Notion links) in `src/lib/reviewReferencesStorage.ts`
- per-PR viewed-file state in `src/lib/viewedFilesStorage.ts`
- menu-bar pull request notification snapshots in `src/lib/menuBarPrSnapshotStorage.ts`
- collapsed repository state in `src/components/pr-sidebar/PrSidebar.tsx`
- mock IPC review persistence for browser dev/Storybook mode in `src/mock-tauri/mock-handlers.ts`

Those cases are intentionally local, user-scoped, and non-secret. They are also isolated behind small modules with explicit storage keys and graceful failure handling.

The risk is not the existing usage itself; it is the tendency for `localStorage` to spread opportunistically into unrelated UI code for state that should instead live in React state, config, or native storage.

## Decision

Lachesi will restrict direct `localStorage` access to a small, explicit set of modules that own local-only preferences or staged review state.

Today the allowed modules are:

- `src/hooks/useTheme.ts`
- `src/lib/reviewPrompt.ts`
- `src/hooks/useDraftComments.ts`
- `src/lib/reviewReferencesStorage.ts`
- `src/lib/viewedFilesStorage.ts`
- `src/lib/menuBarPrSnapshotStorage.ts`
- `src/components/pr-sidebar/PrSidebar.tsx`
- `src/mock-tauri/mock-handlers.ts`

Any new direct `localStorage` usage outside those modules requires an ADR update or a deliberate exception.

Durable, structured, or application-wide data should not use `localStorage` at all — it belongs in native, Tauri-backed persistence. Readest (the reference app for this work) keeps durable data behind a `FileSystem` abstraction with an atomic-save persistence helper and localStorage only for lightweight, ephemeral UI preferences; Lachesi already follows this shape by keeping saved reviews in `~/.local/share/lachesi/` via Rust (see `CLAUDE.md`).

Components and feature hooks must call these owner modules instead of performing ad hoc inline
storage reads/writes.

## Do's and Don'ts

### Do

- Keep `localStorage` access encapsulated in small owner modules
- Use namespaced keys for stored values
- Handle storage failures defensively
- Prefer native config or Tauri-backed persistence for cross-device, secret, or application-wide configuration

### Don't

- Don't access `localStorage` ad hoc from arbitrary components
- Don't store credentials, tokens, or other secrets in browser storage
- Don't use `localStorage` as a generic substitute for application state management
- Don't introduce new storage keys without a clear owner module

## Consequences

### Positive

- Browser persistence remains easy to reason about
- Secret or app-level state is less likely to leak into web storage
- Governance can catch storage sprawl early with a simple rule

### Negative

- New persistence use cases require explicit design rather than quick inline storage calls
- Some small UX conveniences may feel slower to implement because they need a clear owner

### Risks

- The allowlist must be updated when a genuinely new local-only persistence case is introduced
- Tests that intentionally touch `localStorage` may need to reuse existing storage owners instead of adding bespoke access sites

## Compliance and Enforcement

An automated rule enforces one high-confidence invariant:

- direct `localStorage` access in `src/` is only allowed in the approved owner modules

Code review should still reject broader violations that are not yet machine-checked, such as:

- poorly named storage keys
- long-lived data kept in browser storage when native config would be more appropriate

## References

- `src/hooks/useTheme.ts`
- `src/lib/reviewPrompt.ts`
- `src/hooks/useDraftComments.ts`
- `src/lib/reviewReferencesStorage.ts`
- `src/lib/viewedFilesStorage.ts`
- `src/lib/menuBarPrSnapshotStorage.ts`
- `src/components/pr-sidebar/PrSidebar.tsx`
- [Route native calls through typed frontend service modules above tauriCall](./FE-003-route-native-calls-through-typed-frontend-service-modules.md)

## Temporary migration export exception (2026-10-11)

`src/lib/desktopDataExport.ts` may read the existing allowlisted user-content
keys for the explicit desktop export action under migration issue #305. It
creates no storage keys, performs no storage writes/deletes and excludes credentials
and caches. Components call that owner module. The automated rule additionally
rejects storage mutation calls in this module. Bounded synthetic export tests
prove exclusion, no source writes and failure on changing/oversized input. This
exception remains only until the desktop adapter is retired; canonical new
persistence is Rust-owned.
