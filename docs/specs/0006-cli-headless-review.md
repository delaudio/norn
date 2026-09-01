# Spec 0006 - v0.1 CLI and Headless Review Mode

- Status: Draft
- Date: 2026-06-23
- GitHub issue: #30

## Context

Norn is currently Tauri-first:

- the executable entrypoint is the desktop app
- Bitbucket access, review execution, review persistence, fix sessions, and
  publication primitives are exposed as Tauri commands
- local repo resolution already exists in Rust
- normalized `ReviewRun`, `Finding`, and `EvidenceArtifact` contracts already
  exist
- repo-owned config, policy sources, and local evidence pipeline specs now
  define shared behavior that should not be desktop-only

CLI mode is therefore not just a thin wrapper around UI commands. v0.1 requires
a reusable review core that both the Tauri adapter and the future CLI adapter can
call.

## Goals

- define the split between reusable review core, Tauri desktop adapter, and CLI
  adapter
- define a first CLI command surface focused on review execution
- reuse normalized review output rather than inventing CLI-only result shapes
- support local interactive and CI usage
- define markdown, JSON, and exit-code behavior
- document authentication and repository assumptions for Bitbucket-linked flows

## Non-goals

- full desktop parity in the first CLI cut
- interactive chat threads
- staging or publishing Bitbucket draft comments from CLI
- fix/commit/push automation from CLI
- hosted orchestration or enterprise reporting

## Architecture

### Reusable Review Core

The review core should be a Rust module or crate with no Tauri dependency.

It owns:

- loading effective config
- resolving local repo context
- collecting Bitbucket PR metadata and diff payloads through provider clients
- collecting Jira/Notion/resource context when configured
- running local evidence analyzers
- building the AI review prompt
- invoking the model provider
- materializing `ReviewRun`, `Finding`, and `EvidenceArtifact`
- persisting review store updates through an injected storage interface

It should expose an API shaped like:

```rust
pub struct ReviewRequest {
    pub workspace: String,
    pub repo: String,
    pub pr_id: u32,
    pub repo_path: Option<PathBuf>,
    pub output_format: ReviewOutputFormat,
    pub profile: Option<String>,
    pub evidence_only: bool,
    pub fail_on_findings: bool,
    pub session_instruction: Option<String>,
}

pub struct ReviewExecutionResult {
    pub run: ReviewRun,
    pub markdown: String,
    pub warnings: Vec<String>,
    pub analyzer_failures: Vec<String>,
}
```

The exact Rust type names can change, but the boundary matters: desktop and CLI
should call the same review orchestration code.

### Tauri Desktop Adapter

The Tauri adapter owns:

- command registration in `src-tauri/src/lib.rs`
- UI-oriented run state and live logs
- cancellation buttons and progress polling
- chat threads and replies
- draft-comment staging and publication
- fix sessions, commit, and push workflows

Desktop can keep richer state than CLI, but review results should still flow
through the shared `ReviewRun` contract.

### CLI Adapter

The CLI adapter owns:

- argument parsing
- terminal output
- process exit codes
- CI-friendly non-interactive behavior
- reading stdin only where explicitly supported

The CLI should not import or initialize a Tauri runtime.

## Command Surface

### `norn review`

Primary v0.1 command:

```sh
norn review --workspace example-workspace --repo frontend-app --pr 1731
```

Options implemented in the first CLI cut:

```sh
norn review \
  [--repo-path <path>] \
  [--scope working-tree|branch|pr] \
  [--base <ref>] \
  [--workspace <workspace>] \
  [--repo <repo>] \
  [--pr <id>] \
  [--provider github|bitbucket] \
  [--format markdown|json] \
  [--profile <name>] \
  [--ai-provider codex|claude] \
  [--model <name>] \
  [--effort <level>] \
  [--output <path>] \
  [--fail-on-findings] \
  [--min-severity info|low|medium|high|critical] \
  [--run-analyzers] \
  [--allow-provider-diff]
```

Defaults:

- `--format markdown`
- repo path comes from app config, explicit `--repo-path`, or discovery
- `--workspace` and `--repo` must be provided together; explicit identity
  values must match the selected local checkout
