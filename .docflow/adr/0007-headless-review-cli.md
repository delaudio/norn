---
adr: 0007
title: Run reviews through a headless local CLI
status: Accepted
date: 2026-07-24
owner: default-agent
supersedes:
superseded-by:
depends-on: [0002, 0003, 0005, 0006]
tags: [cli, headless, review, automation, codex]
---

# ADR 0007 - Run reviews through a headless local CLI

## Context

Lachesi supports desktop and terminal interfaces, but automated coding agents,
local scripts, and CI jobs need a non-interactive review surface. Driving the
terminal UI is not a stable automation contract, and a Codex skill alone cannot
provide Lachesi review semantics without an executable interface.

The native layer already owns repository resolution, effective repository
configuration, local evidence analyzers, AI provider execution, structured
findings, and review persistence. Headless execution must reuse those semantics
without initializing a Tauri runtime, publishing provider comments, or granting
the reviewer write access to the repository.

Agent-triggered review also creates a recursion risk: when Lachesi invokes
`codex exec`, repository instructions or lifecycle hooks could try to invoke
Lachesi again. Headless review therefore needs an explicit child-process marker
that integrations can use to skip nested review enforcement.

## Capability statement

Lachesi will expose a non-interactive `lachesi review` CLI that reviews local
working-tree, branch, or provider pull-request changes through shared native
review services, emits stable human- and machine-readable results, and can be
orchestrated safely by coding agents and CI.

## User stories / scenarios

- As a developer using Codex, I can run Lachesi after a task and receive
  structured findings for the changes Codex just made.
- As a CI maintainer, I can fail a job only when findings meet an explicit
  severity threshold.
- As a reviewer, I can inspect Markdown output locally without launching the
  desktop app or terminal UI.
- As an automation author, I can enforce a final review without recursively
  launching nested Lachesi reviews.

## Acceptance criteria

1. `lachesi review` runs without initializing the Tauri application runtime and
   supports Markdown and JSON output. It uses ephemeral writable storage by
   default so sandboxed agents do not need access to the desktop database;
   explicit `LACHESI_DATA_DIR` configuration remains respected.
2. Local working-tree review includes staged changes, unstaged changes, and
   untracked text files without modifying the repository.
3. Local branch review compares the current branch with an explicit or resolved
   base reference, and pull-request review can use the configured GitHub or
   Bitbucket provider diff.
4. Headless review loads the effective repository prompt, named profile, and
   policy packs. It skips local evidence analyzers by default because post-task
   automation has already run the repository gate; `--run-analyzers` opts into
   the shared native analyzer pipeline for standalone use.
5. The AI reviewer runs read-only, and a `LACHESI_REVIEW_CHILD` environment
   marker prevents skills or hooks from recursively enforcing another review.
6. JSON output uses the existing structured review finding semantics and never
   includes credentials or other secrets.
7. Exit codes distinguish successful review, threshold findings, invalid
   configuration, unresolved repository/target, required analyzer failure, AI
   provider failure, internal failure, and cancellation.
8. A versioned, installable Codex skill documents the review, remediation,
   rerun, publication, and recursion boundaries.

## Out of scope

- Automatically publishing review comments to GitHub or Bitbucket.
- Automatically committing, pushing, or fixing findings inside the headless
  review process.
- Interactive chat threads or terminal rendering in CLI mode.
- Requiring a provider pull request for local working-tree review.
- Treating implicit skill activation as deterministic enforcement.

## Open questions

- None.

## References

- ../../docs/specs/0006-cli-headless-review.md
- ./0002-http-in-rust.md
- ./0003-credentials-keychain.md
- ./0005-agentic-policy-pack-prototype.md
- ./0006-terminal-ui.md
- ../../.archgate/adrs/ARCH-001-tauri-react-rust-bitbucket-boundary.md
- ../../.archgate/adrs/ARCH-004-keep-tauri-commands-thin-and-delegate-to-native-service-modules.md
- https://learn.chatgpt.com/docs/build-skills
- https://learn.chatgpt.com/docs/hooks

## Revision History

| Date | Revision | Author | Change |
|------|----------|--------|--------|
| 2026-07-24 | r1 | default-agent | Accepted headless review execution and agent orchestration. |
| 2026-07-24 | r2 | default-agent | Made analyzers opt-in for post-task headless review. |
| 2026-07-24 | r3 | default-agent | Made headless storage ephemeral and agent execution one-shot. |

## Approvals

| Role | Name | Date | Signature |
|------|------|------|-----------|
| Maintainer | fdg | 2026-07-24 | approved in chat |
| Maintainer | fdg | 2026-07-24 | approved analyzer opt-in revision in chat |
| Maintainer | fdg | 2026-07-24 | approved one-shot review refinement in chat |
