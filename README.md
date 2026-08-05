# Norn

[![Release][Release]](https://github.com/delaudio/norn/releases)
[![CI][CI]][CI Workflow]
[![Contributing][Contributing]][Issues]

Norn is an open-source, local-first workspace for reviewing Pull Requests on
Bitbucket Cloud and GitHub. It keeps sensitive credentials out of the webview,
keeps review context local, and gives reviewers a structured path from prompt to
published comments. It is available as both a GUI desktop app and a terminal UI.

<details>
<summary>Table of Contents</summary>

- [What Is Norn](#what-is-norn)
- [Quickstart](#quickstart)
- [Documentation](#documentation)
- [CLI, App and TUI](#cli-app-and-tui)
- [Development](#development)
- [Architecture](#architecture)
- [Configuration](#configuration)
- [Roadmap](#roadmap)
- [Contributing](#contributing)
- [Security](#security)
- [License](#license)

</details>

## What Is Norn

Norn is a local review surface around your provider-hosted PRs. It does
not replace Bitbucket or GitHub; it adds an explicit review workflow that is
controlled by the reviewer.

It brings together:

- unified PR browsing across Bitbucket and GitHub;
- a diff-first desktop interface (unified + split views, image previews);
- both interfaces: a Tauri desktop app and a terminal user interface (`norn-tui`);
- reviewer-owned draft comments and publish controls;
- AI-assist runs (Claude or Codex) with local persistence;
- local clone operations for branch sync, fixing, commit, and push flows;
- local review artifacts (runs, findings, evidence, publication state);
- closed-PR analytics and review quality signals.

The same local review contracts and metadata are designed to be usable in
desktop and headless paths, not as ad-hoc one-off scripts.

## Quickstart

### Install Norn

By default, install with Homebrew:

```sh
brew tap delaudio/tap
brew install norn
```

Or run from source with your preferred package runner.

### 5-minute local onboarding

From a repository you own, run:

```sh
cd /path/to/repo
norn init --quick --repo-path . --yes
norn doctor --repo-path .
norn config validate --repo-path .
```

Then start a review:

```sh
norn review --repo-path . --scope working-tree
```

Norn expects repository policy and review defaults in `.norn.yaml`.
Compatibility files are still read during migration only (`.lachesi.yaml`,
`.lachesi/`), with `.norn.yaml` and `.norn/` taking precedence.

### Run GUI App (Tauri)

```sh
pnpm install
pnpm tauri dev
```

This starts the full GUI desktop app with Tauri IPC wired end-to-end.

## Documentation

Most workflows are documented in-repo:

- [Self-hosting guide](docs/self-hosting.md): shared review topology and operations.
- [Homebrew distribution and release runbook](docs/homebrew-distribution.md): install,
  upgrade, rollback guidance.
- [Review evaluation](docs/review-evaluation.md): closed-PR quality gate and score.
- [Architecture and migration specs](docs/specs): policy engine, findings schema,
  repository config, publication model.

Additional product references are also available:

- `SECURITY.md`: safe handling of secrets and local data.
- `LICENSE`: license terms.

For API consumers and AI-assisted flows, the local configuration is the source of
truth: keep local config files in the repository and review commands explicit.

## CLI, App and TUI

Norn ships both a desktop app and terminal interfaces:

- `norn`: review/repository CLI.
- `norn-tui`: terminal interface for PR browsing and review actions.

Build and install the canonical CLI from source:

```sh
make cli-build
make cli-install
norn --version
```

Build/install the terminal UI:

```sh
make tui-build
make tui-install
norn-tui --version
```

`norn-tui` runs inside a local Git repo and resolves GitHub/Bitbucket from
the configured remote.

```sh
norn-tui --workspace
```

`norn-tui --workspace` opens repository picker mode when the current directory
is not a Git checkout with a supported remote.

Credentials for terminal/headless usage can be supplied by OS keychain, or from
environment variables referenced in `~/.config/norn/config.toml`, for example:

```toml
[credentials.github]
token_env = "GITHUB_TOKEN"

[credentials.bitbucket]
username_env = "BITBUCKET_USERNAME"
token_env = "BITBUCKET_TOKEN"
```

Keep real secrets in environment or keychain only.

## Development

Run the core quality gates before submitting a change:

```sh
pnpm install
pnpm run typecheck
pnpm run test
pnpm run test:tauri
pnpm run lint
pnpm run build
```

Additional scripts are available for docs and design system publishing:

```sh
pnpm run storybook
pnpm run storybook:build
pnpm run storybook:deploy
pnpm run docs:dev
pnpm run docs:build
pnpm run docs:deploy
```

Local-only development can use the browser mock layer (for UI and review flow
experiments) with `pnpm dev`.

## Architecture

Norn is split into a React frontend and Rust/Tauri backend:

- **Frontend:** React 19, TypeScript, Vite, Tailwind.
- **Backend:** Tauri v2 with Rust commands over IPC.
- **Providers:** Bitbucket Cloud and GitHub are handled server-side in Rust.
- **State:** local React/Tauri state and local persistence for review models.
- **Storage:** settings and credentials are handled separately; review state is
  persisted locally in SQLite.

Important architectural boundaries:

- Credentials are not injected into web content.
- All provider interactions happen in Rust command handlers.
- `src/lib/tauri.ts` is the single frontend IPC boundary.
- Mock handlers in `src/mock-tauri/` keep browser, Storybook and test flows
  functional without a real provider backend.

## Configuration

Project-level app settings include:

- selected providers and repositories;
- local clone integration (branch/sync/fix/commit/push);
- default diff mode and AI runtime mode;
- optional Jira/Notion integration for context.

Per-repository review control is in `.norn.yaml`, with the current contract
including review mode, prompt extension, finding thresholds, rule list and local
analyzers.

Example:

```yaml
version: "0.1"
review:
  mode: balanced
  prompt:
    extend: "Prioritize migration safety and public API usage changes."
  findings:
    minSeverity: low
    requireAnchors: false
paths:
  include:
    - "src/**"
  exclude:
    - "dist/**"
```

## Roadmap

Current focus:

- solidify `.norn.yaml` policy semantics and evidence pipeline;
- expand policy packs and named review profiles;
- harden headless `norn review` for local and CI;
- improve provider abstraction and report/export quality;
- continue the review engine migration with structured contracts.

Public planning artifacts remain in GitHub issues and the project’s roadmap flow.

## Contributing

We follow lightweight contribution flow:

- open an issue or discussion before large architectural changes;
- keep PRs focused and testable;
- include commands run and scope in the PR description;
- align with the existing ADR and docs-led conventions in this repository.

Before opening a PR, please:

- use GitHub issues for context and discussion;
- keep the change focused and testable;
- include the exact commands you ran in the PR description.

AI-assisted changes are accepted when they are reviewable and scoped to the
problem they solve.

## Security

Security is mostly about secrets hygiene:

- do not commit tokens;
- do not paste production secrets in PR descriptions, screenshots, or fixtures;
- run with provider tokens in environment/OS store when possible.

For security concerns, use `SECURITY.md` procedures.

## License

Norn is released under the license in `LICENSE`.

[Release]: https://img.shields.io/github/v/release/delaudio/norn?label=Norn&sort=semver
[CI]: https://github.com/delaudio/norn/actions/workflows/release-norn-macos.yml/badge.svg
[CI Workflow]: https://github.com/delaudio/norn/actions/workflows/release-norn-macos.yml
[Contributing]: https://img.shields.io/badge/CONTRIBUTING-guidelines-0ea5e9?logo=github&style=flat-square
[Issues]: https://github.com/delaudio/norn/issues
