# Codex Sandbox Permission Diagnostics

## Owning ADRs

- `../../adr/0007-headless-review-cli.md`

## Scope

Implement GitHub issue #198 by classifying provider failures caused by filesystem
or sandbox permission denial during Codex app-server initialization. Return a
stable actionable public error while continuing to exclude raw provider output
from JSON and Markdown results.

Out of scope: changing Codex invocation flags, relaxing host sandbox
permissions, or exposing provider stderr.

## Exit Criteria

- Permission-denied and app-server initialization failures map to an actionable
  sanitized public message.
- Raw provider stdout and stderr remain absent from public output.
- Existing startup, empty-response, and invalid-output classifications remain
  unchanged.
- Focused unit tests cover the classification and redaction boundary.
- The repository verification gate passes: `pnpm run typecheck`, `pnpm run
  test`, and `archgate check`.

## Dependencies

- `../../adr/0007-headless-review-cli.md`
- GitHub issue #198
