# Spec 0012 - v0.1 Norn onboarding readiness probe

- Status: Draft
- Date: 2026-08-05
- GitHub issue: #187

## Context

`norn doctor` provides a deterministic, read-only preflight report consumed by
CLI workflows, TUI entry points, and automation. It must provide enough structure
to make onboarding idempotent without implicit side effects.

## Command surface

### `norn doctor`

- `--repo-path <path>`: repository for readiness inspection; defaults to `.`.
- `--format json|human|text`: JSON for machines, human for manual inspection.
- `--machine-only`: skip repository checks and inspect only machine prerequisites.
- `--json`: shorthand for `--format json`.

## Output contract

Top-level JSON must include:

- `schemaVersion` (`norn.readiness.v1`)
- `status` (`ok` | `warn` | `fail`)
- `timestamp`
- `machine` block
- `repository` block
- `issues` array sorted by severity (`error`, `warning`, `info`)

Machine checks should include:

- OS config directory exists and writable
- default data directory path
- presence of optional migration alias compatibility
- availability/version for `norn`-adjacent CLIs (`claude`, `codex`) when configured
- provider credential availability by provider (`github`, `bitbucket`, `jira`, `notion`)

Repository checks should include:

- git root resolution from current directory
- git remote parse and supported host supportability
- branch and HEAD readability
- clean/dirty working tree flags, untracked path detection where lightweight
- presence and precedence of `.norn.yaml`, `.norn.local.yaml`, and legacy aliases
- loaded profile and warning list from existing `repo_config` loaders
- analyzer detection (names + command resolution, no execution)

## Failure model

- Missing critical repository prerequisites emit `error` and set `status=fail`.
- Optional missing items emit `warning` and keep `status=warn` unless critical path
  is requested.
- Unknown provider or malformed remotes emit `error` with explicit remediation text.

## Determinism

- Probe output is deterministic for the same machine state and repository tree.
- Re-run in unchanged state should not produce divergent issue ordering.

## Non-goals

- Running analyzers or review.
- Writing/repairing `.norn.*` files.
- Resolving or provisioning missing credentials.
