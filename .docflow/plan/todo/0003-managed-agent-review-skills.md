# Managed Agent Review Skills

## Owning ADRs

- `../../adr/0015-managed-agent-review-skill-distribution.md`

## Scope

Implement GitHub issue #217 by packaging the complete `norn-review` skill with
the command release and adding an explicit Norn lifecycle for Codex, Claude
Code, or both. Install versioned skill content from stable package data into the
standard personal skill directories without requiring a source checkout.

The lifecycle must provide inspectable status, idempotent installation and
upgrade, conflict-safe explicit replacement, and ownership-aware removal.
Release and Homebrew validation must bind the packaged skill content to the
candidate version and prove both agent layouts before advancing the public tap.

Out of scope: agents other than Codex and Claude Code, implicit activation as
deterministic enforcement, credential management, and package managers other
than Homebrew.

## Exit Criteria

- ADR 0015 AC1-2: every command release and Homebrew installation contains the
  complete versioned skill in a stable package data location.
- ADR 0015 AC3-5: one documented command surface installs for Codex, Claude
  Code, or both, preserves explicit invocation and automatic selection metadata,
  and resolves the active released `norn` executable.
- ADR 0015 AC6-8: install and upgrade are atomic and idempotent, unmanaged
  conflicts require explicit authorization, status is sanitized, and uninstall
  removes only Norn-managed content.
- ADR 0015 AC9: tests cover single-agent, combined, repeated, conflicting,
  forced replacement, upgrade, failed replacement, and uninstall flows.
- ADR 0015 AC10: Homebrew lifecycle checks verify the candidate skill version
  and both agent layouts before public tap publication.
- Public documentation describes install, status, upgrade, conflict, and
  uninstall behavior for both Codex and Claude Code without requiring a source
  checkout.
- The repository verification gate passes: `pnpm run typecheck`, `pnpm run
  test`, and `archgate check`.

## Dependencies

- `../../adr/0015-managed-agent-review-skill-distribution.md`
- `../../adr/0007-headless-review-cli.md`
- `../../adr/0014-reproducible-release-and-upgrade-channels.md`
- GitHub issue #217
