# OpenTUI migration contract

Internal implementation contract for replacing the Ratatui terminal UI and the
Tauri desktop app with an OpenTUI + React + TypeScript workspace over the
existing Rust engine. Authorized by ADR 0020 and epic #294.

This document is the migration source of truth for paths, executable names,
command behavior, supported platforms, measurement, and the per-capability
parity matrix. It is an internal artefact: it may reference ADR and issue
numbers and must not appear in product strings.

## 1. Package boundaries

| Part | Path | Package / binary | Toolchain | Notes |
| --- | --- | --- | --- | --- |
| Rust engine, headless CLI, service, stdio backend | `src-tauri/` | crate `norn`; binaries `norn`, `norn-tui` (compat), `norn-app` (compat), `norn-backend` | Rust (pinned) | Directory name stays `src-tauri/` during the transition; renamed once at cutover (#309). |
| OpenTUI workspace | `ui/` | package `@norn/ui` | Bun + OpenTUI + React + TypeScript | New interactive surface; owns presentation and transient interaction state. |
| Retained browser diff assets | `src/browser-diff/`, `browser-diff.html`, `vite.browser-diff.config.ts` | served HTML/JS | existing pnpm/Vite | Served by the Rust `diff_server`; preserved unchanged. |
| Docs and web | `apps/docs/`, `apps/web/`, Storybook | existing | existing pnpm | Stay on the current toolchain until cutover. |

Rules:

- The TypeScript layer owns presentation and transient interaction state only.
  Git, providers, review/agent execution, analyzers, persistence, configuration,
  and credential access stay in Rust.
- The OpenTUI workspace must not depend on a sibling checkout; it is a workspace
  package inside this repository.
- The Rust engine and headless binaries must build with no Tauri or Ratatui
  dependency in their active dependency tree.

## 2. Command and runtime contract

- Interactive zero-argument `norn` on a terminal opens the OpenTUI workspace.
  The workspace launches `norn-backend` itself over stdio.
- Non-interactive zero-argument `norn` prints CLI help (unchanged).
- Headless commands (`review`, `doctor`, `setup`, `init`, `metrics`, `config`,
  `policy`, `evaluate`, `auth`, `skills`, `service`) remain Rust-only and are
  unaffected by the UI's runtime.
- Backend process contract: standard output carries only the versioned protocol;
  standard error carries diagnostics. Framing and queues are bounded; the
  workspace and backend each validate protocol version and degrade with a clear
  error rather than proceeding on a mismatch.
- Compatibility aliases during the transition:
  - `norn-tui` launches the OpenTUI workspace (was the Ratatui TUI); `lac`
    remains its deprecated alias.
  - `norn-app` and `lachesi` remain recognized compatibility aliases; the
    desktop GUI is retired at cutover (#309), not before.
- The `desktop-bundle` and `custom-protocol` Cargo features keep their current
  meaning until cutover. No interactive UI is delivered through a bare
  command-distribution binary.

## 3. Protocol (shape only)

Owned by child issue #297. At minimum a versioned, generated TypeScript/Rust
contract with: request/response envelopes keyed by a correlation id; correlated
operation events (progress, streamed logs, terminal states); cancellation;
snapshot/resync; and bounded resource use. Generated request and response
schemas must be validated at the boundary; unknown versions fail closed.

## 4. Supported platforms and pinned versions

| Platform | Interactive workspace | Headless CLI / service |
| --- | --- | --- |
| macOS arm64 (Apple Silicon) | supported (primary) | supported |
| macOS x86_64 (Intel) | supported | supported |
| Linux x86_64 | supported where OpenTUI renders; headless always supported | supported |
| Windows x86_64 | supported where OpenTUI renders; NSIS packaging lane retained | supported |

- Pinned toolchain versions and their sources:
  - Rust: `1.94.0`, pinned in `.github/workflows/release-norn-macos.yml`
    (`RUST_TOOLCHAIN`).
  - Bun and OpenTUI: pinned in `ui/package.json` (`packageManager` and the
    OpenTUI dependency) when the workspace lands in #299.
  - Existing web/docs/Storybook workspace: `pnpm` via Corepack (no explicit pin
    today); it keeps its current toolchain until cutover.
- CI validates the pinned versions in the corresponding manifests before their
  gates become required, and both toolchains are documented in `AGENTS.md` before
  cutover.

## 5. Performance measurement procedure

Owned by child issues #301 (vertical slice) and #307 (hardening). Baseline and
gate procedure:

- Inputs: a synthetic large diff and a real large pull-request diff, plus a
  streamed agent conversation.
- Metrics: first-frame time, per-file diff render time, scroll/frame time under
  load, memory ceiling, and backend round-trip latency.
- Method: deterministic fixtures, repeated runs, recorded thresholds in the
  child issue; failures block further cutover work.
- Screenshot and input tests use native OpenTUI frames rendered in Storybook, not
  a live terminal.

## 6. Architecture enforcement during the transition

- Archgate rules remain enabled. Rules that encode the Tauri/React boundary
  (ARCH-001, ARCH-003, ARCH-004, ARCH-006, FE-*) continue to apply to the
  retained desktop/web surfaces until they are removed.
- New enforcement for the Rust/stdio boundary and the OpenTUI workspace is added
  deliberately by the relevant child issue; rules are updated, not disabled.
- No child issue may remove a rule to make a change pass.

## 7. Scope reconciliation of pending desktop issues

- **#255 (separate desktop review orchestration from large React components):**
  not a migration prerequisite. The Tauri desktop UI is being retired; the
  Rust-side extraction is covered by #296 and the new presentation layer by
  #299/#303. Keep the issue open but off the migration critical path; if the
  desktop app survives past cutover it would be re-scoped.
- **#253 (restrictive CSP for the desktop webview):** tied to the Tauri webview
  and therefore obsoleted by the migration. The equivalent protection for the
  retained browser diff viewer already exists (ADR 0016). Do not implement the
  desktop CSP as part of the migration; close or re-scope #253 with the
  maintainer at cutover. Open PR #259 is in this category.
- **#284 (protect `usePullRequest`/`useBranchStatus` refreshes from stale
  responses):** a correct, provider-agnostic behavior. It applies to the
  desktop hooks that are being retired; the equivalent guarantee is required of
  the new client's request lifecycle (#299) and its review flows (#303). Keep
  the issue open for the remaining desktop until cutover, and carry the
  requirement into the OpenTUI client's tests.
- **Separate, not migration requirements:** #256 (expanded browser workspace),
  #251 (native credential platforms), #164 (audit backlog). They remain their
  own scope. Native credential-platform support stays a Rust concern (#296/#305).

## 8. Parity matrix

Disposition: `port` = reimplement in the OpenTUI client/backend; `preserve` =
keep unchanged (typically Rust or browser assets); `retire` = intentional
removal with maintainer decision; `shared` = Rust/native service reused as-is.

| Capability | Surface | Primary source | Persistence | Destination | Disposition |
| --- | --- | --- | --- | --- | --- |
| Repository list/selection, current-repo resolution | both | `tui/mod.rs`, `App.tsx`, `PrSidebar` | `config.rs` settings | #299 | port |
| PR list + Open/Draft/Merged/All filters, paging | both | `tui/mod.rs` `PrListFilter`, `PrStateTabs`, `usePullRequests` | provider fetch | #299 | port |
| Author/repository filters | desktop | `AuthorFilter`, `RepositoryFilter` | in-memory | #299 | port |
| Local review target (working tree / branch), base selection | both | `tui/mod.rs`, `local_review.rs`, #268/#289 | local git snapshot | #301/#302 | port |
| PR detail metadata, reviewers/approvals | both | `render.rs`, `PrDetailPanel`, `PrHeader` | provider fetch | #301 | port |
| Comments/threads + replies + composer | desktop (TUI shows count only) | `comments/*`, `useComments` | provider + draft store | #304 | port |
| Diff: unified/split/conversation, per-file nav, large-diff collapse | both | `render.rs`, `DiffViewer`, `FileDiff`, `FileTree` | `viewedFilesStorage` | #301/#302 | port |
| Syntax highlighting | both | `delta` subprocess (TUI), `refractor` (desktop) | none | #302 | port |
| Image diffs (terminal protocols; single preview) | both | `tui/image_diff.rs`, `FileDiff.ImagePreviewPanel` | none | #302 | port |
| Authenticated browser diff viewer | TUI + shared assets | `tui/diff_server.rs`, `browser-diff/*` | session token | #302 | preserve |
| Drafts/composer staged comments + explicit publish | both | `tui/mod.rs`, `useDraftComments`, `reviewService` | localStorage (desktop), session (TUI) | #304 | port |
| Structured finding publication + reconciliation | desktop | `reviewFindingPublication.ts`, `reviewService.ts`, Rust `publish_review_finding` | Rust review store | #304 | port |
| AI review start + streaming logs | both | `tui/mod.rs`, `AiReviewPanel`, `useAiReview` | Rust run store | #303 | port |
| AI findings panel + navigation | desktop | `AiReviewPanel` | Rust review store | #303 | port |
| AI conversation threads + replies + cancel | desktop | `useAiReview` | Rust review store + session | #303 | port |
| AI fix / commit / push / conflict resolution | desktop | `useAiReviewFix`, `buildAiFixPayload` | Rust fix store | #303 | port |
| Branch sync (PR source→destination) | desktop | `useBranchSync` | in-memory | #303 | port |
| Settings: AI provider/model/effort | both | `tui/mod.rs` settings, `SettingsDialog` | `config.rs` settings | #305 | port |
| Settings: credentials (GitHub/Bitbucket) + readiness | both | `tui/mod.rs` settings, `useCredentials` | keychain + `config.toml` | #305 | port |
| Settings: repos, diff view, sync, Jira/Notion tokens | desktop | `SettingsDialog` | `config.rs` + keychain | #305 | port |
| Credential onboarding (OAuth/GitHub App) | none shipped | `bitbucket_oauth_onboarding.rs`, `github_app_onboarding.rs` | enrollment tables | #305 | port (onboarding UX decided on #305) |
| Menu-bar sync + tray | desktop | `useMenuBarPrSync`, `lib.rs` `setup_menu_bar` | localStorage snapshot | #306 | retire |
| Native notifications | desktop | `useMenuBarPrSync`, `App.tsx` | localStorage snapshot | #306 | retire |
| Automatic sync polling | desktop | `useAutomaticSyncPolling` | `config.rs` | #305 | port |
| Repository explorer (tree/content/blame/diff/find) | desktop | `RepositoryExplorerPanel`, `localRepoService` | in-memory | #306 | port |
| Branch/worktree operations | desktop | `RepositoryBranchesPanel`, `localRepoService` | local git | #306 | port |
| Review history panel | desktop | `ReviewHistoryPanel` | Rust review store | #306 | port |
| Analytics: closed-PR metrics, review effectiveness | desktop | `OverviewPanel`, `ClosedPrAnalyticsPanel`, `ReviewEffectivenessDashboard` | Rust review store | #306 | port |
| References panel (PR/Jira/Notion/note) | desktop | `ReviewReferencesPanel`, `reviewReferencesStorage` | localStorage | #305 | port |
| Review prompt content (repo + user) and AI conversation user prompts | both | `reviewPrompt.ts`, `.norn.yaml` `review.prompt`, `AiReviewPanel` threads | repo config + Rust review store | #303 | port |
| Desktop-local user data (drafts, references, user prompts) | desktop | localStorage stores + Rust review store | localStorage + SQLite | #305 | migrate |
| Jira/Notion context in AI payload | desktop | `useReviewContext`, `jira.ts` | keychain tokens | #303 | port |
| Clipboard copy | both | `tui/terminal.rs` OSC52, `lib/clipboard.ts` | none | #302 | port |
| Shortcuts help / theme | desktop | `ShortcutsDialog`, `useTheme` | localStorage | #305 | port |
| Find in repository | desktop | `RepositoryExplorerPanel` | in-memory | #306 | port |
| External file open | desktop | `localRepoService` `open_repository_file_external` | none | #306 | port |
| Tray + menu-bar review trigger | desktop | `App.runBackgroundMenuReview`, `backgroundReviewStart` | Rust review store | #306 | retire |
| Readiness preflight / `--skip-readiness` / `--current-repo` | TUI | `tui/mod.rs` | `config.rs` | #299 | port |
| Headless CLI/service | CLI | `cli.rs`, `headless_review.rs`, `self_hosted_service.rs` | Rust stores | n/a | preserve |

Every row has an owning child issue and a single disposition. Dispositions:
`port` (reimplement in the OpenTUI client/backend), `preserve` (keep unchanged;
typically Rust or browser assets), `retire` (intentional removal), `migrate`
(recover existing user data), `shared` (reuse the Rust/native service). The
retirements - desktop tray and menu-bar sync, desktop OS notifications, the
desktop CSP, and the menu-bar background-review trigger - are explicit maintainer
decisions recorded by epic #294; they are not silent drops. Credential-onboarding
UX is the only open product decision and is an explicit blocker on #305.

## 9. Vertical slice and cutover gates

- **First vertical slice (#301):** real file → diff → finding through the Rust
  backend and the OpenTUI client, measured on a large diff. Material
  performance or correctness failures block further cutover work.
- **Final cutover (#309):** full functional, visual, lifecycle, and performance
  gate; installed-artifact and upgrade tests; then remove Tauri, Ratatui, and
  obsolete build requirements. Release publication and destructive user-data
  cleanup are separate, explicit steps.
