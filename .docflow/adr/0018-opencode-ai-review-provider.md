---
adr: 0018
title: Run AI reviews through an OpenCode provider
status: Implemented
date: 2026-09-28
owner: default-agent
supersedes:
superseded-by:
depends-on: [0002, 0003, 0007]
tags: [ai, provider, opencode, deepseek, cli, review]
---

# ADR 0018 - Run AI reviews through an OpenCode provider

## Context

Norn executes AI reviews through local coding-agent CLIs: Claude Code
(`claude`) and Codex (`codex`). Each installed CLI owns its own authentication
and provider routing, while Norn owns payload assembly, isolation, bounded
execution, cancellation, structured output, and persistence.

Developers also run models that neither CLI routes to by default, most visibly
DeepSeek. OpenCode is a terminal coding agent with a headless `opencode run`
mode, an explicit `provider/model` selector, and its own credential store.
DeepSeek's current models surface there as `deepseek/deepseek-flash`
(DeepSeek-V4.1-Flash) and `deepseek/deepseek-v4-pro`. Adding a raw HTTP model
client instead would fork payload assembly, isolation, and credential handling
away from the established CLI contract and duplicate provider authentication
that OpenCode already solves.

OpenCode exposes the pieces Norn needs to stay consistent with the existing
providers: a non-interactive `run` subcommand that reads the prompt from
standard input, a `--model provider/model` flag, a machine-readable
`--format json` event stream, a session identifier for follow-up turns, and an
inline runtime configuration that can constrain tool permissions.

## Capability statement

Norn exposes OpenCode as a third AI review provider alongside Claude and Codex,
executing the same review payload through an installed `opencode run` under the
same read-only, bounded, cancellable, and auditable local review contract, with
DeepSeek models as the default catalog.

## User stories / scenarios

- As a developer who reviews with DeepSeek, I can select OpenCode as my AI
  review provider and choose `deepseek/deepseek-flash` without configuring a
  second integration.
- As a terminal user, I can switch the AI review provider and model from the
  TUI using the same settings surface as Claude and Codex.
- As a desktop user, I can pick OpenCode in Settings and run an inline review
  against a pull request or local target.
- As an automation author, I can run `norn review --ai-provider opencode
  --model deepseek/deepseek-v4-pro` from a script or agent.
- As a security reviewer, I can confirm that an OpenCode review cannot modify
  the repository and cannot read the filesystem in isolated diff-only mode.

## Acceptance criteria

1. `opencode` is accepted wherever `claude` and `codex` are: `norn review` and
   `norn setup` parse `--ai-provider opencode`, and the value round-trips
   through local settings as `aiProvider: "opencode"` with `opencodeModel` and
   `opencodeEffort`.
2. The desktop settings UI and the terminal UI expose OpenCode as an AI review
   provider, with a model field accepting `provider/model` that offers at least
   `deepseek/deepseek-flash` and `deepseek/deepseek-v4-pro`, plus a
   variant/effort control.
3. A review through the OpenCode provider executes an installed `opencode run`
   non-interactively, with the review payload written to private temporary
   storage and delivered on standard input, the selected model passed as
   `--model`, and the effort/variant passed as `--variant`.
4. OpenCode execution is read-only with respect to the reviewed repository. An
   inline runtime configuration defaults every tool to deny and only re-enables
   repository-reading tools for repository-backed reviews; mutation, shell,
   subagent, and network tools stay denied. Isolated diff-only execution runs
   in a private temporary working directory with every tool denied.
5. OpenCode execution is bounded by the existing AI provider timeout, is
   cancellable through the existing process-tree mechanism, and maps failures
   onto the existing sanitized provider error taxonomy without exposing
   provider stdout, stderr, credentials, or diff content.
6. The assistant response is reconstructed from the OpenCode JSON event stream,
   and the session identifier is captured in the run log. The event stream is
   parsed defensively: malformed or unrecognized events are ignored, an explicit
   error event fails the review even after partial text, and a run that produces
   no assistant text fails with the empty-response error.
7. Model and effort values are validated against a safe character set before
   invocation; an invalid value fails with the existing configuration error
   instead of being passed to the process.
8. Setup and readiness detect the `opencode` CLI, report its availability and
   version, and never read or expose provider credentials.
9. Existing Claude and Codex behavior, configuration, and outputs remain
   backward compatible; a settings file without `opencodeModel` or
   `opencodeEffort` deserializes to defaults.
10. Tests cover configuration round-trip, CLI parsing, model/effort
    validation, command construction, event-stream parsing, readiness probing,
    provider selection in both interactive surfaces, and mock IPC parity.

## Out of scope

- Adding an OpenCode provider to the shared or self-hosted review service.
- Managing DeepSeek or any provider API key inside Norn; OpenCode owns provider
  authentication and credential storage.
- A direct OpenAI/Anthropic-compatible HTTP model client for DeepSeek.
- Changing Claude or Codex execution semantics, prompts, or stored sessions.
- Distributing the managed Norn review skill to OpenCode agents.

## Open questions

- None.

## References

- [All Bitbucket HTTP lives in Rust](./0002-http-in-rust.md)
- [Credentials in the OS keychain, config in a settings file](./0003-credentials-keychain.md)
- [Run reviews through a headless local CLI](./0007-headless-review-cli.md)
- [Distribute managed agent review skills with installed Norn](./0015-managed-agent-review-skills.md)
- `../../src-tauri/src/services/review.rs`
- `../../.archgate/adrs/FE-001-expose-ai-review-as-explicit-user-invoked-actions.md`
- <https://opencode.ai/docs/cli>
- <https://opencode.ai/docs/permissions>

## Revision History

| Date | Revision | Author | Change |
|------|----------|--------|--------|
| 2026-09-28 | r1 | default-agent | Initial draft and acceptance of an OpenCode-backed AI review provider. |
| 2026-09-28 | r2 | default-agent | Strengthened isolation to default-deny all tools, reject invalid model/variant settings, and fail on stream error events after partial output following a pre-push review. |
| 2026-09-28 | r3 | default-agent | Marked implemented after the provider shipped in `v0.3.1` through PR #264. |

## Approvals

| Role | Name | Date | Signature |
|------|------|------|-----------|
| Maintainer | fdg | 2026-09-28 | approved implementation in chat |
