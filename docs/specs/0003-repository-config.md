# Spec 0003 - v0.1 Repository Config and Precedence

- Status: Draft
- Date: 2026-06-23
- GitHub issue: #26

## Context

Norn already has local app configuration and local review customization:

- non-secret app config is stored as `settings.json` in the OS config dir
- secrets live outside repo config in the OS keychain, with environment fallback
  for development
- app config tracks repositories, local clone paths, default diff view, theme,
  review terminal, and Jira base URL
- per-repo review prompt overrides currently live in browser `localStorage`

That model is still useful. Repo-owned configuration should not replace it.
Instead, v0.1 introduces `.norn.yaml` as the versioned review policy surface
that travels with the repository.

## Goals

- make review behavior reproducible across machines and reviewers
- keep machine-local preferences and secrets out of the repository
- define deterministic precedence across built-in defaults, app settings, repo
  config, local overrides, and ad hoc session input
- make invalid repo config fail clearly before a review starts
- allow older clients to handle future config files safely

## Non-goals

- storing Bitbucket, Jira, Notion, or model credentials in repo config
- replacing `settings.json`
- defining organization-wide hosted policy management
- implementing every analyzer or policy engine in this spec

## Config Files

### App config: `settings.json`

Location: OS config dir, managed by Norn.

This remains the local, non-secret app configuration file. It owns:

- tracked Bitbucket repositories
- local clone paths
- UI preferences such as theme and default diff view
- preferred review terminal
- Jira site URL used for local link enrichment
- derived credential-presence flags returned to the UI

### Repo config: `.norn.yaml`

Location: repository root.

This is the shared, committed review configuration. It owns:

- review mode and default behavior
- policy sources
- analyzer enablement and command configuration
- include/exclude paths
- finding severity overrides
- publish defaults
- future CLI/headless defaults that should be shared by the team

### Repo policy folder: `.norn/`

Location: repository root.

This optional folder is a lighter-weight committed review configuration. Norn
uses it only when `.norn.yaml` is absent. It owns:

- `system-prompt.md`, `review-prompt.md`, `review.md`, or `prompt.md` as a
  repository-owned review prompt replacement, checked in priority order
- `packs/*/pack.yaml` policy packs loaded as local policy packs

If both `.norn.yaml` and `.norn/` exist, configuration loading fails with
actionable guidance. Norn never chooses or merges multiple repository roots
implicitly.

The legacy `.lachesi.yaml` and `.lachesi/` names remain fallback-only during
the Norn compatibility window. Any canonical repository root coexisting with a
legacy root is rejected with migration guidance; old and new roots are never
merged implicitly.

### Local repo override: `.norn.local.yaml`

Location: repository root, ignored by git.

This optional file may override non-secret repo behavior for one machine. It is
intended for local analyzer paths, temporary include/exclude tuning, or personal
review-mode preferences. It must never contain credentials.

The legacy `.lachesi.local.yaml` name is read only when `.norn.local.yaml` is
absent. Coexisting local override names are rejected. Both forms must be
regular, untracked files covered by a Git ignore rule.

### Session overrides

Session overrides are explicit inputs provided for one review run, such as a
temporary prompt instruction or "publish as local-only". They are not persisted
unless a later UX explicitly writes them into app config or repo config.

## Precedence

When resolving review behavior, Norn applies configuration in this order:

1. built-in app defaults
2. app-level local `settings.json`
3. repo-owned `.norn.yaml`, or `.norn/` when `.norn.yaml` is absent
4. local non-committed `.norn.local.yaml`
5. ad hoc prompt/session overrides

Later layers override earlier layers only for fields they define.

Arrays are replaced by default, not deep-merged. This keeps precedence
predictable for fields such as `include`, `exclude`, `analyzers`, and
`policy.sources`.

Objects are shallow-merged by key unless a field explicitly documents replacement
semantics.

Secrets are never resolved from repo config. Credential resolution remains:

1. OS keychain
2. development environment variables
3. unavailable

When a signed organization source is explicitly configured, its defaults and
enforced layers wrap the repository and local layers according to
[Spec 0007](./0007-signed-organization-policy.md). Repositories without that
source retain the local order above and perform no organization-policy fetch.

## v0.1 Schema

