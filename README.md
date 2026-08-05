# Norn

Norn is an open-source, local-first review workspace for pull requests across Bitbucket Cloud
and GitHub.

It combines a focused desktop review UI with AI-assisted review runs, structured findings, local
evidence, closed-PR analytics, and reviewer-controlled publication back to the source provider.

The name comes from the Norns, the figures who shape fate in Norse mythology. Norn follows the
thread of a change: the PR metadata, diff, linked context, local repository state, review findings,
and the comments a human reviewer decides to publish.

## What It Is

Your code host remains the source of truth for pull requests. Norn is the local review surface
around it.

Today it is a Tauri desktop app that can:

- track multiple Bitbucket Cloud and GitHub repositories in one sidebar;
- browse open, draft, merged, and declined pull requests;
- render readable unified or split diffs with syntax highlighting and diff stats;
- preview changed image files such as PNG, JPEG, SVG, GIF, and WebP;
- show PR metadata, reviewers, approvals, branch age/status, and existing comments;
- stage inline or general review comments locally before publishing them;
- approve PRs through the configured provider API;
- run AI review flows with Claude or Codex, persist review threads, and continue the conversation;
- turn AI review output into structured findings and draft comments;
- run local fix, commit, push, branch sync, and conflict-resolution workflows from a configured clone;
- sync and analyze closed PRs by author, repository, churn, lead time, risk, and review coverage;
- enrich review context with Jira issues and Notion pages when credentials are configured;
- store review history, review jobs, run logs, findings, evidence, and publication state locally.

The long-term direction is a local/open-source review engine: desktop first, but with shared review
contracts that can power a future CLI, CI usage, policy checks, private policy packs, and public
documentation.

For the optional self-hosted shared-review process, see
[`docs/self-hosting.md`](docs/self-hosting.md).

For the versioned offline review-quality corpus and regression gate, see
[`docs/review-evaluation.md`](docs/review-evaluation.md).

## Why

Web review UIs can feel cramped and noisy for repeated, high-context review work. More importantly,
modern review is no longer just "read a diff and leave comments". A good reviewer often needs to
inspect local branches, run deterministic checks, read linked tickets/docs, ask an AI assistant for a
second pass, curate the useful findings, and decide what to publish.

Norn keeps that workflow local and explicit:

- credentials stay out of the webview;
- AI output is treated as draft review material, not automatic remote feedback;
- provider writes require reviewer action;
- repo-specific rules live near the code through `.norn.yaml`;
- review output is moving toward a structured schema instead of free-form chat only.

## Current Product Surface

### Provider Support

- Bitbucket Cloud repositories using workspace/repository coordinates.
- GitHub repositories using owner/repository coordinates.
- Separate provider configuration panels and separate local credentials.
- Bitbucket credentials use Atlassian email + API token.
- GitHub credentials use a GitHub token stored separately in the local credentials layer.
- Provider-aware PR list, PR detail, diff, diffstat, comments, approvals, branch status, image
  previews, local clone discovery, and closed-PR analytics.

### Pull Request Review

- Multi-repository PR list with repository and author filters.
- Open, draft, merged, and declined PR states.
- PR detail header with branches, reviewers, approvals, and branch status.
- Unified and split diff modes powered by parsed provider diffs.
- File-level diff stats and changed-file navigation primitives.
- Image previews for binary image changes when the provider can return the file content.
- Existing provider comments and local draft comments rendered in context.
- Local pending-review bar for staged comments, with publish/discard controls.

### AI Review Workspace

- Claude or Codex review runs from the active PR.
- Replyable review threads persisted per PR.
- Review run state, logs, cancellation, success/failure tracking, and review history.
- Structured `ReviewRun`, `ReviewFinding`, and `ReviewEvidenceArtifact` models layered alongside
  the conversational thread.
- Finding publication tracking for staged drafts and published provider comments.
- AI fix workflow that operates against the local clone and can progress through fix, verification,
  commit, and push phases.

### Local Repository Integration

Each tracked repository can point to a local clone. That enables:

- branch status checks;
- branch checkout/fetch/pull utilities;
- PR branch synchronization;
- conflict-resolution assistance;
- AI fix execution in the worktree;
- commit and push steps controlled by the reviewer.

### Closed PR Analytics

Norn can cache and analyze closed PRs locally. The analytics view supports:

- 14/30/90 day ranges;
- repository, author, and text filtering;
- PR count, churn, average changed files, average close time, and AI review coverage;
- charts by author, repository, churn, risk category, and weekly frequency;
- a horizontally scrollable largest-PR table with opened and closed dates.

### Context Integrations

