# Secure Terminal Credential Onboarding

## Owning ADRs

- `../../adr/0012-norn-onboarding-contract.md`

## Scope

Implement GitHub issue #216 by adding a terminal-first credential lifecycle for
GitHub and Bitbucket that reuses the existing Rust credential module. Provide
sanitized status, secure interactive or standard-input credential entry,
validation, replacement, and removal without requiring the desktop app. Wire
the flow into `norn setup` guidance and `norn doctor` readiness while preserving
environment-variable and terminal-config references.

Credential mutation must store secrets only through the OS keychain. Command
arguments, output, logs, settings files, repository files, crash diagnostics,
and fixtures must never contain raw token values.

Out of scope: OAuth browser flows, third-party secret managers, remote credential
synchronization, desktop settings redesign, and agent-skill installation.

## Exit Criteria

- ADR 0012 AC1: machine credential setup is explicit and has independent,
  deterministic success and failure signals.
- ADR 0012 AC2-3: status and mutation paths provide safe preview or confirmation
  semantics, with non-interactive behavior restricted to documented inputs.
- ADR 0012 AC5: human and JSON output, errors, logs, and tests never expose raw
  credentials or private machine paths.
- ADR 0012 AC6: `norn auth status` or the selected equivalent integrates with
  machine readiness and reports pass/warn/fail state without mutation.
- ADR 0003 AC1-2: secrets remain in the OS keychain and non-secret settings
  remain in the OS config directory.
- ADR 0003 AC3-4: keychain and environment/config-reference precedence remains
  compatible, and environment credentials are never silently persisted.
- GitHub and Bitbucket credentials can be configured, validated, replaced, and
  removed from a clean Homebrew command installation.
- Unit and integration tests cover masked input, standard-input handling,
  replacement, removal, missing credentials, invalid credentials, keychain
  failures, environment fallbacks, and redaction in human and JSON output.
- The repository verification gate passes: `pnpm run typecheck`, `pnpm run
  test`, and `archgate check`.

## Dependencies

- `../../adr/0012-norn-onboarding-contract.md`
- `../../adr/0003-credentials-keychain.md`
- `../../adr/0013-norn-readiness-probe.md`
- GitHub issue #216

Shipped at HEAD `9179adc05831efe4ddc1924551a9e0fa4a7d88c6` through
https://github.com/delaudio/norn/pull/220.
