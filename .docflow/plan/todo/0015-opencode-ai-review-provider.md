# OpenCode AI Review Provider

## Owning ADRs

- `../../adr/0018-opencode-ai-review-provider.md`

## Scope

Add an OpenCode-backed AI review provider alongside Claude and Codex, executed
through an installed `opencode run` CLI. Wire the provider through the shared
native review service, the headless `norn review` CLI, the desktop settings
surface, and the terminal UI provider selector. Expose DeepSeek models
(`deepseek/deepseek-flash`, `deepseek/deepseek-v4-pro`) as the default catalog
while keeping the model selector open to any `provider/model` OpenCode accepts.

Keep OpenCode authentication, credential storage, and model routing inside
OpenCode. Norn delivers the review payload on standard input, constrains tool
permissions through an inline runtime configuration, parses the JSON event
stream, bounds execution with the existing provider timeout, and reuses the
existing cancellation, sanitized error, and inline-log contracts.

Out of scope: a direct DeepSeek HTTP client, shared-service provider support,
OpenCode-side Norn skill distribution, and any change to Claude or Codex
execution.

## Exit Criteria

- ADR 0018 AC1: `opencode` is accepted by `norn review`, `norn setup`, and the
  config surface, and round-trips through local settings.
- ADR 0018 AC2: desktop settings and the TUI expose OpenCode with model and
  effort/variant selection.
- ADR 0018 AC3-6: OpenCode execution delivers the payload on stdin, is
  read-only, bounded, cancellable, and reconstructs the response from the JSON
  event stream.
- ADR 0018 AC7-8: model/effort validation and setup/readiness detection are
  covered by tests and expose no credentials.
- ADR 0018 AC9-10: Claude and Codex stay backward compatible and the automated
  suite covers configuration, CLI parsing, command construction, parsing,
  readiness, both interactive surfaces, and mock IPC parity.
- `pnpm run typecheck`, `pnpm run test`, and `archgate check` pass.

## Dependencies

- `../../adr/0007-headless-review-cli.md`
- `../../adr/0018-opencode-ai-review-provider.md`
