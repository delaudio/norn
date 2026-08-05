# Norn Naming and Compatibility Contract

## Owning ADRs

- `../../adr/0011-norn-naming-and-compatibility.md`

## Scope

Implement GitHub issue #173 by establishing the accepted Norn naming and
compatibility contract before any runtime, configuration, data, or delivery
rename. The work records canonical identifiers, compatibility precedence,
deprecation timing, and the public repository target while keeping GitHub
organization renaming out of scope.

## Exit Criteria

- ADR 0011 AC1: fresh installs have one canonical Norn command identity.
- ADR 0011 AC2: the migration matrix covers current external and persisted
  identifiers.
- ADR 0011 AC3: old/new configuration and environment precedence is defined.
- ADR 0011 AC4: local data and credential migration safety is explicit.
- ADR 0011 AC5: aliases have a measurable removal condition.
- GitHub issue #173 is closed after this accepted contract merges to `main`.

## Dependencies

- `../../adr/0003-credentials-keychain.md`
- `../../adr/0006-terminal-ui.md`
- `../../adr/0007-headless-review-cli.md`
- GitHub issue #173

---

Shipped as commit `79a90ece76be7b778383ea8c8fa88108ba66f51f`
through [PR #195](https://github.com/lachesi-hq/lachesi/pull/195).
