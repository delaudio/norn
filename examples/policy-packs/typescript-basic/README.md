# TypeScript Review Basics

A small public example pack for reviewing TypeScript changes. It demonstrates
prompt rules, a path rule, an AST-oriented rule declaration, and analyzer
defaults in one loadable manifest.

## Install Locally

```yaml
# .norn.yaml
version: 0.1

policy:
  packs:
    - ./examples/policy-packs/typescript-basic
```

Use an absolute path or a checked-in relative path your team controls when the
pack lives outside the reviewed repository.

## What It Covers

- `ts.no-explicit-any`, `ts.prefer-unknown-over-any`: type widening.
- `ts.no-non-null-assertion`: unsound non-null assertions.
- `ts.surface-caught-errors`, `ts.no-empty-catch`: swallowed failures.
- `ts.public-api-docs`: changes to a public entry point.

Analyzer defaults enable `typecheck` and mark `tests` and `lint` optional.

## Adapting It

Copy `pack.yaml` into a private repository or an internal bundle and keep the
rule ids stable once reviewers depend on them. Add rules only for recurring
review failures. Keep customer names, private paths, credentials, and model
transcripts out of committed rules.

## Files

- `pack.yaml` - loadable policy pack manifest.