- `--pr` is valid only for PR scope and `--base` only for branch scope
- branch scope reviews committed changes from merge base through `HEAD`; when
  local changes are present, Norn warns that working-tree scope must be run
  separately
- `.norn.yaml` is loaded from repo root when present
- `--profile` overrides `review.profile`; if omitted, `review.profile` or a
  `default` profile is used when configured
- local `.norn.local.yaml` is loaded when present
- manual publication is not attempted
- findings do not fail the process unless `--fail-on-findings` is set
- local analyzers are skipped by default because agent-driven review follows
  the repository validation gate; `--run-analyzers` opts in for standalone use
- working-tree review includes ordinary untracked text files but skips
  potentially sensitive paths such as environment files, credential files, and
  private-key material with a warning
- `--run-analyzers` requires a non-empty review target and fails target
  resolution instead of reporting that analyzers ran when there are no changes
- headless review uses temporary local storage by default and removes it after
  completion; setting `NORN_DATA_DIR` explicitly opts into a chosen
  persistent location
- a non-empty diff is sent to the configured AI provider only when local setup
  allows headless diff sharing or `--allow-provider-diff` authorizes that run
- the known Codex sandbox fails before provider invocation with
  `review.sandboxRestricted`; this host approval is separate from diff-sharing
  consent. Claude permission failures map to the same code after its managed
  skill requests host permission up front
- Codex and Claude provider waits default to five minutes and can be bounded
  from 30 to 1,800 seconds with `NORN_AI_PROVIDER_TIMEOUT_SECONDS`; timeout
  failures use `review.providerTimeout`

Planned options such as custom config paths, JSONL streaming, evidence-only
execution, per-run session instructions, and source-specific enrichment
switches are not part of the first CLI cut. They must not be advertised by
`norn review --help` until implemented.

### `norn config validate`

Validates effective config without running review:

```sh
norn config validate --repo-path .
```

Exit behavior follows the config exit-code model below.

### `norn config migrate`

Previews or executes the bounded repository-config namespace migration:

```sh
norn config migrate --repo-path . --dry-run --format json
```

The dry run lists source/target renames and YAML policy-path rewrites without
changing the repository. Omitting `--dry-run` performs the migration and never
overwrites an existing canonical target.

### `norn metrics`

Aggregates persisted structured review runs and append-only finding feedback:

```sh
norn metrics --tenant local --workspace example-workspace --repo frontend-app
```

The command supports human and `norn.review-effectiveness.v1` JSON output,
tenant/provider/repository filters, and an inclusive-start/exclusive-end
completion-time window. Metric definitions and missing-feedback behavior are
specified in [Review effectiveness metrics](0008-review-effectiveness-metrics.md).

### `norn evidence`

Planned follow-up command:

```sh
norn evidence --workspace example-workspace --repo frontend-app --pr 1731 --format json
```

This will run configured analyzers and emit evidence without invoking the
model. Neither this command nor `norn review --evidence-only` is implemented
in the first CLI cut.

## Output Formats

### Markdown

Human-readable output for local terminal usage.

It should include:

- review title and PR identifier
- summary
- findings grouped by severity
- file/line anchors when present
- evidence and analyzer warnings
- selected review profile, when one was used
- footer with run id and schema version

### JSON

Machine-readable output for CI and downstream tools.

The top-level JSON object should be:

```json
{
  "schemaVersion": "norn.headless-review.v1",
  "status": "succeeded",
  "exitCode": 1,
  "warnings": [],
  "minimumSeverity": "high",
  "analyzersRan": false,
  "target": {
    "scope": "branch",
    "repoPath": "/workspace/frontend-app",
    "workspace": null,
    "repo": "frontend-app",
    "prId": null,
    "source": "feature/example",
    "destination": "main"
  },
  "reviewRun": {
    "id": "run-1",
    "schemaVersion": "v0.1",
    "provider": "bitbucket",
    "reviewProfile": "agentic-balanced",
    "findings": [],
    "evidence": []
  }
}
```

The top-level headless envelope uses `norn.headless-review.v1`. Readers retain
support for the legacy Lachesi schema identifier during the compatibility window.
`reviewRun` independently uses the same `v0.1` contract documented in the
findings spec. Setup and runtime failures use the same top-level headless
schema with `status: "failed"`, `exitCode`, and `error`.

