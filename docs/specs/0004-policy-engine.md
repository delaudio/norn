# Spec 0004 - v0.1 Policy Engine

- Status: Draft
- Date: 2026-06-23
- GitHub issue: #27

## Context

Norn should review pull requests against project-specific rules, not only
generic code-review heuristics.

`main` already has the early building blocks:

- a default review prompt plus per-repo local prompt overrides
- Jira and linked resource context in review payloads
- normalized `ReviewRun` and `Finding` objects with `ruleId`, `severity`,
  `rationale`, `anchor`, `evidenceIds`, and publication state
- `.norn.yaml` as the repo-owned config surface for policy sources

There is no first-class policy engine yet. v0.1 should therefore start with an
incremental contract that is useful immediately and does not promise a broad
cross-language static-analysis runtime.

## Goals

- make ADR-backed and path-scoped rules first-class review inputs
- attach policy violations to normalized findings through `ruleId`,
  `severity`, `rationale`, remediation, and evidence
- support prompt-assisted evaluation while preserving structured rule metadata
- define limited AST-oriented rule declarations as an extension point
- document precedence, conflicts, suppression, and anchoring behavior

## Non-goals

- implementing a complete AST engine for every language
- replacing existing linters, typecheckers, test runners, Semgrep, or CodeQL
- distributing organization-wide policy outside the repository
- guaranteeing every policy violation can be anchored inline

## Policy Sources

v0.1 supports four policy source types.

### ADR Sources

ADR sources point to markdown files or directories listed in `.norn.yaml`:

```yaml
policy:
  sources:
    - type: adr
      path: .docflow/adr
```

ADR files are parsed for:

- identifier from filename, for example `adr:0001`
- title
- status
- decision text
- consequences

Only accepted ADRs are enforced by default. Proposed or superseded ADRs may be
included as review context but should not produce blocking findings unless a
rule explicitly opts in.

Concrete v0.1 examples from this repo:

- `adr:0001` - Bitbucket HTTP must live in Rust commands, not webview fetches
- `adr:0002` - secrets must live in the OS keychain, not local config files
- `adr:0003` - diff anchoring should stay behind `src/lib/diff.ts` and
  `react-diff-view` widget integration

### Explicit Rule Blocks

Explicit rules can be declared in `.norn.yaml`:

```yaml
policy:
  rules:
    - id: arch.bitbucket-http-in-rust
      source: adr:0001
      severity: high
      appliesTo:
        include:
          - "src/**"
          - "src-tauri/**"
      instruction: >
        New Bitbucket REST calls must be implemented in Rust Tauri commands.
        The React webview must not call api.bitbucket.org directly.
      remediation: >
        Move the HTTP call behind a Rust command and expose a typed DTO over IPC.
```

Explicit rules are the preferred bridge from prose ADRs to enforceable review
behavior.

### Path-Scoped Instructions

Path-scoped rules apply only to changed files matching glob filters:

```yaml
policy:
  pathRules:
    - id: ui.diff-anchor-boundary
      severity: medium
      paths:
        include:
          - "src/components/pr-detail/**"
          - "src/lib/diff.ts"
      instruction: >
        Keep Bitbucket inline anchor mapping centralized in src/lib/diff.ts.
        Components should consume helper functions rather than reconstructing
        change keys directly.
```

These are first-class in v0.1 because they are cheap to evaluate: Norn can
match them against the changed path list before invoking the model.

### Limited AST-Oriented Rules

AST rules are allowed only as declarations with narrow language and matcher
metadata. v0.1 does not promise a general AST runtime.

```yaml
policy:
  astRules:
    - id: ui.no-webview-bitbucket-fetch
      language: typescript
      severity: high
      selector:
        kind: callExpression
        callee: fetch
        argumentContains: "api.bitbucket.org"
      appliesTo:
        include:
          - "src/**"
      instruction: >
        The webview must not fetch Bitbucket API endpoints directly.
```

v0.1 clients may evaluate these rules in one of three ways:

- declaratively, when a supported local matcher exists
- through an analyzer such as Semgrep
- prompt-assisted, by including the structured rule in model context

Unsupported AST rules should produce warnings, not silent success.

## Rule Model

```ts
interface PolicyRule {
  id: string;
  source: string | null;
  severity: "info" | "low" | "medium" | "high" | "critical";
  confidence?: "low" | "medium" | "high";
  appliesTo?: {
    include?: string[];
    exclude?: string[];
  };
  instruction: string;
  rationale?: string;
  remediation?: string;
  enforcement?: "prompt" | "analyzer" | "ast" | "manual";
}
```

`id` must be stable. It becomes `Finding.ruleId` when the rule produces a
finding.

`source` links a rule back to an ADR, markdown policy file, or analyzer rule.

`severity` is the default severity for violations. The model may lower severity
only when the evidence is weak; it should not raise severity above the rule
without explaining why.

