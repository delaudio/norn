# Norn Local Data Migration

## Owning ADRs

- `../../adr/0011-norn-naming-and-compatibility.md`
- `../../adr/0003-credentials-keychain.md`

## Scope

Implement GitHub issue #176 by moving settings, terminal configuration,
SQLite review history, browser-local preferences, keychain references, and
environment-variable identifiers to canonical Norn names. Legacy sources stay
readable during the compatibility window, canonical inputs take precedence,
and successful migration never deletes the recoverable source.

## Exit Criteria

- ADR 0011 AC3: canonical `NORN_*` inputs win and legacy environment names are
  used only as fallbacks.
- ADR 0011 AC4: settings and SQLite migration are validated and atomic, while
  keychain migration never writes secrets outside the secure credential layer.
- Fresh installations create canonical config, database, keychain, schema,
  metric, and browser-storage identifiers.
- Migration is idempotent, retains legacy sources, and has fake-store tests for
  settings, SQLite, credentials, and the native WebView profile copied before
  the first canonical window opens.
- GitHub issue #176 acceptance criteria pass.

## Dependencies

- `../../adr/0011-norn-naming-and-compatibility.md`
- `../../adr/0003-credentials-keychain.md`
- GitHub issue #176
