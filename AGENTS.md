# AGENTS.md

This file provides guidance to coding agents working in this repository.

## What this repository is

Norn is an open-source, local-first review workspace for pull requests
across Bitbucket Cloud and GitHub. It is a Tauri v2 desktop app with a Rust
backend (`src-tauri/`) and a React 19 + TypeScript + Vite frontend (`src/`).
Provider access, local evidence, review findings, and publication workflows
are controlled from the local app; credentials must stay out of the webview.

## Repository structure

- `.docflow/adr/0000-template.md` - canonical ADR template.
- `.docflow/adr/NNNN-<kebab-slug>.md` - one ADR per decision, contiguous
  numbering, no gaps.
- `.docflow/INDEX.md` - table regenerated from every ADR metadata block.
- `.docflow/CONVENTIONS.md` - authoring rules; read before editing ADRs,
  plans, or agent coordination files.
- `.docflow/plan/todo/NNNN-<slug>.md` - pending ADR-backed work, lower
  numbers run first.
- `.docflow/plan/done/<YYYY-MM-DD>-<slug>.md` - shipped ADR-backed work,
  chronological.
- `.docflow/_agent/` - single-agent coordination: `ROLES.md`, `WORKLOG.md`,
  `CURRENT_FOCUS.md`, `HANDOFF.md`, and `prompts/`.
- `.archgate/adrs/` - Archgate-enforced architectural rules and checks. Keep
  this catalogue in place; docflow does not replace it.

## Commands

```bash
pnpm run dev          # start Vite dev server (browser mode, mock IPC)
pnpm run typecheck    # tsc --noEmit
pnpm run test         # Vitest/jsdom test suite
pnpm run lint         # Biome checks
pnpm run build        # TypeScript + Vite production build
pnpm tauri dev        # start full Tauri app
archgate check        # run ADR compliance checks
```

Credentials for Tauri dev: `BITBUCKET_USERNAME` and `BITBUCKET_TOKEN` env
vars must be set before launching.

Repository commands are also wrapped by platform-native task runners at the
repo root. Use `make <recipe>` on macOS/Linux and `just <recipe>` on Windows.
Both must expose the same recipe names. Any recipe added, renamed, or removed
must be mirrored in both `Makefile` and `justfile`; Archgate checks enforce
this parity.

## Hard rules when editing ADRs

These come from `.docflow/CONVENTIONS.md` and override default behaviour:

- One decision per ADR. Splits become new ADRs that supersede; never expand
  scope inside an existing one.
- Status lifecycle: `Proposed -> Accepted -> Implemented -> (Superseded |
  Deprecated)`.
- ADR section order: metadata -> Context -> Capability statement -> User
  stories / scenarios -> Acceptance criteria -> Out of scope -> Open questions
  -> References -> Revision History -> Approvals.
- Acceptance criteria are testable and numbered.
- ADRs are internal artefacts. ADR numbers, ADR titles, and the existence of
  the ADR catalogue must never appear in product strings a user can read: UI
  copy, API responses, customer-visible errors/logs, public docs, release
  notes, marketing copy, or support communications.
- Product exception: Norn may expose and document a generic `.norn.yaml`
  policy-source type named `adr`. Do not expose this repository's internal ADR
  numbers, titles, or catalogue paths through that exception.
- `.lachesi.yaml`, `.lachesi/`, and `.lachesi.local.yaml` are legacy read-only
  fallbacks. New examples, instructions, and writes use the canonical Norn
  equivalents.
- References are allowed in code comments, commit messages, PR descriptions,
  and internal docs such as `AGENTS.md`, `.docflow/CONVENTIONS.md`,
  `.docflow/INDEX.md`, and `.docflow/plan/`.

## Implementation work

- Start from the ADRs and the Archgate rules. Identify which decisions a code
  change implements or affects before changing behaviour.
- If implementation reveals a capability gap or changed decision, update the
  relevant ADR rather than silently diverging.
- Add or update tests for implemented behaviour. Map tests back to ADR
  acceptance criteria where practical.
- Do not leak ADR identifiers into user-visible surfaces. Put the ADR link in
  the commit message, PR description, or an internal code comment instead.

## Audit trail and revision discipline

- Substantive ADR changes append a row to the Revision History table.
  Editorial changes are excluded but should be flagged `editorial` in the
  commit message.
- Approvals populate when an ADR is Accepted and update on each later
  substantive revision.
- Regenerate `.docflow/INDEX.md` after any ADR status change or new ADR.

## Multi-agent workflow

A single agent owns this repo. The `.docflow/_agent/` directory tracks live
state and history; LOCKS discipline is not in use.

- Update `.docflow/_agent/CURRENT_FOCUS.md` when a session changes state.
- Append to `.docflow/_agent/WORKLOG.md` when committing docflow-managed work.
- Use `.docflow/_agent/HANDOFF.md` as the entry point for a fresh session.

- Before integrating, check for number collisions. Sync onto the current
  `main` and run the docflow audit; if a new ADR or `plan/todo` number clashes
  with what landed on `main`, renumber locally before integrating. Numbers are
  immutable once merged.

## Plan folder

- A pending item gets a `.docflow/plan/todo/NNNN-<slug>.md` file before work
  starts, naming the owning ADR(s), scope, and exit criteria.
- Completion event: pull request merged to `main` with the local gate passing:
  `pnpm run typecheck`, `pnpm run test`, and `archgate check`.
- On completion, move the item to `.docflow/plan/done/<YYYY-MM-DD>-<slug>.md`
  with a footer naming the shipped commit or bootstrap base.
- The owning ADRs advance `Accepted -> Implemented` on the same change.
  Regenerate `.docflow/INDEX.md`.

## Git contract

- Commit messages follow Conventional Commits.
- Commits touching ADRs include a `Rationale:` footer.
- Signed commits are expected.
- ADR revision tags such as `adr-NNNN-rN` are not used.
- Agent commits do not include `Co-Authored-By` trailers unless the user asks.
- Cross-references between ADRs use relative paths from the current file.
- Integration is PR-based with squash merge as the default strategy.

## Key architecture notes

All external provider calls go through Rust commands behind Tauri IPC. The
frontend calls `tauriCall(commandName, args)` from `src/lib/tauri.ts`; every
registered Rust command in `src-tauri/src/lib.rs` must have a matching mock
handler in `src/mock-tauri/mock-handlers.ts`.

When invoking a user-installed CLI binary from Rust Tauri commands, use a zsh
login shell:

```rust
Command::new("/bin/zsh")
    .arg("-lc")
    .arg(&shell_cmd)
    .output()?
```

macOS GUI-launched apps have a minimal `PATH` that omits user install
locations. Without `/bin/zsh -l`, binaries installed by Homebrew, npm, or local
CLI installers may not be found.

App-generated data lives under `dirs::data_local_dir()` in the `norn`
subdirectory. The legacy `lachesi` subdirectory is migration input only.
Secrets must stay in the OS credentials store or local environment, never in
repo config, examples, screenshots, or fixtures.

CSS custom properties in this project are hex color values, not HSL component
tuples. Use `fill="var(--primary)"`, not `fill="hsl(var(--primary))"`.

Navigation is state-driven through the `AppSelection` union type, not React
Router. Browser dev and Storybook use the mock IPC layer in
`src/mock-tauri/`.
