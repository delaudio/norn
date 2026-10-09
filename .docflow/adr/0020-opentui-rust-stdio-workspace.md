---
adr: 0020
title: Run the interactive workspace on OpenTUI React over a Rust stdio backend
status: Accepted
date: 2026-10-09
owner: default-agent
supersedes: 0006
superseded-by:
depends-on: [0002, 0003, 0004]
tags: [opentui, tui, react, typescript, rust, stdio, ipc, migration, desktop]
---

# ADR 0020 - Run the interactive workspace on OpenTUI React over a Rust stdio backend

## Context

Norn currently ships two interactive surfaces that share one Rust engine: a
Ratatui terminal UI (`src-tauri/src/tui/`) and a Tauri desktop app (React
webview plus the same Rust services). The engine owns the durable, security-
sensitive work - Git, provider HTTP, review and agent execution, analyzers,
credentials, persistence, and configuration - and must stay in Rust.

Both interactive surfaces are costly to evolve. Ratatui makes high-quality
diffs, panels, finding navigation, and agent conversations hard to build and
hard to test visually, and the Tauri webview duplicates a second UI whose
capability set has already drifted from the terminal. The product needs one
interactive workspace that is pleasant to develop with reusable components,
deterministic visual tests, and large-diff performance, without rewriting the
engine.

The migration is authorized by epic #294 and must not weaken the existing trust
boundaries: provider calls and credentials stay in Rust and outside the UI, the
headless CLI and self-hosted service stay Rust-only, the authenticated browser
diff viewer is preserved, and desktop-local user content is migrated rather than
discarded.

## Capability statement

Norn exposes a single interactive terminal workspace built with OpenTUI + React
+ TypeScript that talks to the existing Rust engine through a versioned,
generated, bounded stdio protocol, so the Rust CLI and service remain usable
without Bun, OpenTUI, or a graphical toolkit, and the Tauri/Ratatui surfaces can
be retired once parity is proven.

## User stories / scenarios

- As a reviewer, I can browse repositories and pull requests, read and navigate
  diffs, findings, and agent conversations, stage comments, and publish
  explicitly from one OpenTUI workspace.
- As a contributor, I can build and visually test UI components with native
  frames and screenshot regressions instead of a live terminal.
- As a maintainer, I can run Norn's headless CLI and self-hosted service with no
  Bun, OpenTUI, or Tauri in the active dependency path.
- As a user with desktop-local drafts, references, and prompts, I keep my content
  when the Tauri desktop app is retired.
- As an automation author, my existing `norn review` and related commands keep
  working unchanged.

## Acceptance criteria

1. A bare interactive `norn` invocation on a terminal opens the OpenTUI
   workspace, which starts the local Rust backend itself; a non-interactive
   zero-argument invocation prints CLI help; headless commands remain Rust-only.
2. The Rust engine builds and tests with no Tauri or Ratatui dependency in its
   active dependency tree; the CLI and service run without Bun or OpenTUI.
3. A versioned, typed protocol with a generated contract covers requests and
   results, correlated operation events, cancellation, and snapshots. Standard
   output is protocol-only and standard error is diagnostic; framing and queues
   are bounded; runtime validation rejects unknown or incompatible protocol
   versions.
4. The OpenTUI client is the only interactive UI after cutover, and its
   components are covered by native-frame Storybook scenarios with screenshot
   regressions and input tests.
5. Every shipped terminal and desktop workflow has a disposition of preserved or
   explicitly retired, recorded in the migration parity matrix with an owning
   child issue; no capability is dropped silently.
6. Desktop-local user content (drafts, references, and prompts) is recovered or
   migrated before the desktop app is removed.
7. The authenticated browser diff viewer is preserved with its security policy
   and behavior.
8. Headless CLI and service contracts, supported installation routes, and
   configuration/credential locations and formats are preserved, and a matched
   installed UI/backend pair is tested.
9. Tauri, Ratatui, and obsolete build requirements are removed only at cutover,
   after the functional, visual, lifecycle, and performance gates documented in
   the migration contract pass.
10. Archgate enforcement remains in force during the transition; affected rules
    are updated deliberately, not disabled wholesale.

## Out of scope

- Performing the migration itself; the ordered child issues #295-#309 own the
  implementation.
- Replacing or re-hosting the provider architecture, introducing a network
  service in place of the local engine, or adding a new agent framework.
- Automatic comment publication or any change to the explicit AI-review and
  staged-publication model.
- Treating unshipped backlog features (#256 expanded browser workspace, #251
  native credential platforms, #164 audit backlog) as migration requirements.

## Open questions

- None at the architecture level. The credential-onboarding UX is an explicit
  product decision owned by child issue #305, not an unresolved architecture
  question.

## References

- [Support a terminal UI as a second local review interface](./0006-terminal-ui.md) (superseded)
- [All Bitbucket HTTP lives in Rust](./0002-http-in-rust.md)
- [Credentials in the OS keychain, config in a settings file](./0003-credentials-keychain.md)
- [Diff rendering with react-diff-view](./0004-diff-rendering.md)
- [Offer an authenticated browser diff viewer from the terminal UI](./0016-browser-diff-viewer.md)
- `../../docs/opentui-migration/migration-contract.md`
- `../../AGENTS.md`
- Epic #294 and child issues #295-#309
- Reference workflow: `chtrs` at commit `c7d5ccb73fc49247e9ed6880e5783faf694dd525`

## Revision History

| Date | Revision | Author | Change |
|------|----------|--------|--------|
| 2026-10-09 | r1 | default-agent | Accepted the OpenTUI React over Rust stdio backend migration and the retirement of the Ratatui and Tauri surfaces, superseding the terminal UI decision. |

## Approvals

| Role | Name | Date | Signature |
|------|------|------|-----------|
| Maintainer | delaudio | 2026-10-09 | authorized through epic #294 |
