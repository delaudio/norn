# Spec 0005 - v0.1 Local Evidence Pipeline

- Status: Draft
- Date: 2026-06-23
- GitHub issue: #28

## Context

Norn already runs review and fix workflows against a local clone. The app has
repo path resolution, branch state, PR diff state, Jira context, review logs,
and a normalized `ReviewRun.evidence` field.

What is still missing is a first-class evidence pipeline: deterministic analyzer
execution, normalized outputs, clear timeout behavior, and merge rules between
tool signals and LLM findings.

v0.1 should prefer explicit repo commands over auto-detection. Reproducibility
matters more than clever guesses.

## Goals

- define the initial analyzer adapters for typecheck, lint, tests, and scanners
- define a local invocation contract that can run in desktop and future CLI mode
- normalize analyzer output into evidence artifacts independent of one tool
- attach evidence to review runs and later to findings
- define timeout, failure, and cancellation behavior
- define how analyzer evidence and LLM findings are merged and prioritized

## Non-goals

- implementing every analyzer adapter immediately
- remote SaaS execution
- treating raw analyzer output as a complete review finding without
  interpretation
- replacing CI

## v0.1 Analyzer Set

The initial first-party adapter names are:

- `tsc`: TypeScript typecheck command
- `biome`: Biome lint/format diagnostics
- `tests`: repo-defined test command
- `semgrep`: optional security/static scanner

Adapters are configured through effective repo config:

```yaml
analyzers:
  tsc:
    enabled: true
    command: "npm run typecheck"
    timeoutSeconds: 120
  biome:
    enabled: true
    command: "npm run lint"
    timeoutSeconds: 120
  tests:
    enabled: false
    command: "npm test"
    timeoutSeconds: 300
  semgrep:
    enabled: false
    command: "semgrep --json --config p/typescript"
    timeoutSeconds: 300
```

When a repo omits an analyzer, Norn should not auto-run it in v0.1. A future
version may offer suggestions, but execution should stay explicit.

## Invocation Contract

```ts
interface AnalyzerInvocation {
  id: "tsc" | "biome" | "tests" | "semgrep" | string;
  command: string;
  cwd: string;
  env: Record<string, string>;
  timeoutMs: number;
  changedFiles: string[];
  include: string[];
  exclude: string[];
}
```

Execution rules:

- run from the local repo root
- inherit only a controlled environment from the app process
- never inject secrets unless an adapter explicitly requires a credential class
  and the user approved it
- capture stdout, stderr, exit code, duration, and timeout/cancellation state
- stream progress into review logs
- treat command strings as repo config, not user chat input

The desktop app may run analyzers before the LLM review or in parallel with
context collection. CLI/headless mode should use the same invocation contract.

## Result Contract

```ts
interface AnalyzerRunResult {
  analyzerId: string;
  status: "passed" | "failed" | "timedOut" | "cancelled" | "skipped" | "error";
  command: string;
  cwd: string;
  exitCode: number | null;
  startedAt: string;
  finishedAt: string | null;
  durationMs: number | null;
  stdout: string;
  stderr: string;
  diagnostics: AnalyzerDiagnostic[];
}

interface AnalyzerDiagnostic {
  id: string;
  ruleId: string | null;
  severity: "info" | "low" | "medium" | "high" | "critical";
  message: string;
  path: string | null;
  line: number | null;
  endLine: number | null;
  column: number | null;
  source: "tsc" | "biome" | "tests" | "semgrep" | "other";
  raw: unknown;
}
```

Adapters should normalize diagnostics when possible and still preserve bounded
raw output for debugging.

## Evidence Artifacts

Analyzer results are stored as `ReviewEvidenceArtifact` entries:

```json
{
  "id": "run-1-evidence-analyzer-tsc",
  "kind": "analyzer",
  "source": "tsc",
  "title": "TypeScript typecheck",
  "summary": "Failed with 2 diagnostics in 18.4s",
  "payload": "{\"status\":\"failed\",\"exitCode\":2,\"diagnostics\":[...]}"
}
```

Guidelines:

- `payload` should be compact JSON, not unbounded terminal output
- stdout/stderr should be truncated with clear markers when large
- diagnostics should include paths relative to the repo root
- analyzer evidence should be attached to `ReviewRun.evidence` even when the LLM
  produces no matching finding

## Adapter Notes

### `tsc`

v0.1 may parse plain compiler output or structured output when the project
provides it.

