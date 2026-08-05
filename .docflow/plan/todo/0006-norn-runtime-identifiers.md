# Norn Runtime Identifiers

## Owning ADRs

- `../../adr/0011-norn-naming-and-compatibility.md`

## Scope

Implement GitHub issue #174 by making Norn the canonical package, binary,
Tauri bundle, menu, CLI, and terminal UI identity. The change adds explicit
deprecated legacy binary aliases and preserves Makefile/justfile recipe parity.

## Exit Criteria

- ADR 0011 AC1: fresh builds expose Norn as the product identity.
- ADR 0011 AC5: `lachesi` and `lac` aliases emit deprecation messages.
- GitHub issue #174 acceptance criteria pass for package, Tauri, CLI, TUI,
  task-runner, Rust, and TypeScript surfaces.

## Dependencies

- `../../adr/0011-norn-naming-and-compatibility.md`
- GitHub issue #174
