---
adr: 0015
title: Distribute managed agent review skills with installed Norn
status: Implemented
date: 2026-09-01
owner: codex
supersedes:
superseded-by:
depends-on: [0007, 0014]
tags: [distribution, skills, codex, claude, cli, homebrew]
---

# ADR 0015 - Distribute managed agent review skills with installed Norn

## Context

Norn ships a shared `norn-review` skill that lets coding agents run the
headless review CLI after implementation and before a push. The Homebrew
formula currently installs only the `norn` and `norn-tui` executables, while
the skill remains available only from a source checkout through manually
managed links. A released Norn installation therefore cannot provide the
agent-review workflow without an unrelated clone and a second, unversioned
installation procedure.

Codex and Claude Code use compatible skill content but distinct personal skill
directories and explicit invocation syntax. Distribution must support both
agents, preserve unmanaged user files, follow Norn upgrades, and avoid links
to source or versioned package paths that may disappear.

## Capability statement

Norn will distribute versioned agent-review skill assets with its supported
command release and provide an explicit, idempotent lifecycle that installs,
inspects, upgrades, and removes Norn-managed skills for Codex, Claude Code, or
both without requiring a source checkout.

## User stories / scenarios

- As a Homebrew user, I can install the Norn review skill for my coding agent
  without cloning the Norn repository.
- As a developer using both Codex and Claude Code, I can manage the shared
  review skill for both agents through one Norn command surface.
- As a user with an existing custom skill, I can inspect a conflict and keep my
  unmanaged files unless I explicitly authorize replacement.
- As a maintainer, I can prove that packaged skill content matches the released
  Norn version and survives install, upgrade, and uninstall lifecycle checks.

## Acceptance criteria

1. Every supported command release contains the complete, versioned
   `norn-review` skill content required by Codex and Claude Code.
2. The Homebrew formula installs the skill content into a stable package data
   location without requiring Node.js, Rust, or a source checkout.
3. A documented Norn command installs the managed skill for Codex, Claude Code,
   or both into their standard personal skill directories.
4. Codex and Claude Code can explicitly invoke the installed skill using the
   documented syntax for each supported agent version.
5. Installed skill instructions resolve the released `norn` executable from
   the active command path when no local-source executable is available.
6. Repeated installation and managed upgrades are idempotent and use atomic
   replacement, leaving the prior working skill intact on failure.
7. An unmanaged destination is never overwritten or removed without explicit
   user authorization.
8. Status output identifies managed targets and versions without exposing
   private paths in machine-readable or release output, and uninstall removes
   only Norn-managed content.
9. Automated tests cover Codex-only, Claude-only, combined, repeated,
   conflicting, upgrade, failed replacement, and uninstall flows.
10. Homebrew lifecycle validation proves that installed skill assets match the
   candidate release before the public tap advances.
11. The shared skill gives Codex and Claude Code explicit host-specific
    permission instructions, treats diff-sharing authorization separately from
    host sandbox approval, and never grants or broadens either permission on
    the user's behalf.

## Out of scope

- Installing skills for agents other than Codex and Claude Code.
- Treating implicit skill activation as deterministic review enforcement.
- Silently replacing or deleting unmanaged personal skills.
- Managing provider credentials through the skill lifecycle.
- Adding package managers other than Homebrew in this delivery slice.

## Open questions

- None.

## References

- ./0007-headless-review-cli.md
- ./0014-reproducible-release-and-upgrade-channels.md
- ../../integrations/agent-skills/norn-review/SKILL.md
- ../../packaging/homebrew/norn.rb.template
- https://github.com/delaudio/norn/issues/217

## Revision History

| Date | Revision | Author | Change |
|------|----------|--------|--------|
| 2026-09-01 | r1 | codex | Initial draft. |
| 2026-09-01 | r2 | codex | Accepted the managed Codex and Claude Code skill distribution contract after guided assessment. |
| 2026-09-01 | r3 | codex | Kept agent invocation syntax in versioned skill documentation so upstream syntax changes do not invalidate the capability contract. |
| 2026-09-01 | r4 | codex | Marked managed Codex and Claude Code skill distribution implemented after release packaging and lifecycle validation shipped. |
| 2026-09-01 | r5 | codex | Added the shared diff-consent and host-permission handoff contract for Codex and Claude Code. |

## Approvals

| Role | Name | Date | Signature |
|------|------|------|-----------|
| Maintainer | fdg | 2026-09-01 | approved in chat; implementation verified through PR #220 |
| Maintainer | fdg | 2026-09-01 | approved in chat for Codex and Claude Code permission handling |
