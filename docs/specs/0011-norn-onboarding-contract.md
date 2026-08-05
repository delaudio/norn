# Spec 0011 - v0.1 Norn onboarding contract

- Status: Draft
- Date: 2026-08-05
- GitHub issue: #186

## Context

Norn has already renamed identity and repository/config namespaces, and now needs a
consistent first-run contract before first users are expected to run local reviews.
Current behavior is partially split across CLI, settings, and desktop surfaces.

Onboarding must be safe, deterministic, and reversible so users can retry after
missing credentials, incomplete repo state, or declined repo edits.

## Goals

- Define clear machine-owned setup and repository-owned initialization as separate
  steps.
- Provide deterministic proposals for repository onboarding without mutating files.
- Add non-interactive/JSON/dry-run support paths for CLI tooling.
- Keep secrets out of generated files, logs, and preview output.
- Preserve existing repository files unless an explicit approval flow is passed.

## Non-goals

- Changing analyzer command semantics at execution time.
- Enforcing a single UI experience across all surfaces before the API contract is
  stable.
- Full Homebrew bootstrap and packaging (handled in later issues).

## Scope and surfaces

### Machine setup command (`norn setup`)

- Detect available review providers (`claude`, `codex`) and validate CLI presence.
- Probe available provider credential sources without copying tokens.
- Resolve GitHub/Bitbucket account context and map provider credentials to
  reusable application config or explicit status diagnostics.
- Store only non-secret preferences under machine-owned app config.

### Repository initialization command (`norn init`)

- Produce repository-specific initialization proposals from local evidence.
- Support quick defaults and guided flow with an explicit preview/approval step.
- Support `--dry-run` and JSON output for automated consumption.
- Restrict `--yes` to the documented quick path.

### Readiness command (`norn doctor`)

- Report machine state, repository config state, policy source, analyzer
  availability, and review readiness.
- Be read-only and non-mutating by default.
- Provide machine-parseable JSON state with stable fields.

## Behavioral rules

### Secrets and privacy

- Never write explicit provider tokens into committed repository files.
- Never emit raw secret values in command output.
- Never include private local paths unless normalized for diagnostics.

### Determinism and idempotence

- Re-running quick/guided onboarding with unchanged inputs should produce a stable
  proposal.
- Re-running on an unchanged repository should avoid writing files.
- Merge and repair operations must report conflicts explicitly.

### Failure handling

- Missing credentials and partially valid config must fail clearly with actionable
  remediation steps.
- Cancelled or invalid writes must leave previous config byte-for-byte intact.
- Read-only states remain usable for local-only workflows where possible.