```yaml
version: 0.1

review:
  profile: frontend-strict
  mode: balanced
  prompt:
    replace: |
      Full team-specific review instructions.
    extend: |
      Extra team-specific review policy appended after the active prompt.
  findings:
    minSeverity: low
    requireAnchors: false

paths:
  include:
    - "src/**"
    - "src-tauri/**"
  exclude:
    - "**/*.snap"
    - "dist/**"
    - "target/**"

policy:
  packs:
    - ./norn-policies/agentic-code
  sources:
    - type: adr
      path: .docflow/adr
    - type: markdown
      path: docs/review-rules.md

analyzers:
  tsc:
    enabled: true
    command: "npm run typecheck"
  biome:
    enabled: true
    command: "npm run lint"
  tests:
    enabled: false
    command: "npm test"
  semgrep:
    enabled: false
    config:
      - "p/owasp-top-ten"

publish:
  defaultMode: inline
  requireManualSubmit: true
  allowGeneralComments: true

profiles:
  frontend-strict:
    mode: strict
    minSeverity: medium
    prompt:
      extend: |
        Pay extra attention to async UI states, generated API contracts, and persisted filters.
    policyPacks:
      - ./norn-policies/react-saas
    analyzers:
      tsc: required
      tests: optional
```

## Field Reference

### `version`

Required. v0.1 clients support `0.1`.

### `review.mode`

Optional. Controls default review depth.

Allowed values:

- `fast`
- `balanced`
- `strict`

Built-in default: `balanced`.

### `review.profile`

Optional named review profile to apply by default for this repository.

The profile id must exist under top-level `profiles`. If omitted and a profile
named `default` exists, Norn applies `default`. A per-run UI or CLI override
can select a different profile for that review.

Missing profile ids produce a warning and Norn falls back to the base review
config.

### `profiles`

Optional map of named review profiles. Profiles are operating modes layered on
top of the base repo config and loaded policy packs.

```yaml
profiles:
  agentic-balanced:
    mode: balanced
    minSeverity: medium
    prompt:
      extend: |
        Treat large agent-authored refactors as high risk unless tests or local evidence prove behavior preservation.
    policyPacks:
      - ./.norn/packs/agentic-code
    analyzers:
      tsc: required
      tests: optional
```

Profile fields:

- `mode`: overrides `review.mode` for that run.
- `minSeverity`: sets `review.findings.minSeverity`.
- `prompt.extend`: prepended before repo-owned `review.prompt.extend`.
- `policyPacks`: additional local packs to load for the selected profile.
- `analyzers`: analyzer requirements. Supported values are `required`,
  `optional`, and `disabled`.

`required` enables an analyzer already defined by the repo or loaded packs. If no
analyzer config exists for that id, configuration validation fails.

### `review.prompt.replace`

Optional. Replaces the built-in review prompt with a full repository-owned
prompt. Use this when the repository needs to own the whole system prompt shape,
including output format instructions.

When Norn loads `.norn/system-prompt.md`, `.norn/review-prompt.md`,
`.norn/review.md`, or `.norn/prompt.md`, it maps that file to
`review.prompt.replace`.

### `review.prompt.extend`

Optional. Appended to the built-in review prompt after repo config is resolved.

This is the committed team-level prompt extension. If `review.prompt.replace`
is present, the extension is appended to that replacement prompt instead of to
the built-in default. It has lower precedence than the current local per-repo
prompt override and lower precedence than session instructions.

### `review.findings.minSeverity`

Optional. Minimum severity that should be shown or published by default.

Allowed values:

- `info`
- `low`
- `medium`
- `high`
- `critical`

Built-in default: `info`.

### `review.findings.requireAnchors`

Optional. If `true`, findings without a reliable changed-line anchor are kept in
the review run but excluded from automatic inline comment staging.

Built-in default: `false`.

### `paths.include` and `paths.exclude`

Optional glob lists evaluated relative to the repo root.

If `include` is omitted, all changed paths are eligible. `exclude` is applied
after `include`.

### `policy.sources`

Optional list of local policy inputs. v0.1 source types:

- `adr`: directory containing Architecture Decision Records
- `markdown`: single markdown policy file
- `yaml`: structured local rule file, reserved for the policy-engine spec
- `pack`: local policy pack directory or manifest file

Missing policy paths should produce a warning, not block review, unless a future
field marks the source as required.

### `policy.packs`

Optional shorthand list of local policy pack directories or manifest files.

```yaml
policy:
  packs:
    - ./norn-policies/react-saas
    - ./.norn/packs/agentic-code
```

Each entry is resolved relative to the repository root unless it is absolute. If
the entry is a directory, Norn looks for `pack.yaml`, `lachesi-pack.yaml`, or
`.lachesi-pack.yaml`.

Pack manifests may provide:

- `review.prompt.extend`
- `review.prompt.replace`
- `policy.rules`
- `policy.pathRules`
- `policy.astRules`
- `policy.suppressions`
- `analyzers`

Pack analyzer entries are defaults: a repo-level analyzer with the same id wins.
Pack prompt extensions are prepended before the repo-owned prompt extension.
Pack prompt replacements apply only when the active prompt has no replacement.
Missing packs produce warnings. Credential-like fields inside pack manifests are
blocking validation errors.