- Jira issue extraction from PR context, using an Atlassian site URL and Jira token.
- Notion page extraction from linked Jira content, using a Notion token.
- Manual and detected review references that can be sent with the review prompt.

### Local Storage And Secrets

- Bitbucket, GitHub, Jira, and Notion credentials are stored through the local credentials layer.
- Non-secret app settings are stored as JSON in the OS config directory.
- AI review stores and background review jobs are stored in a local SQLite database.
- Legacy JSON review storage is migrated into SQLite when read.
- Before the first canonical desktop window opens, Norn atomically copies the
  legacy WebView profile into the canonical application identifier and retains
  the old profile for rollback.
- `NORN_DRY_RUN=1` can exercise comment flows without posting to the provider. The legacy
  `LACHESI_DRY_RUN` name remains a fallback during the compatibility window.

## Repo-Owned Review Config

Norn can read `.norn.yaml` from a configured local repository. During the compatibility window it
also reads `.lachesi.yaml`, `.lachesi/`, and `.lachesi.local.yaml` only when the corresponding Norn
source is absent. Multiple repository roots, including `.norn.yaml` plus `.norn/`, fail with
migration guidance instead of silently choosing or merging. The current schema already
models the direction of the review engine:

```yaml
version: "0.1"
review:
  mode: balanced
  prompt:
    extend: "Pay special attention to migration safety and public API changes."
  findings:
    minSeverity: low
    requireAnchors: false
paths:
  include:
    - "src/**"
  exclude:
    - "dist/**"
policy:
  rules:
    - id: no-cross-module-imports
      severity: medium
      instruction: "Flag imports that cross module ownership boundaries."
analyzers:
  tsc:
    enabled: true
    command: "pnpm typecheck"
    timeoutSeconds: 120
publish:
  defaultMode: inline
  requireManualSubmit: true
```

Some of this is already used for prompt/config loading; some is intentionally documented as the
contract for the next review-engine milestones.

## Architecture

Norn is split across a React frontend and a Rust/Tauri backend.

- **Frontend:** React 19, TypeScript, Vite, Tailwind v4, shadcn-style primitives, Radix components,
  Phosphor icons, Geist fonts.
- **Desktop shell:** Tauri v2.
- **Provider access:** Rust `reqwest` commands behind Tauri IPC for Bitbucket Cloud and GitHub.
  Tokens never need to enter the browser webview.
- **State model:** React hooks plus Tauri commands; no external frontend state library.
- **IPC boundary:** frontend calls go through `src/lib/tauri.ts`, which can route to mock handlers
  for browser dev, Storybook, and Vitest.
- **Diff rendering:** `gitdiff-parser`, `react-diff-view`, `react-virtuoso`, and `refractor`.
- **Persistence:** OS config JSON for settings, local credentials storage for secrets, SQLite for
  review stores/jobs.
- **Review model:** structured findings, evidence, publication state, and chat threads share the
  same TypeScript/Rust DTO vocabulary.

Implementation specs live in:

- `docs/specs/0001-findings-schema.md`
- `docs/specs/0002-bitbucket-publication-model.md`
- `docs/specs/0003-repository-config.md`
- `docs/specs/0004-policy-engine.md`
- `docs/specs/0005-local-evidence-pipeline.md`
- `docs/specs/0006-cli-headless-review.md`

## Development

### Requirements

- Node.js and pnpm
- Rust toolchain
- Tauri v2 prerequisites for your OS
- Bitbucket Cloud account email and API token for Bitbucket usage
- GitHub token for GitHub usage
- Claude CLI and/or Codex CLI available locally for AI review/fix flows

### Install

```sh
pnpm install
```

### Run

```sh
pnpm dev
```

Runs the Vite app in browser mode with mock IPC on port `5210`.

```sh
pnpm tauri dev
```

Runs the desktop app against the Tauri backend.

### Headless CLI

Build and install the canonical `norn` CLI plus the deprecated `lachesi`
compatibility alias:

```sh
make cli-build
make cli-install
norn --version
```

The Codex review integration uses this installed `norn` executable.

### Terminal UI

Build and install the terminal UI as `norn-tui`:

```sh
make tui-build
make tui-install
```

Run `norn-tui` from inside a local Git clone. It reads the repository remote, detects GitHub or
Bitbucket, and opens pull requests for that repository:

```sh
cd ~/dev/current/norn
norn-tui
norn-tui --version
```

Use `norn-tui --workspace` to open the configured repository picker instead. If the current directory is
not inside a Git repository, has no remote, or uses an unsupported remote host, `norn-tui` exits with a
message explaining the problem and the `--workspace` fallback.

The legacy `lac` command remains a deprecated compatibility alias for the documented migration
window.