Default severity:

- diagnostics on changed files: `high`
- diagnostics outside changed files: `medium`

### `biome`

Prefer JSON output when command config provides it. Otherwise, store summary and
raw bounded output.

Default severity:

- correctness/security lint categories: `medium`
- style-only diagnostics: `low`

### `tests`

The `tests` adapter is intentionally repo-defined. v0.1 should not infer Jest,
Vitest, Cargo, or Playwright automatically.

Default severity:

- failing tests touching changed behavior: `high`
- unrelated suite failure: `medium`
- command infrastructure failure: `low` to `medium`, depending on evidence

### `semgrep`

Semgrep is optional. When enabled, JSON output should be normalized into
diagnostics with Semgrep rule ids preserved.

Default severity should follow Semgrep severity when present.

## Timeout, Error, and Cancellation

Each analyzer has a timeout. If omitted:

- `tsc`: 120 seconds
- `biome`: 120 seconds
- `tests`: 300 seconds
- `semgrep`: 300 seconds

Behavior:

- `passed`: exit code 0
- `failed`: non-zero exit code with completed process
- `timedOut`: process exceeded timeout and was terminated
- `cancelled`: user cancelled the review run
- `skipped`: disabled, missing command, or no matching changed files
- `error`: Norn could not start or manage the process

Timeouts and errors should not automatically fail the whole review. They become
evidence and warnings unless the repo config later marks an analyzer as required.

## Merge With LLM Review

Analyzer output is evidence. It is not automatically the canonical finding list.

Merge rules:

1. run deterministic analyzers and collect evidence
2. include analyzer summaries and changed-file diagnostics in the LLM review
   prompt
3. ask the LLM to promote only relevant diagnostics into findings
4. preserve analyzer `ruleId` and evidence id in promoted findings
5. keep unpromoted analyzer evidence available in `ReviewRun.evidence`

When a diagnostic already has a precise changed-line location, the resulting
finding should reuse that anchor unless the diff no longer contains the line.

When a diagnostic is outside the changed diff, the resulting finding should be
local-only unless the model can explain why it is caused by the PR.

## Prioritization

Findings backed by deterministic evidence should outrank equally severe
LLM-only findings.

Suggested ordering:

1. critical/high analyzer-backed findings on changed lines
2. critical/high policy findings
3. medium analyzer-backed findings on changed lines
4. high-confidence LLM findings
5. unanchored or low-confidence findings

This affects display and prompt emphasis; it does not delete lower-priority
findings.

## Example

Input analyzer result:

```json
{
  "analyzerId": "tsc",
  "status": "failed",
  "exitCode": 2,
  "diagnostics": [
    {
      "id": "tsc-1",
      "ruleId": "TS2322",
      "severity": "high",
      "message": "Type 'string | null' is not assignable to type 'string'.",
      "path": "src/lib/reviewFindingPublication.ts",
      "line": 42,
      "endLine": null,
      "column": 11,
      "source": "tsc",
      "raw": "src/lib/reviewFindingPublication.ts(42,11): error TS2322: ..."
    }
  ]
}
```

Promoted finding:

```json
{
  "id": "finding-1",
  "fingerprint": "tsc:TS2322:src/lib/reviewFindingPublication.ts:42",
  "title": "Nullable publication state is assigned to a required string",
  "severity": "high",
  "confidence": "high",
  "category": "typing",
  "status": "new",
  "summary": "The typecheck reports TS2322 on a changed line.",
  "rationale": "This is deterministic compiler evidence, not an inferred issue.",
  "ruleId": "TS2322",
  "source": "analyzer",
  "anchor": {
    "path": "src/lib/reviewFindingPublication.ts",
    "startLine": 42,
    "endLine": null,
    "side": "new"
  },
  "suggestedFix": "Handle the null case or narrow before assignment.",
  "evidenceIds": ["run-1-evidence-analyzer-tsc"],
  "publication": null
}
```

## Implementation Guidance

The first implementation slice should:

1. extend effective repo config with analyzer command definitions
2. add a Rust-side process runner that captures bounded stdout/stderr
3. normalize each analyzer result into `AnalyzerRunResult`
4. materialize analyzer results as `ReviewEvidenceArtifact` entries
5. include analyzer summaries in the review prompt
6. teach finding materialization to preserve analyzer evidence ids and rule ids

Existing behavior should remain unchanged when no analyzers are configured.
