# Agentic Code Review Rules

This is a public prototype policy pack for reviewing code produced or heavily
modified by AI coding agents. It demonstrates Norn's local policy pack
format without depending on a hosted registry or private policy content.

## Install Locally

Reference the pack from a repository-owned `.norn.yaml`:

```yaml
version: 0.1
review:
  profile: agentic-balanced
policy:
  packs:
    - ./examples/policy-packs/agentic-code
```

When the pack lives outside the reviewed repository, use an absolute path or a
checked-in relative path that your team controls.

## Profiles

- `agentic-fast` focuses on high-confidence blockers and keeps analyzers
  optional.
- `agentic-balanced` requires typecheck evidence and keeps tests/lint optional.
- `agentic-strict` requires typecheck, tests, and lint and lowers the finding
  threshold.

## Adapting The Pack

Start by copying `pack.yaml` into a private repository or an internal policy
bundle. Keep the rule ids stable once reviewers depend on them. Add rules only
when they describe recurring review failures, not one-off taste preferences.

Keep public and private material separate:

- Do not commit customer names, private repository paths, internal incident
  details, credentials, or model transcripts.
- Use generic examples in public packs and sanitized examples in private packs.
- Keep analyzer commands deterministic local checks such as typecheck, lint,
  tests, or scanners already installed by the repository.
- Prefer narrow path rules over broad prose when only one subsystem is affected.

The pack is intentionally prompt- and declaration-oriented. Unsupported AST
rules should warn rather than claim complete static-analysis coverage.

## Files

- `pack.yaml` - loadable policy pack manifest.
- `examples/norn.yaml` - repository config that enables this pack.
- `examples/review-output.md` - sample human-readable output.
- `examples/review-output.json` - sample structured output.