The TUI detects PNG, JPEG, GIF, and WebP changes and queries the active terminal after entering the
alternate screen. Kitty, Sixel, and iTerm2 protocols render the selected image inside the diff pane.
Press `i` on a modified image to switch between its base and changed versions. Other terminals, or
images that cannot be decoded, keep a metadata/error view instead of emitting graphics escapes.
Provider image reads are limited to 8 MiB, with decoding limited to 8192 pixels per side and 40
megapixels. Animated formats show their first decoded frame.

For terminal-only usage, provider credentials can come from the OS keychain, ordinary environment
variables, or environment-variable references in `~/.config/norn/config.toml`:

```toml
[credentials.github]
token_env = "GITHUB_TOKEN"

[credentials.bitbucket]
username_env = "BITBUCKET_USERNAME"
token_env = "BITBUCKET_TOKEN"
```

Keep actual tokens in the environment or OS credentials store, not in the TOML file.

### Test And Build

```sh
pnpm test
pnpm test:tauri
pnpm lint
pnpm build
pnpm storybook
pnpm storybook:build
pnpm storybook:deploy
pnpm docs:dev
pnpm docs:build
pnpm docs:deploy
```

`pnpm build` runs TypeScript and Vite production build. `pnpm lint` runs Biome.
`pnpm test` runs the regular jsdom/Vitest suite. `pnpm test:tauri` runs the separate Tauri IPC
smoke lane, which boots a mock Tauri webview and calls selected low-risk Rust commands through the
real IPC dispatcher without publishing comments or mutating remote providers. CI should run it as a
separate job from the browser/jsdom test lane because it depends on the Rust/Tauri toolchain.
`pnpm storybook:build` creates the static Storybook output in `storybook-static/`.
`pnpm storybook:deploy` deploys the built Storybook through the dedicated Cloudflare Workers Static
Assets config.
`pnpm docs:dev` and `pnpm docs:build` run the Astro/Starlight documentation site in `apps/docs`.
`pnpm docs:deploy` deploys the built documentation site through the dedicated Cloudflare Workers
Static Assets config.

### macOS arm64 builds

macOS arm64 app bundles are not signed. If macOS reports that `Norn.app` is damaged and cannot
be opened, clear the quarantine attributes and apply an ad-hoc local signature to the built app:

```sh
xattr -rc "Norn.app"
sudo codesign --force --deep --sign - "Norn.app"
```

Run those commands against the copied app bundle, for example the app in `/Applications` or the
bundle produced by `pnpm tauri build`.

### Published Storybook

The design system Storybook is prepared for publication at:

```txt
https://design-system.lachesi.dev
```

Deployment instructions for Cloudflare are in `docs/storybook-publishing.md`.

## Configuration

In the app settings, configure:

- review provider: Bitbucket or GitHub;
- one or more provider-specific repositories;
- optional local clone paths for branch, fix, commit, push, and sync workflows;
- Atlassian email and Bitbucket API token;
- GitHub token;
- default diff view;
- AI provider: Claude or Codex;
- Claude/Codex model and effort;
- preferred terminal for review/fix launches;
- optional Jira base URL, Jira token, and Notion token;
- automatic sync interval, menu-bar sync, and desktop notifications.

### Menu Bar Legend

The macOS menu bar item uses compact status symbols:

- `●` idle or normal open pull request;
- `○` pull request sync disabled;
- `◌` draft pull request;
- `↻` sync running or sync action;
- `▶` background review running or review action;
- `✓` merged pull request;
- `×` declined or superseded pull request.

For development, `.env` fallbacks may be used by local code paths, but secrets should not be
committed. See `SECURITY.md`.

## Roadmap

The public roadmap is tracked in GitHub issues and the Norn roadmap project. The current direction
is:

- harden the `.norn.yaml` policy engine and local evidence pipeline;
- add policy pack loading and named review profiles;
- continue hardening the headless `norn review` CLI for local and CI usage;
- improve provider abstraction across Bitbucket, GitHub, AI providers, and publication targets;
- expand report export, review summaries, and dogfooding guides;
- publish the website and design system documentation.

## Project Status

Norn is early and moving quickly. The desktop review workflow is usable, but the structured
review engine is still being hardened. Expect API/schema changes while the project converges on the
v0.1 contracts described in `docs/specs`.

## Security

Do not open public issues for credential exposure, private repository data, or local path leaks.
Report vulnerabilities privately through GitHub Security Advisories when available.

Credentials and tokens must stay in the OS credentials store or local environment, never in repo
config, examples, screenshots, or fixtures.

## License

Norn is released under the license in `LICENSE`.
