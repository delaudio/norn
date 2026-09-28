# Spec 0001 — v0.1 Findings Schema and Review Output Contract

- Status: Draft
- Date: 2026-06-23
- GitHub issue: #25

## Context

`main` already has several useful review artifacts:

- `AiReviewRunState` for in-flight run metadata and logs
- `AiReviewThread` for persisted assistant/reviewer conversation
- `AiReviewDraftCommentSuggestion` for structured comment candidates extracted
  from an AI review thread
- `DraftComment` for reviewer-controlled staged Bitbucket comments
- `AiReviewFixState` for the local fix/commit/push workflow

These objects are useful, but they are not yet a full review contract.

Today the canonical review content is still mostly free-form assistant markdown
inside a thread. That is good enough for reading and chatting, but weak for:

- dedupe across reruns
- attaching deterministic evidence
- projecting the same review into multiple surfaces
- tracking dismissal / accepted-risk / published / fixed states
- reusing review output in future headless flows

v0.1 needs a normalized `ReviewRun` + `Finding` model layered on top of the
existing artifacts, not a rewrite that discards them.

## Goals

- define a machine-readable review output contract for desktop and future
  headless execution
- separate execution state, conversation state, findings, evidence, and publish
  state
- support stable finding identity across reruns
- make current draft-comment flows a projection of findings rather than the
  canonical review store
- keep the first cut compatible with the current `main` implementation

## Non-goals

- replacing the current chat/thread UX immediately
- defining the full long-term storage backend for every artifact
- solving provider-specific publication details beyond the data the model must
  carry

## Decision

Introduce a normalized `ReviewRun` object that contains:

1. immutable run metadata
2. a PR snapshot reference
3. zero or more structured `Finding` objects
4. zero or more structured `EvidenceArtifact` objects
5. optional links back to the current conversational and publication artifacts

`AiReviewThread`, `DraftComment`, and `AiReviewFixState` remain valid runtime
objects, but they are no longer the canonical shape of a completed review.

## Proposed model

### `ReviewRun`

```ts
interface ReviewRun {
  id: string;
  schemaVersion: "v0.1";
  provider: "bitbucket";
  workspace: string;
  repo: string;
  prId: number;
  sourceBranch: string;
  destinationBranch: string;
  status: "running" | "succeeded" | "failed" | "cancelled";
  turnKind: "initial" | "reply";
  createdAt: string;
  finishedAt: string | null;
  diffFingerprint: string;
  threadId: string | null;
  summaryMarkdown: string | null;
  evidence: EvidenceArtifact[];
  findings: Finding[];
}
```

Notes:

- `id` is unique per run attempt.
- `diffFingerprint` identifies the PR snapshot the findings were generated
  against. v0.1 may implement this as a hash over the review diff payload.
- `summaryMarkdown` preserves the current assistant review output for the
  existing UI while findings adoption is incremental.
- `threadId` links back to the existing persisted chat/thread artifact.

### `Finding`

```ts
interface Finding {
  id: string;
  fingerprint: string;
  title: string;
  severity: "info" | "low" | "medium" | "high" | "critical";
  confidence: "low" | "medium" | "high";
  category:
    | "bug"
    | "security"
    | "performance"
    | "architecture"
    | "typing"
    | "test"
    | "maintainability"
    | "docs"
    | "other";
  status:
    | "new"
    | "dismissed"
    | "accepted"
    | "published"
    | "fixed"
    | "stale";
  summary: string;
  rationale: string | null;
  ruleId: string | null;
  source: "llm" | "analyzer" | "merged";
  anchor: FindingAnchor | null;
  suggestedFix: string | null;
  evidenceIds: string[];
  publication: FindingPublication | null;
}
```

### `FindingAnchor`

```ts
interface FindingAnchor {
  path: string;
  startLine: number;
  endLine: number | null;
  side: "new" | "old";
}
```

Notes:

- v0.1 only needs single-file anchors.
- multi-line ranges are supported in the model even if the current UI usually
  renders a single relevant line.
- findings without a reliable anchor remain valid findings; they simply cannot
  be projected to inline publication automatically.

### `EvidenceArtifact`

```ts
interface EvidenceArtifact {
  id: string;
  kind: "conversation" | "diff" | "analyzer" | "doc" | "manual";
  source:
    | "claude"
    | "codex"
    | "opencode"
    | "bitbucket-diff"
    | "jira"
    | "notion"
    | "tsc"
    | "biome"
    | "tests"
    | "semgrep"
    | "other";
  title: string;
  summary: string | null;
  payload: string | null;
}
```

Notes:

