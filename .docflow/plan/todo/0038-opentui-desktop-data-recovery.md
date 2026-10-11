# Desktop user-data recovery and Rust ownership

## Owning ADRs

- `../../adr/0020-opentui-rust-stdio-workspace.md` (AC 5, 6).

## Scope

Define bounded versioned Rust-owned drafts, review references, viewed-file state
and user-prompt persistence. Export allowlisted user data from the existing desktop
webview without credentials or transient caches. Import via explicit preview and
confirmation with a backup, stable identity, idempotence, conflict reporting and
no implicit overwrites. Preserve canonical and legacy source profiles/config.
Provide verified recovery instructions for already-upgraded users on supported
platforms, including how to obtain the export bridge without requiring the old
app to have been opened before upgrade. Coordinate draft schema with #304.

## Exit Criteria

1. A typed bounded versioned export/import format preserves all shipped user-data
   fields and reports malformed/ambiguous entries instead of silently discarding
   them. Provider-less historical scope requires explicit disambiguation.
2. Rust owns canonical persistent records. Preview reads without mutation;
   confirmation is bound to input and destination versions. Atomic import keeps
   existing conflicts by default, backs up changed destination data, and repeated
   import neither duplicates nor silently replaces newer values.
3. The working desktop has an explicit allowlisted export action. Platform recovery
   routes are documented and tested with synthetic storage/export fixtures; no
   test reads real credentials or developer webview profiles.
4. Interrupted/stale/conflicting/oversized/malformed imports preserve originals;
   protocol, CLI, UI/frame, root and Rust gates pass as applicable.
5. All legacy data remains intact. Owning issue/decision advance only after actual
   complete parity and verified merge, not after this partial delivery.

## Dependencies

- #301 merged; integrate after PR #339 (plan 0037) passes CI and merges.

## Status

- Claimed by: default-agent, 2026-10-11, `issue-305-desktop-data-recovery`.
- Blockers: PR #339 pending CI; read-only inventory and preparation proceed.
- Stopped:
