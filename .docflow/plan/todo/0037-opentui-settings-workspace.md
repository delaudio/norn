# Terminal Settings and Onboarding

## Owning ADRs

- `../../adr/0020-opentui-rust-stdio-workspace.md` (AC 3, 5, 6).
- `../../adr/0003-credentials-keychain.md`.

## Scope

Expose non-secret Rust-owned settings through typed read/save methods with
version fencing and atomic canonical writes. Add terminal forms for tracked
repositories, review provider, AI provider/model/effort including arbitrary custom
models, display preferences and existing integration preferences. Reuse credential
status and explicit writes through Rust, with masked draft input and no secret
readback. Saving credentials is a separate explicit operation from preferences;
partial failure must not pretend everything saved. Existing configured users
continue to use their local settings and OS credentials. First-run empty state
provides a settings entry and the existing Rust setup/auth route.

Desktop-local draft/reference/viewed/prompt export/import is a subsequent delivery
under #305; no legacy data is deleted or old interface retired in this item.

## Exit Criteria

1. Read/save methods use validated bounded non-secret schemas, optimistic version
   checks and atomic config persistence. Arbitrary supported custom model IDs and
   unrelated settings survive round trips; canonical writes preserve legacy input.
2. Forms support focus/edit/paste, masked secret input, cancel/discard/save and
   visible load/save errors without exposing secret values in status, logs or frames.
3. Provider credentials remain in the existing OS credential store/environment;
   no plaintext fallback is introduced. Presence/status and explicit secret writes
   are Rust-owned. Platform gaps stay tracked against #251.
4. Isolated config/fake-keychain unit tests prove validation, stale saves, cancel,
   custom models and no secret readback; deterministic stdio and UI/frame gates pass.
5. Root/UI/Rust/Archgate gates pass; #305 remains open until data recovery ships.

## Dependencies

- #301 merged. Integrate after PR #338 to keep the delivery series ordered.

## Status

- Claimed by: default-agent, 2026-10-11, `issue-305-settings-workspace`.
- Blockers: None; PR #338 merged at `3168dcd`.
- Stopped:
