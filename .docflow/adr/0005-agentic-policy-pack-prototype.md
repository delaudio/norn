---
adr: 0005
title: Publish an agentic code review policy pack prototype
status: Implemented
date: 2026-07-10
owner: default-agent
supersedes:
superseded-by:
depends-on: [0001]
tags: [policy, examples, monetization]
---

# ADR 0005 - Publish an agentic code review policy pack prototype

## Context

Lachesi can load local policy packs from repository configuration. The product
strategy calls out policy packs as the near-term wedge for proving local-first
review governance and future commercial packaging.

The repository needs a concrete pack that exercises the public pack contract
without exposing private customer context or pretending there is already a
hosted registry.

## Capability statement

Lachesi will include a public `agentic-code` policy pack prototype under
`examples/policy-packs/`. The pack demonstrates the local manifest format,
agentic-code review rules, named profiles, analyzer defaults, and example
review output while staying installable from a local path.

## User stories / scenarios

- As a maintainer, I can dogfood a realistic policy pack against Lachesi and
  other repositories.
- As a contributor, I can inspect a working pack format without relying on
  private policy content.
- As a future commercial pack author, I can adapt the public prototype into a
  private pack without changing the loader contract.

## Acceptance criteria

1. A local `agentic-code` pack manifest exists under `examples/policy-packs/`.
2. The pack contains 15-25 agentic review rules across prompt, path, and AST
   oriented policy declarations.
3. The pack exposes `agentic-fast`, `agentic-balanced`, and `agentic-strict`
   profiles.
4. Example repository config and example markdown/JSON outputs are included.
5. Documentation explains how to adapt the pack safely without committing
   private customer context or credentials.
6. A test loads the checked-in pack through the existing local pack loader.

## Out of scope

- A hosted policy registry.
- Private or paid policy distribution.
- A complete static-analysis runtime for every rule.

## Open questions

- None.

## References

- ../../docs/strategy/open-core-boundary.md
- ../../docs/specs/0003-repository-config.md
- ../../docs/specs/0004-policy-engine.md
- https://github.com/delaudio/norn/issues/43

## Revision History

| Date | Revision | Author | Change |
|------|----------|--------|--------|
| 2026-07-10 | r1 | default-agent | Accepted the public agentic policy pack prototype. |
| 2026-10-04 | r2 | default-agent | Marked Implemented; the pack, profiles, examples, and loader coverage shipped. |

## Approvals

| Role | Name | Date | Signature |
|------|------|------|-----------|
| Maintainer | fdg | 2026-07-10 | approved in chat |