### `analyzers`

Optional map of analyzer definitions.

Each analyzer has:

- `enabled`: boolean
- `command`: local shell command for analyzers that run in the repo
- `config`: analyzer-specific non-secret configuration

Commands run only in local or CI contexts that explicitly enable analyzer
execution. GUI review may still read this config without executing every
analyzer.

### `publish`

Optional publication defaults.

`defaultMode` controls how findings should be projected when possible:

- `inline`
- `file`
- `general`
- `localOnly`

`requireManualSubmit` must default to `true` in v0.1. Norn may stage pending
review comments, but the reviewer keeps final control before publishing.

`allowGeneralComments` controls whether unanchored findings may become general
PR comments.

## Local App Boundary

The following settings stay in `settings.json` or other local storage:

- tracked repository list
- local clone paths
- theme
- default diff view
- review terminal
- Jira site URL
- local prompt override
- collapsed sidebar state and other UI-only preferences

The following settings belong in `.norn.yaml`:

- review depth defaults
- path filters
- analyzer defaults
- policy locations
- finding filters
- publication defaults

The same setting should not exist in both places unless there is a clear local
override use case. When both exist, repo config should control shared review
behavior, while app config should control local ergonomics.

## Prompt Resolution

The effective prompt for a review is built in this order:

1. built-in `DEFAULT_REVIEW_PROMPT`, unless replaced by committed
   `review.prompt.replace` or a `.norn/` prompt file
2. committed `review.prompt.extend` from `.norn.yaml` or policy packs
3. local per-repo prompt override from browser storage
4. explicit session instruction

Local prompt overrides replace the resolved repo prompt for that machine.

## Validation

Repo config is validated before a review run starts.

Blocking errors:

- missing required `version`
- unsupported major version
- invalid enum values
- wrong top-level field types
- analyzer entries with `enabled: true` and an invalid command type
- path filters that are not strings
- repo config attempts to define credential fields

Warnings:

- unknown fields under a supported minor version
- missing optional policy source paths
- disabled analyzers with incomplete command config
- publication defaults that cannot apply to the current provider

When blocking validation fails:

- the review should not start
- the UI should show the file path and validation message
- CLI/headless mode should exit non-zero

When warnings exist:

- the review may start
- warnings should be recorded in review logs and future evidence artifacts

## Forward Compatibility

`version` uses `major.minor`.

Rules:

- clients must reject unsupported major versions
- clients may accept newer minor versions if all required fields are understood
- unknown fields in a supported major version produce warnings, not hard errors
- fields prefixed with `x-` are explicitly experimental and should be ignored by
  older clients with a warning

This lets teams commit a config that newer Norn clients can use while older
clients fail safely when the contract is incompatible.

## Minimal Example

```yaml
version: 0.1

review:
  mode: balanced

paths:
  exclude:
    - "dist/**"
    - "target/**"
    - "**/*.snap"

publish:
  requireManualSubmit: true
```

This example coexists with local `settings.json`: the app still gets tracked
repos, local paths, theme, terminal preference, and Jira URL from local settings.
The repo only contributes shared review behavior.

## Advanced Example

```yaml
version: 0.1

review:
  mode: strict
  prompt:
    extend: |
      Treat ADR violations as architecture findings.
      Flag any new direct backend calls from React components.
  findings:
    minSeverity: low
    requireAnchors: false

paths:
  include:
    - "src/**"
    - "src-tauri/**"
    - ".docflow/adr/**"
  exclude:
    - "**/*.generated.*"
    - "**/*.snap"
    - "target/**"

policy:
  sources:
    - type: adr
      path: .docflow/adr
    - type: markdown
      path: docs/review-policy.md

analyzers:
  tsc:
    enabled: true
    command: "npm run typecheck"
  biome:
    enabled: true
    command: "npm run lint"
  tests:
    enabled: true
    command: "npm test -- --runInBand"
  semgrep:
    enabled: false
    config:
      - "p/typescript"

publish:
  defaultMode: inline
  requireManualSubmit: true
  allowGeneralComments: true
```

A developer may add `.norn.local.yaml` to temporarily disable a slow analyzer:

```yaml
version: 0.1

analyzers:
  tests:
    enabled: false
```

That local override is not committed and does not change team policy.

## Implementation Guidance

The first implementation slice should:

1. load `.norn.yaml` from the selected local repo root when available
2. validate it into a typed `RepoReviewConfig`
3. merge it with app config and local/session overrides into an
   `EffectiveReviewConfig`
4. pass the effective config into review prompt construction and future analyzer
   orchestration
5. record config warnings in AI review logs

The migration is additive. Existing users without `.norn.yaml` should see the
same behavior they see today.
