---
adr: 0012
title: Define the Norn first-run onboarding contract
status: Accepted
date: 2026-08-05
owner: codex
supersedes: []
superseded-by:
depends-on: [0011, 0003]
tags: [norn, onboarding, config, cli, cli-headless]
---

# ADR 0012 - Define the Norn first-run onboarding contract

## Context

Norn now has canonical identity and repository/runtime migration support, but
first-run behavior is still partially implicit. Users can reach states with missing
machine credentials, absent repository policy, legacy-only files, or analyzer
commands that are unreviewed on first launch. This breaks discoverability and can
lead to inconsistent local behavior across CLI, TUI, and desktop entry points.

## Capability statement

Norn will provide a bounded onboarding contract that separates machine setup from
repository initialization, supports deterministic quick and guided flows with dry-run
and non-interactive modes, and never writes secrets during proposal generation.

## User stories / scenarios

- As a new developer, I want an onboarding command that sets up machine-ready
  provider sources and local config quickly, so I can run local review safely
  from day one.
- As an existing user, I want repository initialization to produce deterministic
  proposals and a preview step, so I can trust changes before they are written.
- As a script runner, I want JSON output for non-interactive onboarding checks, so
  I can integrate Norn startup into automation.
- As an operator, I want a readiness check that is read-only and machine-friendly,
  so CI and scripts can verify readiness before running review.

## Acceptance criteria

1. Machine setup and repository initialization are separate, explicit steps with
   independent success/failure signals.
2. A proposal or mutation path supports `--dry-run` and emits concrete,
   actionable diffs without writing files.
3. A non-interactive `--yes` mode is restricted to documented quick defaults.
4. Legacy namespace artifacts remain readable during the compatibility window but do
   not silently become authoritative for onboarding decisions.
5. Secrets and personal paths are excluded from committed repository files and from
   all generated console output by default.
6. Onboarding readiness is exposed as a read-only human/JSON output surface with
   pass/warn/fail/unsupported states.

## Out of scope

- Advanced policy-pack recommendation services.
- Remote policy synchronization and credential migration from third parties.
- Full Homebrew, signing, and release plumbing (handled in later issues).

## Open questions

- None.

## References

- `../../docs/specs/0011-norn-onboarding-contract.md`
- `../../docs/specs/0003-repository-config.md`
- `../../docs/specs/0006-cli-headless-review.md`
- `../adr/0011-norn-naming-and-compatibility.md`
- https://github.com/lachesi-hq/lachesi/issues/186

## Revision History

| Date | Revision | Author | Change |
|------|----------|--------|--------|
| 2026-08-05 | r1 | codex | Initial accepted decision for onboarding contract boundaries. |

## Approvals

| Role | Name | Date | Signature |
|------|------|------|-----------|
