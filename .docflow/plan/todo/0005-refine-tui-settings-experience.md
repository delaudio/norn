# Refine the TUI Settings Experience

## Owning ADRs

- `../../adr/0006-terminal-ui.md`
- `../../adr/0003-credentials-keychain.md`
- `../../adr/0012-norn-onboarding-contract.md`

## Scope

Refine the shipped Terminal UI settings experience so it reads as part of the
review workspace and makes provider credential onboarding explicit. Present a
bounded settings panel with the same visual hierarchy, borders, colors, and
footer language as the main workspace. Group AI review settings, provider
credentials, and CLI readiness into distinct sections.

Replace the generic credential action with provider-aware Configure, Replace,
and Remove actions. Give GitHub token entry and the Bitbucket username/token
sequence visible labels, contextual guidance, masked secret input, and
predictable back/cancel behavior.

Out of scope: OAuth, a new credential store, desktop settings redesign, and a
general redesign of the review workspace.

## Exit Criteria

- ADR 0006 AC6: focused terminal-backend tests cover the normal and narrow
  settings layouts, section hierarchy, contextual help, focus, edit, save, and
  cancel behavior.
- ADR 0006 AC2 and ADR 0003 AC1-4: credential actions continue to use the
  shared native credential layer and never render raw tokens. When the active
  source is the environment, Remove refuses the action and Configure starts a
  fresh keychain flow without copying the environment value.
- ADR 0012 AC1: GitHub and Bitbucket configuration are explicit terminal
  onboarding flows with provider-specific action labels and input prompts.
- ADR 0012 AC5-6: settings expose sanitized credential source/readiness state
  without secrets or private machine paths.
- Escape from the Bitbucket token step returns to the username step while
  retaining the entered username. Escape from any other editor closes only that
  editor; pending AI provider, model, and effort values remain unchanged.
- The repository verification gate passes: `pnpm run typecheck`, `pnpm run
  test`, and `archgate check`.
- The command-distribution Rust lane and lint pass: `cargo test --locked
  --manifest-path src-tauri/Cargo.toml --all-targets --no-default-features
  --features custom-protocol` and the matching `cargo clippy` invocation with
  warnings denied.

## Dependencies

- `../done/2026-09-01-complete-tui-settings.md`
- `../done/2026-09-01-secure-terminal-credential-onboarding.md`
- `../../adr/0006-terminal-ui.md`
- `../../adr/0003-credentials-keychain.md`
- `../../adr/0012-norn-onboarding-contract.md`
