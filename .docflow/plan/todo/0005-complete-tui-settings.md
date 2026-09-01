# Complete and Discoverable TUI Settings

## Owning ADRs

- `../../adr/0012-norn-onboarding-contract.md`

## Scope

Implement GitHub issue #218 by making TUI settings visible from the main
interface and completing the terminal settings experience. Add a discoverable
`s settings` action, responsive help behavior, provider-aware AI configuration
that preserves custom model and effort values, and sanitized readiness for
GitHub, Bitbucket, Codex, and Claude.

Credential actions must reuse the secure terminal lifecycle from plan item
0002 and the existing OS-keychain boundary. Non-secret values continue to use
the shared Norn settings file so TUI, headless CLI, and desktop behavior remain
compatible.

Out of scope: a new credential store, rendering raw secret values, redesigning
the desktop settings interface, and unrelated TUI review features.

## Exit Criteria

- ADR 0012 AC1: terminal settings and credential onboarding are explicit,
  discoverable machine-setup surfaces independent from repository init.
- ADR 0012 AC5-6: settings and readiness output exclude secrets and private
  paths while exposing actionable provider state.
- ADR 0006 AC2: TUI settings reuse native configuration, credential, and
  provider services rather than reimplementing them.
- ADR 0006 AC6: focused terminal-backend tests cover the settings footer,
  responsive help, navigation, save, cancel, errors, and credential status.
- The main interface visibly advertises settings at the minimum supported
  terminal size without silently hiding the entry point.
- AI provider, model, and effort settings preserve valid custom values and are
  reflected immediately in TUI review runs.
- GitHub and Bitbucket status and actions reuse plan item 0002 without exposing
  raw tokens or persisting them in non-secret settings.
- Public TUI documentation and screenshots show the settings entry point and
  terminal-only onboarding path.
- The repository verification gate passes: `pnpm run typecheck`, `pnpm run
  test`, and `archgate check`.

## Dependencies

- `./0002-secure-terminal-credential-onboarding.md`
- `../../adr/0012-norn-onboarding-contract.md`
- `../../adr/0003-credentials-keychain.md`
- `../../adr/0006-terminal-ui.md`
- GitHub issue #218
