---
adr: 0013
title: Read-only readiness probing for machine and repository onboarding
status: Accepted
date: 2026-08-05
owner: codex
supersedes: []
superseded-by:
depends-on: [0012, 0003, 0011]
tags: [norn, onboarding, readiness, cli, cli-headless]
---

# ADR 0013 - Read-only readiness probing for machine and repository onboarding

## Context

Norn currently has first-run command surfaces (`config validate` and `config
migrate`) but no single, stable read-only readiness snapshot. Consumers still need to
discover whether machine context, repository structure, credentials, analyzers, and local
configuration are sufficient before review or before onboarding mutates anything.

This gap causes inconsistent behavior across CLI, TUI, and desktop startup paths.

## Capability statement

Norn will expose a read-only, deterministic onboarding readiness probe (`norn doctor`)
that returns machine state and repository state without performing mutations.

## User stories / scenarios

- As a first-time user, I need one command that tells me what is missing before review
  starts.
- As an operator, I need stable machine and repository readiness status for
  automation and onboarding orchestration.
- As a shell script, I need machine-readable output to gate CI or local preflight.
- As an automation, I need a consistent read-only API for readiness across CLI,
  TUI, and desktop.

## Acceptance criteria

1. The readiness probe performs no file writes, credential writes, or network writes.
2. The probe returns explicit machine-state and repository-state sections with
   deterministic fields and stable names.
3. Missing provider credentials are reported as capabilities/state, not hard failures,
   while required state for current command path is clearly marked.
4. `.norn.yaml`/`.norn.local.yaml` and legacy configuration precedence is
   evaluated and reported without creating or migrating files.
5. Git remote/provider/branch/dirt-state are inspected without changing index,
   working tree, or staging area.
6. AI tool availability (`codex`, `claude`) is checked through command discovery
   and version query where available, without executing model calls.
7. JSON output is supported and stable for machine consumption, including non-empty
   `status` and `issues` arrays.

## Out of scope

- Remediating failures; fixing repositories; writing config.
- Any mutation of credentials, repository files, or provider state.
- Installing missing external CLIs.

## Open questions

- None.

## References

- `../../docs/specs/0012-norn-readiness-probe.md`
- `../../docs/specs/0011-norn-onboarding-contract.md`
- `../../docs/specs/0003-repository-config.md`
- `../../docs/specs/0006-cli-headless-review.md`
- `../adr/0011-norn-naming-and-compatibility.md`

## Revision History

| Date | Revision | Author | Change |
|------|----------|--------|--------|
| 2026-08-05 | r1 | codex | Accepted readiness probe contract for issue #187. |

## Approvals

| Role | Name | Date | Signature |
|------|------|------|-----------|