## Evaluation Order

Policy inputs are resolved in this order:

1. built-in Norn review heuristics
2. ADR-derived policy context
3. explicit `.norn.yaml` rules
4. path-scoped rules matching changed files
5. analyzer or AST rule declarations
6. local `.norn.local.yaml` policy overrides
7. session-level reviewer instructions

Later layers may narrow, suppress, or strengthen earlier layers, but they should
not erase evidence. When a rule is overridden, the effective rule set should
record the winning source for review logs.

## Conflict Handling

Rules conflict when they share a target and give incompatible instructions.

Conflict resolution:

1. exact `id` override wins
2. local `.norn.local.yaml` may disable a rule for one machine
3. session overrides may suppress a rule for one run
4. otherwise the stricter rule wins and Norn emits a policy warning

Severity conflict:

- if the same `id` appears twice, the later layer's severity wins
- if different rules apply to the same code, findings keep their original
  severities

## Suppression

Suppressions are explicit and auditable.

Repo-level suppression:

```yaml
policy:
  suppressions:
    - ruleId: ui.diff-anchor-boundary
      paths:
        include:
          - "src/legacy/**"
      reason: "Legacy code pending diff layer migration."
      expiresAt: "2026-09-30"
```

Session-level suppression may be passed at review time for one run. It should be
recorded in review logs and should not modify committed config.

Inline source-code suppressions are out of scope for v0.1 unless an analyzer
already supports them.

Expired suppressions should warn and stop suppressing the rule.

## Finding Projection

When a policy rule produces a violation, Norn creates or asks the model to
create a normalized finding:

```ts
{
  ruleId: "arch.bitbucket-http-in-rust",
  severity: "high",
  category: "architecture",
  source: "merged",
  rationale: "Repository architecture policy requires provider API calls to stay behind the native boundary.",
  suggestedFix: "Move the API call behind a Tauri command and typed DTO.",
  evidenceIds: ["policy:adr:0001", "diff:file:src/foo.ts"],
  anchor: {
    path: "src/foo.ts",
    startLine: 42,
    endLine: null,
    side: "new"
  }
}
```

Anchoring rules:

- if the violation maps to a changed line, create an inline finding
- if it maps to a changed file but not a specific line, create a file-level or
  local-only finding
- if it comes from broad architecture context, create a local-only finding unless
  the model can cite a precise changed line

Publication follows the Bitbucket publication spec. Policy findings may be
staged as draft comments, but v0.1 keeps manual submit as the default.

## Prompt-Assisted Baseline

Prompt-assisted evaluation is allowed in v0.1.

The prompt must include structured policy context:

- rule id
- source
- severity
- matching paths
- instruction
- rationale
- remediation

The model may interpret whether changed code violates the rule, but the output
must preserve the rule id in the resulting finding.

This gives Norn immediate value from ADR/path rules while leaving room for
stronger analyzers later.

## Examples

### ADR-Backed Rule

```yaml
policy:
  rules:
    - id: arch.credentials-keychain
      source: adr:0002
      severity: critical
      appliesTo:
        include:
          - "src-tauri/**"
          - "src/**"
      instruction: >
        Bitbucket, Jira, Notion, and model credentials must not be written to
        settings.json, .norn.yaml, localStorage, or frontend state.
      remediation: >
        Store secrets through the credentials module and OS keychain. Keep only
        non-secret config in settings.json or repo config.
```

Violation result:

- `ruleId`: `arch.credentials-keychain`
- `category`: `security`
- `severity`: `critical`
- anchor: changed line if a token/config write is visible, otherwise local-only

### Path-Scoped Rule

```yaml
policy:
  pathRules:
    - id: diff.inline-anchor-boundary
      severity: medium
      paths:
        include:
          - "src/components/pr-detail/**"
          - "src/components/comments/**"
      instruction: >
        Inline comment UI must use src/lib/diff.ts helpers for Bitbucket
        to/from anchor mapping. Do not duplicate change-key construction in
        components.
      remediation: >
        Move anchor mapping logic to src/lib/diff.ts and consume the helper from
        UI components.
```

Violation result:

- `ruleId`: `diff.inline-anchor-boundary`
- `category`: `architecture`
- `severity`: `medium`
- anchor: the duplicated mapping code when visible in the diff

## Implementation Guidance

The first implementation slice should:

1. load policy packs and policy sources from effective repo config
2. extract ADR metadata and explicit rule blocks into `PolicyRule` objects
3. match path rules against changed files before building the review prompt
4. add policy artifacts to `ReviewRun.evidence`
5. include matched rules in the AI review payload as structured context
6. require generated policy findings to preserve `ruleId`
7. emit validation warnings for unsupported AST rules

The policy engine should be additive. A repo without `.norn.yaml` should keep
the current review behavior.
