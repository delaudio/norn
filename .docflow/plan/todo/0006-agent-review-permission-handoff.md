# Agent Review Permission Handoff

## Owning ADRs

- `../../adr/0007-headless-review-cli.md`
- `../../adr/0015-managed-agent-review-skill-distribution.md`

## Scope

Make agent-triggered headless review predictable across Codex and Claude Code.
Require explicit local authorization before a headless review sends the selected
diff and review instructions to the configured AI provider, teach both managed
skills to request host execution permission before launching Norn, detect known
restricted host execution before provider invocation, and bound provider CLI
waits with stable machine-readable failures.

Out of scope: granting host permissions on the user's behalf, editing Codex or
Claude Code permission files, changing desktop or TUI review consent, exposing
provider stderr, or broadening the diff selected by `norn review`.

## Exit Criteria

- Headless review accepts one-run diff-sharing authorization and an explicit
  locally persisted setup choice, while sending no diff when neither exists.
- The managed skill gives Codex and Claude Code agent-specific instructions to
  request the narrow host permission before running the same review command.
- A known restricted Codex host fails before provider invocation with a stable
  `review.sandboxRestricted` machine code and actionable public guidance.
- Provider waits are bounded for both Codex and Claude and return a sanitized
  `review.providerTimeout` failure rather than hanging indefinitely.
- Tests cover parsing, persistence shape, consent enforcement, sandbox
  classification, timeout classification, and both installed skill targets.
- Public documentation explains the disclosure and permission boundaries in
  English without exposing private paths or internal architecture records.
- The repository verification gate passes: `pnpm run typecheck`, `pnpm run
  test`, and `archgate check`.

## Dependencies

- `../../adr/0007-headless-review-cli.md`
- `../../adr/0015-managed-agent-review-skill-distribution.md`