- `payload` is intentionally loose in v0.1. It can contain raw stdout, reduced
  JSON, markdown, or document excerpts.
- analyzer-specific normalization rules belong to the evidence-pipeline spec.

### `FindingPublication`

```ts
interface FindingPublication {
  mode: "inline" | "file" | "general" | "localOnly";
  draftIds: string[];
  remoteCommentIds: number[];
  publishedAt: string | null;
}
```

Notes:

- `draftIds` link to the current local `DraftComment` objects.
- `remoteCommentIds` link to Bitbucket comment ids once publication happens.
- this object tracks publication state without making draft comments the
  canonical finding store.

## Identity and dedupe

`Finding.id` is unique per run.

`Finding.fingerprint` is the stable identity used for dedupe across reruns.
v0.1 should compute it from the most stable available inputs:

- `ruleId` when present
- normalized anchor
- normalized title / summary
- category / severity where useful

The important rule is:

- rerunning a review against the same logical issue should produce a new
  `Finding.id` but the same `fingerprint`

This gives the product a clean distinction between:

- run-local instances
- logical issue identity across time

## Relationship to current `main` artifacts

### `AiReviewThread`

Remains the conversation artifact.

- assistant markdown is preserved for reading and follow-up chat
- the thread is not the canonical store of findings
- one `ReviewRun` may link to one active thread via `threadId`

### `AiReviewRunState`

Remains execution-state metadata for live UI:

- running / failed / cancelled / logs / elapsed time
- when a run completes successfully, a `ReviewRun` should be materialized from
  it plus the structured review output

### `AiReviewDraftCommentSuggestion`

Becomes a projection target, not a final review object:

- structured suggestions may be derived from findings
- in the current transition phase, they may still also be derived from the
  conversational review thread

### `DraftComment`

Remains reviewer-controlled staged publication state:

- drafts are local publication artifacts
- drafts may reference one or more findings
- drafts are not themselves findings

### `AiReviewFixState`

Remains fix-session execution state.

Future work may allow fix runs to consume structured findings directly rather
than only the conversational assistant review text.

## Rendering contract

The desktop app should be able to render all of the following from one
`ReviewRun`:

- a high-level review summary
- a findings list sorted by severity and confidence
- grouped findings by file / path
- inline publish affordances for anchored findings
- local-only findings when anchoring is impossible
- evidence details on demand

The current thread/chat UI can continue to coexist with this model during the
transition.

## Serialization example

```json
{
  "id": "run-2026-06-23T09:30:00Z-1731",
  "schemaVersion": "v0.1",
  "provider": "bitbucket",
  "workspace": "example-workspace",
  "repo": "frontend-app",
  "prId": 1731,
  "sourceBranch": "CB-2066-category-drilldown",
  "destinationBranch": "develop",
  "status": "succeeded",
  "turnKind": "initial",
  "createdAt": "2026-06-23T09:30:00Z",
  "finishedAt": "2026-06-23T09:31:12Z",
  "diffFingerprint": "sha256:example",
  "threadId": "thread-1731-review-1",
  "summaryMarkdown": "## Review\n\nOne high-confidence bug and one test gap.",
  "evidence": [
    {
      "id": "evidence-diff-1",
      "kind": "diff",
      "source": "bitbucket-diff",
      "title": "PR diff snapshot",
      "summary": "Unified diff used for this run",
      "payload": null
    }
  ],
  "findings": [
    {
      "id": "finding-1",
      "fingerprint": "fp-1",
      "title": "Category ERP id path lacks regression coverage",
      "severity": "medium",
      "confidence": "high",
      "category": "test",
      "status": "new",
      "summary": "The new branch path should be covered by a regression test.",
      "rationale": "The PR changes URL generation behavior for category drill-down.",
      "ruleId": null,
      "source": "llm",
      "anchor": {
        "path": "src/app/dashboard/budget/utils/buildOrdersUrlFromBudgetRow.ts",
        "startLine": 17,
        "endLine": null,
        "side": "new"
      },
      "suggestedFix": "Add a regression test covering categoryErpId-driven URL generation.",
      "evidenceIds": ["evidence-diff-1"],
      "publication": {
        "mode": "inline",
        "draftIds": [],
        "remoteCommentIds": [],
        "publishedAt": null
      }
    }
  ]
}
```

## Implementation guidance

Implement this spec incrementally:

1. add the new TypeScript/Rust DTOs
2. preserve current thread + markdown behavior
3. introduce a materialized `ReviewRun` alongside the current thread model
4. map staged draft comments and publish state back to findings
5. move publication and fix flows to consume findings directly where practical

The migration must be additive first. `main` already has a working review flow;
v0.1 should strengthen it, not break it.