Headless output retains evidence identifiers, kinds, sources, titles, and
summaries, but omits raw evidence payloads. Analyzer stdout and stderr can
contain credentials or other sensitive process output and are never serialized
to terminal or CI output. Summaries and findings are derived from the reviewed
diff and model response, so consumers must protect them like source code rather
than treating the complete review artifact as secret-free.

### JSONL

Streaming format for CI logs and long-running reviews.

Example events:

```jsonl
{"type":"started","workspace":"example-workspace","repo":"frontend-app","prId":1731}
{"type":"log","message":"Running analyzer: tsc"}
{"type":"warning","message":"Semgrep skipped: command not found"}
{"type":"result","reviewRun":{...}}
```

JSONL is useful for future integrations but can be deferred if JSON and markdown
ship first.

## Exit Codes

```text
0  review completed and no failing condition was requested
1  review completed, findings at or above threshold exist, and --fail-on-findings was set
2  config validation failed
3  authentication or authorization failed
4  repository or PR could not be resolved
5  analyzer required by config failed, timed out, or could not start
6  model provider failed
7  runtime/internal error
130 cancelled by user
```

Analyzer failures are relevant only when `--run-analyzers` is set. They are
non-fatal by default and use exit code `5` only when the effective config marks
the analyzer as required.

Findings use exit code `1` only when `--fail-on-findings` is set. The threshold
is controlled by `--min-severity` or repo config.

## Local Interactive Usage

Local usage optimizes for readable terminal output:

```sh
norn review --workspace example-workspace --repo backend-api --pr 1020
```

Expected behavior:

- use app config and keychain credentials when available
- resolve local repo path from explicit flag, settings, or discovery
- load `.norn.yaml` and `.norn.local.yaml`
- print progress to stderr
- print markdown result to stdout unless `--output` is set
- keep review state ephemeral unless `NORN_DATA_DIR` is explicitly configured

## CI Usage

CI usage should be deterministic and non-interactive:

```sh
norn review \
  --workspace "$BITBUCKET_WORKSPACE" \
  --repo "$BITBUCKET_REPO_SLUG" \
  --pr "$BITBUCKET_PR_ID" \
  --repo-path "$PWD" \
  --format json \
  --fail-on-findings \
  --min-severity high
```

CI assumptions:

- repo path is explicit
- credentials come from environment or an injected credential provider
- no desktop settings dialog exists
- no Tauri runtime exists
- output should be stable enough for artifacts and annotations

CI should not attempt interactive publication in v0.1.

## Auth and Provider Assumptions

Bitbucket-linked review requires:

- workspace
- repo slug
- PR id
- credentials from keychain, environment, or injected CLI credential provider

The core should preserve the existing security boundary:

- secrets are not read from `.norn.yaml`
- credential sources and raw analyzer evidence are not included in JSON output
- model-derived summaries and findings can reflect reviewed source content and
  must be handled with the same confidentiality as the diff
- webview-only assumptions must not leak into CLI

Jira and Notion enrichment are optional. Missing enrichment credentials should
warn, not fail review, unless future config marks them required.

## Desktop Behaviors Excluded From v0.1 CLI

The first CLI cut intentionally excludes:

- chat replies to an existing review thread
- manual pending-review draft staging
- publishing Bitbucket comments
- AI fix sessions
- commit and push workflow
- conflict resolution workflow
- GUI progress panes and stored UI layout

These can be added later after the shared review core is stable.

## Implementation Plan

1. Extract review orchestration from Tauri command handlers into a core module.
2. Keep Tauri commands as thin adapters that call the core and update UI stores.
3. Define a storage trait for review-store persistence.
4. Define provider traits for Bitbucket, Jira, Notion, and model execution.
5. Add a CLI binary entrypoint that calls the core without Tauri.
6. Implement `norn config validate`.
7. Implement `norn review --format markdown|json`.
8. Add `--evidence-only` once analyzer execution is available.

The extraction should be incremental. Desktop behavior must keep working while
core logic moves behind adapter boundaries.
