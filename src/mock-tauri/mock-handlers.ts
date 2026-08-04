import { readMigratedStorageValue } from "@/lib/storageMigration";
import type {
  AiReviewDraftCommentSuggestion,
  AiReviewFixState,
  AiReviewJob,
  AiReviewMessage,
  AiReviewRunState,
  AiReviewStore,
  BranchStatus,
  BranchSyncResult,
  FindingPublicationRequest,
  FindingReconciliationAction,
  FindingReconciliationRequest,
  FindingReconciliationSummary,
  PrComment,
  PrFilePreview,
  PrListFilter,
  PublishedCommentIdentity,
  PullRequestPage,
  RepositoryBlameLine,
  RepositoryFileContent,
  RepositoryFileDiff,
  RepositoryFileEntry,
  ReviewEffectivenessFilter,
  ReviewFinding,
  ReviewFindingPublication,
  ReviewFindingPublicationEvent,
  ReviewRun,
} from "@/types";
import {
  mockClosedPrMetrics,
  mockComments,
  mockConfig,
  mockDiffstat,
  mockEmptyReviewEffectivenessSummary,
  mockPullRequestDetail,
  mockPullRequests,
  mockRawDiff,
  mockReviewEffectivenessLast7DaysReport,
  mockReviewEffectivenessReport,
} from "./fixtures";

type Handler = (args?: Record<string, unknown>) => unknown;

const SEVEN_DAYS_MS = 7 * 24 * 60 * 60 * 1000;

interface SavedReview {
  content: string;
  generatedAt: string;
}

let mockCommentId = 100000;
let mockPullRequestDetailState = mockPullRequestDetail;
const mockPublishedFindings = new Map<string, PublishedCommentIdentity>();
const mockReviewStoreMemory = new Map<string, AiReviewStore>();
const mockFixStates = new Map<string, AiReviewFixState>();
const mockReviewRunStates = new Map<string, AiReviewRunState>();
const mockReviewRunTimers = new Map<string, number[]>();
const mockBranchStatuses = new Map<string, BranchStatus>();
const mockSvgPreview = `<svg xmlns="http://www.w3.org/2000/svg" width="360" height="180" viewBox="0 0 360 180">
  <rect width="360" height="180" rx="16" fill="#111827"/>
  <circle cx="90" cy="90" r="42" fill="#2dd4bf"/>
  <rect x="160" y="58" width="140" height="18" rx="9" fill="#f8fafc"/>
  <rect x="160" y="88" width="108" height="14" rx="7" fill="#94a3b8"/>
  <rect x="160" y="114" width="78" height="14" rx="7" fill="#64748b"/>
</svg>`;
let mockReviewJobs: AiReviewJob[] = [
  {
    id: "job-1",
    workspace: "example-workspace",
    repo: "backend-api",
    prId: 1020,
    prTitle: "APP-1777 - feat(user-notes): add mock query endpoints",
    sourceBranch: "feature/user-notes-mock-endpoints",
    destinationBranch: "develop",
    status: "succeeded",
    trigger: "menuBar",
    threadId: "thread-1782153283369168000",
    error: null,
    createdAt: new Date(Date.now() - 1000 * 60 * 45).toISOString(),
    startedAt: new Date(Date.now() - 1000 * 60 * 44).toISOString(),
    finishedAt: new Date(Date.now() - 1000 * 60 * 42).toISOString(),
  },
  {
    id: "job-2",
    workspace: "example-workspace",
    repo: "frontend-app",
    prId: 1731,
    prTitle: "APP-2066 - fix saved-view filter returning empty results",
    sourceBranch: "bugfix/APP-2066-saved-view-filter",
    destinationBranch: "develop",
    status: "running",
    trigger: "menuBar",
    threadId: "thread-running",
    error: null,
    createdAt: new Date(Date.now() - 1000 * 90).toISOString(),
    startedAt: new Date(Date.now() - 1000 * 80).toISOString(),
    finishedAt: null,
  },
];

const mockRepositoryFiles: RepositoryFileEntry[] = [
  { path: "src/App.tsx", status: "modified" },
  { path: "src/components/records/RecordTable.tsx", status: "unchanged" },
  { path: "src/components/records/RecordDetails.tsx", status: "unchanged" },
  { path: "src/lib/api.ts", status: "unchanged" },
  { path: "src/lib/format.ts", status: "added" },
  { path: "src/lib/localDraft.ts", status: "untracked" },
  { path: "src/lib/legacy.ts", status: "deleted" },
  { path: "package.json", status: "unchanged" },
  { path: "README.md", status: "unchanged" },
];

const mockRepositoryFileContents: Record<string, string> = {
  "src/App.tsx": `import { RecordTable } from "./components/records/RecordTable";

export function App() {
  return <RecordTable />;
}
`,
  "src/components/records/RecordTable.tsx": `export function RecordTable() {
  return (
    <table>
      <tbody>
        <tr>
          <td>REC-1001</td>
        </tr>
      </tbody>
    </table>
  );
}
`,
  "src/components/records/RecordDetails.tsx": `export function RecordDetails() {
  return <section>Record details</section>;
}
`,
  "src/lib/api.ts": `export async function getRecords() {
  return [];
}
`,
  "src/lib/format.ts": `export function formatCurrency(value: number) {
  return new Intl.NumberFormat("en-US", {
    style: "currency",
    currency: "USD",
  }).format(value);
}
`,
  "src/lib/localDraft.ts": `export const localDraft = true;
`,
  "package.json": `{
  "name": "frontend-app",
  "private": true
}
`,
  "README.md": `# Frontend app

Mock repository fixture for Norn browser development.

## Usage

- Run \`pnpm dev\`
- Open the [local app](https://example.com)

\`\`\`ts
export const preview = true;
\`\`\`
`,
};

const mockRepositoryFileDiffs: Record<string, string> = {
  "src/App.tsx": `diff --git a/src/App.tsx b/src/App.tsx
index 1111111..2222222 100644
--- a/src/App.tsx
+++ b/src/App.tsx
@@ -1,5 +1,5 @@
 import { RecordTable } from "./components/records/RecordTable";
 
 export function App() {
-  return <RecordTable />;
+  return <RecordTable showLocalDraft />;
 }
`,
  "src/lib/format.ts": `diff --git a/src/lib/format.ts b/src/lib/format.ts
new file mode 100644
index 0000000..3333333
--- /dev/null
+++ b/src/lib/format.ts
@@ -0,0 +1,6 @@
+export function formatCurrency(value: number) {
+  return new Intl.NumberFormat("en-US", {
+    style: "currency",
+    currency: "USD",
+  }).format(value);
+}
`,
  "src/lib/localDraft.ts": `diff --git a/src/lib/localDraft.ts b/src/lib/localDraft.ts
new file mode 100644
index 0000000..4444444
--- /dev/null
+++ b/src/lib/localDraft.ts
@@ -0,0 +1 @@
+export const localDraft = true;
`,
  "src/lib/legacy.ts": `diff --git a/src/lib/legacy.ts b/src/lib/legacy.ts
deleted file mode 100644
index 5555555..0000000
--- a/src/lib/legacy.ts
+++ /dev/null
@@ -1 +0,0 @@
-export const legacy = true;
`,
};

function mockBlameForPath(path: string): RepositoryBlameLine[] {
  const content = mockRepositoryFileContents[path] ?? "";
  const lineCount = content.split("\n").length;
  return Array.from({ length: lineCount }, (_, index) => ({
    line: index + 1,
    sha: "6f52c9a1cf5cd075762f13d0b0f8bf8d0f4f3f7d",
    shortSha: "6f52c9a1",
    author: index % 2 === 0 ? "Ada Lovelace" : "Grace Hopper",
    authorEmail: index % 2 === 0 ? "ada@example.com" : "grace@example.com",
    authorTime: 1710000000 + index * 60,
    summary: path.endsWith("format.ts") ? "Add currency formatting helper" : "Update fixture file",
    message: path.endsWith("format.ts")
      ? "Add currency formatting helper\n\nUse a shared helper for display consistency."
      : "Update fixture file\n\nRefresh the mock repository content used by the explorer.",
  }));
}

/** localStorage-backed store that simulates on-disk review persistence for browser dev mode. */
const REVIEW_STORE_KEY = "norn.mock.reviews.v1";
const LEGACY_REVIEW_STORE_KEY = "lachesi.mock.reviews";

function reviewKey(args?: Record<string, unknown>): string {
  return `${args?.workspace}_${args?.repo}_${args?.id}`;
}

function runKey(args?: Record<string, unknown>): string {
  return `${args?.workspace}/${args?.repo}/${args?.id}`;
}

function fixKey(args?: Record<string, unknown>): string {
  return `${runKey(args)}/${String(args?.threadId ?? "default")}`;
}

function branchKey(args?: Record<string, unknown>): string {
  return `${args?.workspace}/${args?.repo}/${args?.source}/${args?.destination}`;
}

function updateReviewRunState(key: string, patch: Partial<AiReviewRunState>): AiReviewRunState {
  const current =
    mockReviewRunStates.get(key) ??
    ({
      prKey: key,
      prTitle: null,
      threadId: null,
      turnKind: null,
      status: "idle",
      logs: [],
      startedAt: null,
      finishedAt: null,
      generatedAt: null,
      error: null,
    } satisfies AiReviewRunState);
  const next = { ...current, ...patch };
  mockReviewRunStates.set(key, next);
  return next;
}

function updateFixState(key: string, patch: Partial<AiReviewFixState>): AiReviewFixState {
  const current =
    mockFixStates.get(key) ??
    ({
      prKey: key.split("/").slice(0, 3).join("/"),
      threadId: key.split("/").slice(3).join("/") || null,
      repoPath: mockConfig.repos[0]?.localPath ?? null,
      status: "idle",
      phase: "idle",
      logs: [],
      startedAt: null,
      finishedAt: null,
      suggestedCommitMessage: null,
      summary: null,
      commitSha: null,
      error: null,
      filesTouched: [],
      tests: [],
      claudeDurationMs: null,
      claudeSessionId: null,
    } satisfies AiReviewFixState);
  const next = { ...current, ...patch };
  mockFixStates.set(key, next);
  return next;
}

function clearMockReviewRunTimer(key: string): void {
  const timers = mockReviewRunTimers.get(key);
  if (timers?.length) {
    for (const timer of timers) {
      window.clearTimeout(timer);
    }
  }
  mockReviewRunTimers.delete(key);
}

function trackMockReviewRunTimer(key: string, timer: number): void {
  const current = mockReviewRunTimers.get(key) ?? [];
  mockReviewRunTimers.set(key, [...current, timer]);
}

function appendMockReviewLog(key: string, line: string): void {
  const current = mockReviewRunStates.get(key);
  if (!current) return;
  if (current.logs[current.logs.length - 1] === line) return;
  updateReviewRunState(key, {
    logs: [...current.logs, line],
  });
}

function nowId(prefix: string): string {
  return `${prefix}-${Date.now()}-${Math.random().toString(16).slice(2, 8)}`;
}

function createMessage(role: "user" | "assistant", content: string): AiReviewMessage {
  return {
    id: nowId("msg"),
    role,
    content,
    createdAt: String(Date.now()),
  };
}

function createSavedReviewStore(
  args: Record<string, unknown> | undefined,
  content: string,
): SavedReview {
  const key = reviewKey(args);
  const now = String(Date.now());
  const threadId = nowId("thread");
  const review: SavedReview = {
    content,
    generatedAt: now,
  };
  setReviewStore(key, {
    activeThreadId: threadId,
    threads: [
      {
        id: threadId,
        title: "AI review",
        createdAt: now,
        updatedAt: now,
        claudeSessionId: `mock-session-${threadId}`,
        messages: [createMessage("assistant", content)],
      },
    ],
    reviewRuns: getReviewStore(key)?.reviewRuns ?? [],
  });
  return review;
}

function loadSavedReviewFromStore(args: Record<string, unknown> | undefined): SavedReview | null {
  const store = getReviewStore(reviewKey(args));
  const thread = store?.threads[store.threads.length - 1];
  if (!thread) return null;
  let message: AiReviewMessage | null = null;
  for (let index = thread.messages.length - 1; index >= 0; index -= 1) {
    const candidate = thread.messages[index];
    if (candidate?.role === "assistant") {
      message = candidate;
      break;
    }
  }
  if (!message) return null;
  return {
    content: message.content,
    generatedAt: message.createdAt,
  };
}

function loadReviewStore(): Map<string, AiReviewStore> {
  try {
    const raw = readMigratedStorageValue(
      localStorage,
      REVIEW_STORE_KEY,
      LEGACY_REVIEW_STORE_KEY,
      (value) => {
        const parsed: unknown = JSON.parse(value);
        if (parsed == null || typeof parsed !== "object" || Array.isArray(parsed)) return null;
        const valid = Object.values(parsed).every((entry) => {
          if (entry == null || typeof entry !== "object") return false;
          const review = entry as Record<string, unknown>;
          return (
            (review.activeThreadId == null || typeof review.activeThreadId === "string") &&
            Array.isArray(review.threads) &&
            (review.reviewRuns == null || Array.isArray(review.reviewRuns))
          );
        });
        return valid ? JSON.stringify(parsed) : null;
      },
    );
    if (raw) {
      const obj = JSON.parse(raw) as Record<string, AiReviewStore>;
      const stored = new Map(Object.entries(obj));
      mockReviewStoreMemory.clear();
      for (const [key, review] of stored) mockReviewStoreMemory.set(key, review);
      return stored;
    }
  } catch {
    // Use the in-memory browser mock when localStorage is unavailable.
  }
  return new Map(mockReviewStoreMemory);
}

function saveReviewStore(store: Map<string, AiReviewStore>): void {
  mockReviewStoreMemory.clear();
  for (const [key, review] of store) mockReviewStoreMemory.set(key, review);
  try {
    const obj = Object.fromEntries(store);
    localStorage.setItem(REVIEW_STORE_KEY, JSON.stringify(obj));
  } catch {
    // ignore storage failures
  }
}

function getReviewStore(key: string): AiReviewStore | undefined {
  return loadReviewStore().get(key);
}

function setReviewStore(key: string, review: AiReviewStore): void {
  const store = loadReviewStore();
  store.set(key, review);
  saveReviewStore(store);
}

function ensureFindingPublication(
  finding: ReviewFinding,
  mode: ReviewFindingPublication["mode"],
): ReviewFindingPublication {
  if (!finding.publication) {
    finding.publication = {
      mode,
      draftIds: [],
      remoteCommentIds: [],
      publishedAt: null,
    };
  }
  return finding.publication;
}

function applyFindingPublicationEvent(
  finding: ReviewFinding,
  event: ReviewFindingPublicationEvent,
): void {
  switch (event.kind) {
    case "stageDraft": {
      const publication = ensureFindingPublication(finding, event.mode);
      publication.mode = event.mode;
      if (event.draftId && !publication.draftIds.includes(event.draftId)) {
        publication.draftIds.push(event.draftId);
      }
      return;
    }
    case "removeDraft": {
      if (!finding.publication) return;
      if (event.draftId) {
        finding.publication.draftIds = finding.publication.draftIds.filter(
          (draftId) => draftId !== event.draftId,
        );
      }
      if (
        finding.publication.draftIds.length === 0 &&
        finding.publication.remoteCommentIds.length === 0 &&
        !finding.publication.publishedAt
      ) {
        finding.publication = null;
      }
      if (!finding.publication && finding.status === "published") {
        finding.status = "new";
      }
      return;
    }
    case "publishDraft": {
      const publication = ensureFindingPublication(finding, event.mode);
      publication.mode = event.mode;
      if (event.draftId) {
        publication.draftIds = publication.draftIds.filter((draftId) => draftId !== event.draftId);
      }
      if (
        event.remoteCommentId != null &&
        !publication.remoteCommentIds.includes(event.remoteCommentId)
      ) {
        publication.remoteCommentIds.push(event.remoteCommentId);
      }
      publication.publishedAt = event.publishedAt ?? new Date().toISOString();
      finding.status = "published";
    }
  }
}

function recordMockFindingPublicationEvents(
  store: AiReviewStore,
  events: ReviewFindingPublicationEvent[],
): AiReviewStore {
  if (!store.reviewRuns?.length || events.length === 0) return store;
  const reviewRuns = store.reviewRuns.map((run) => ({
    ...run,
    findings: run.findings.map((finding) => ({
      ...finding,
      publication: finding.publication
        ? {
            ...finding.publication,
            draftIds: [...finding.publication.draftIds],
            remoteCommentIds: [...finding.publication.remoteCommentIds],
          }
        : null,
    })),
  }));

  for (const event of events) {
    const run = reviewRuns.find((candidate) => candidate.id === event.reviewRunId);
    const finding = run?.findings.find(
      (candidate) => candidate.fingerprint === event.findingFingerprint,
    );
    if (!finding) continue;
    applyFindingPublicationEvent(finding, event);
  }

  return { ...store, reviewRuns };
}

function deleteReviewStore(key: string): void {
  const store = loadReviewStore();
  store.delete(key);
  saveReviewStore(store);
}

function pruneReviews(keepKeys: string[]): void {
  const store = loadReviewStore();
  const keepSet = new Set(keepKeys);
  for (const key of store.keys()) {
    if (!keepSet.has(key)) store.delete(key);
  }
  saveReviewStore(store);
}

function defaultInitialReviewContent(): string {
  return "## AI Review\n\n### Summary\n\nThis PR looks good overall. A few observations:\n\n- **Logic**: The changes are well-structured and follow existing patterns.\n- **Tests**: Consider adding unit tests for the new utility functions.\n- **Performance**: No obvious performance concerns.\n\n### Suggestions\n\n1. `prAgeDays` could short-circuit when `createdOn` is an empty string.\n2. The `AgeBadge` inline styles could be extracted into a CSS class for reuse.\n\n### Conclusion\n\nApproved with minor suggestions.\n\n## Resources\n\n- [React Hooks reference](https://react.dev/reference/react) — Official docs for all built-in hooks used in this PR\n- [TypeScript Utility Types](https://www.typescriptlang.org/docs/handbook/utility-types.html) — Reference for `Partial`, `Pick`, `Omit` and other utilities\n- [Tauri v2 Commands](https://v2.tauri.app/develop/calling-rust/) — How frontend calls Rust commands via IPC";
}

function defaultReplyContent(userMessage: string): string {
  return `I reviewed your follow-up.\n\n- I considered: ${userMessage}\n- The earlier findings still mostly stand, but I would narrow them to the actionable items only.\n- If you want, I can produce a shorter final review focused on bugs and regressions.`;
}

interface MockCompletedReviewInput {
  provider: ReviewRun["provider"];
  workspace: string;
  repo: string;
  prId: number;
  sourceBranch: string;
  destinationBranch: string;
  reviewedBaseSha: string | null;
  reviewedHeadSha: string | null;
  reviewProfile: string | null;
}

function mockReviewProvider(workspace: string, repo: string): ReviewRun["provider"] {
  return (
    mockConfig.repos.find(
      (candidate) => candidate.workspace === workspace && candidate.repo === repo,
    )?.provider ?? mockConfig.reviewProvider
  );
}

function enqueueMockInlineReviewSuccess(
  key: string,
  title: string,
  reviewStoreKey: string,
  threadId: string,
  content: string,
  turnKind: "initial" | "reply",
  review: MockCompletedReviewInput,
): void {
  clearMockReviewRunTimer(key);
  for (const [delay, line] of [
    [160, "Claude session initialized (claude-sonnet-4)"],
    [360, "Finding files matching `**/*.{ts,tsx}`."],
    [620, "Reading file: src/App.tsx"],
    [880, "Claude is drafting the review…"],
  ] as const) {
    trackMockReviewRunTimer(
      key,
      window.setTimeout(() => {
        appendMockReviewLog(key, line);
      }, delay),
    );
  }
  const timer = window.setTimeout(() => {
    const currentStore = getReviewStore(reviewStoreKey);
    if (!currentStore) return;
    const thread = currentStore.threads.find((candidate) => candidate.id === threadId);
    if (!thread) return;
    const message = createMessage("assistant", content);
    thread.messages = [...thread.messages, message];
    thread.updatedAt = message.createdAt;
    thread.claudeSessionId = thread.claudeSessionId ?? `mock-session-${threadId}`;
    currentStore.activeThreadId = threadId;
    const runId = nowId("run");
    currentStore.reviewRuns = [
      ...(currentStore.reviewRuns ?? []),
      {
        id: runId,
        schemaVersion: "v0.1",
        provider: review.provider,
        workspace: review.workspace,
        repo: review.repo,
        prId: review.prId,
        sourceBranch: review.sourceBranch,
        destinationBranch: review.destinationBranch,
        reviewedBaseSha: review.reviewedBaseSha,
        reviewedHeadSha: review.reviewedHeadSha,
        status: "succeeded",
        turnKind,
        reviewProfile: review.reviewProfile,
        createdAt: message.createdAt,
        finishedAt: message.createdAt,
        diffFingerprint: `mock:${review.reviewedBaseSha ?? "unknown"}:${review.reviewedHeadSha ?? "unknown"}`,
        threadId,
        summaryMarkdown: content,
        evidence: [],
        findings: [
          {
            id: `${runId}-finding-1`,
            fingerprint: "mock:build-records-url:filter-id",
            title: "Add filter id regression coverage",
            severity: "medium",
            confidence: "high",
            category: "test",
            status: "new",
            summary: "The changed filter id path needs regression coverage.",
            rationale: null,
            ruleId: null,
            source: "llm",
            anchor: {
              path: "src/app/views/utils/buildRecordsUrlFromSavedView.ts",
              startLine: 17,
              endLine: 17,
              side: "new",
            },
            suggestedFix: "Add a focused test for the generated filter query.",
            evidenceIds: [],
            publication: null,
          },
        ],
      },
    ];
    setReviewStore(reviewStoreKey, currentStore);
    updateReviewRunState(key, {
      prTitle: title,
      threadId,
      turnKind,
      status: "succeeded",
      finishedAt: String(Date.now()),
      generatedAt: message.createdAt,
      error: null,
      logs: [
        ...(mockReviewRunStates.get(key)?.logs ?? []),
        "Claude produced a result.",
        "Claude finished successfully.",
      ],
    });
    mockReviewRunTimers.delete(key);
  }, 1200);
  trackMockReviewRunTimer(key, timer);
}

function enqueueMockFixSuccess(key: string): void {
  window.setTimeout(() => {
    updateFixState(key, {
      status: "running",
      phase: "runningClaude",
      logs: [
        "Starting AI review fix pipeline…",
        "Using the configured local path for this repository.",
        "Claude is applying the review feedback…",
      ],
    });
  }, 120);
  window.setTimeout(() => {
    updateFixState(key, {
      status: "succeeded",
      phase: "readyToCommit",
      finishedAt: String(Date.now()),
      summary: "Claude fixed the actionable review findings and updated the affected files.",
      suggestedCommitMessage: "Fix AI review findings for PR #1731",
      filesTouched: [
        "src/app/views/utils/buildRecordsUrlFromSavedView.ts",
        "src/app/views/utils/buildRecordsUrlFromSavedView.spec.ts",
      ],
      tests: ["pnpm test -- buildRecordsUrlFromSavedView"],
      claudeDurationMs: 530,
      claudeSessionId: "mock-session-1",
      logs: [
        "Starting AI review fix pipeline…",
        "Using the configured local path for this repository.",
        "Claude is applying the review feedback…",
        "Claude finished successfully. Review the suggested commit message and commit when ready.",
      ],
      error: null,
    });
  }, 650);
}

function enqueueMockCommitSuccess(key: string, message: string): void {
  const current = mockFixStates.get(key);
  if (!current) return;
  window.setTimeout(() => {
    updateFixState(key, {
      status: "succeeded",
      phase: "readyToPush",
      finishedAt: String(Date.now()),
      suggestedCommitMessage: message,
      commitSha: "abc123def456",
      logs: [
        ...current.logs,
        "Staging Claude's changes.",
        "Creating commit. Pre-commit hooks may be running…",
        "Commit created successfully: abc123def456",
      ],
      error: null,
    });
  }, 400);
}

function enqueueMockPushSuccess(key: string): void {
  const current = mockFixStates.get(key);
  if (!current) return;
  window.setTimeout(() => {
    updateFixState(key, {
      status: "succeeded",
      phase: "completed",
      finishedAt: String(Date.now()),
      logs: [
        ...current.logs,
        "Pushing the branch. Pre-push hooks may be running…",
        "Push completed successfully for commit abc123def456.",
      ],
      error: null,
    });
  }, 400);
}

interface NewInlineCommentArgs {
  path: string;
  to: number | null;
  from: number | null;
  raw: string;
  parentId: string | null;
}

function publicationByteLength(value: string): number {
  return new TextEncoder().encode(value).length;
}

function validatePublicationIdentifier(value: string, field: string): void {
  if (!value.trim()) {
    throw new Error(`Structured finding publication requires ${field}.`);
  }
  if (value !== value.trim()) {
    throw new Error(`Structured finding publication ${field} must not contain whitespace.`);
  }
  if (publicationByteLength(value) > 512) {
    throw new Error(`Structured finding publication ${field} is too long.`);
  }
}

function validatePublicationText(value: string, field: string, maximum: number): void {
  if (!value.trim()) {
    throw new Error(`Structured finding publication requires ${field}.`);
  }
  if (publicationByteLength(value) > maximum) {
    throw new Error(`Structured finding publication ${field} is too long.`);
  }
}

function mockPublicationMarkdown(request: FindingPublicationRequest): string {
  const severity = `${request.severity.charAt(0).toUpperCase()}${request.severity.slice(1)}`;
  let markdown = `**${request.title}**\n\n${request.body}\n\nSeverity: **${severity}**`;
  if (request.suggestedFix != null) {
    const longestBacktickRun = Math.max(
      0,
      ...request.suggestedFix.split(/[^`]+/).map((run) => run.length),
    );
    const fence = "`".repeat(Math.max(3, longestBacktickRun + 1));
    markdown += `\n\nSuggested fix:\n\n${fence}suggestion\n${request.suggestedFix}`;
    if (!request.suggestedFix.endsWith("\n")) markdown += "\n";
    markdown += fence;
  }
  return `${markdown}\n\n<!-- lachesi:finding-lineage:${"0".repeat(64)} -->\n<!-- lachesi:finding:${"0".repeat(64)} -->`;
}

function findingPublicationKey(request: FindingPublicationRequest): string {
  return JSON.stringify([
    request.tenantId,
    request.provider,
    request.workspace.toLowerCase(),
    request.repository.toLowerCase(),
    request.pullRequestId,
    request.baseSha.toLowerCase(),
    request.headSha.toLowerCase(),
    request.findingFingerprint,
    request.anchor.path,
    request.anchor.startLine,
    request.anchor.endLine,
    request.anchor.side,
    request.title,
    request.body,
    request.severity,
    request.suggestedFix ?? null,
  ]);
}

export function publishMockReviewFinding(
  request: FindingPublicationRequest,
): PublishedCommentIdentity {
  validatePublicationIdentifier(request.tenantId, "tenantId");
  validatePublicationIdentifier(request.workspace, "workspace");
  validatePublicationIdentifier(request.repository, "repository");
  validatePublicationIdentifier(request.findingFingerprint, "findingFingerprint");
  validatePublicationText(request.title, "title", 1024);
  validatePublicationText(request.body, "body", 64 * 1024);
  if (request.suggestedFix != null) {
    validatePublicationText(request.suggestedFix, "suggestedFix", 64 * 1024);
  }
  if (publicationByteLength(mockPublicationMarkdown(request)) > 32 * 1024) {
    throw new Error("Structured finding publication markdown is too long.");
  }

  const headSha = request.headSha.toLowerCase();
  const baseSha = request.baseSha.toLowerCase();
  if (
    !/^(?:[0-9a-f]{40}|[0-9a-f]{64})$/.test(headSha) ||
    !/^(?:[0-9a-f]{40}|[0-9a-f]{64})$/.test(baseSha)
  ) {
    throw new Error("Structured finding publication requires a full commit SHA.");
  }
  if (request.pullRequestId <= 0 || !Number.isInteger(request.pullRequestId)) {
    throw new Error("Structured finding publication requires a valid pull request id.");
  }
  if (request.pullRequestId !== mockPullRequestDetailState.id) {
    throw new Error("The pull request is unavailable in the browser mock.");
  }
  const anchor = request.anchor;
  if (
    !anchor?.path.trim() ||
    anchor.path.startsWith("/") ||
    anchor.path.includes("\\") ||
    anchor.path.split("/").includes("..") ||
    anchor.path.includes("\0") ||
    publicationByteLength(anchor.path) > 4096 ||
    !Number.isInteger(anchor.startLine) ||
    !Number.isInteger(anchor.endLine) ||
    anchor.startLine <= 0 ||
    anchor.endLine < anchor.startLine
  ) {
    throw new Error("Structured finding publication requires a valid inline anchor.");
  }

  const currentHead = mockPullRequestDetailState.sourceCommitHash?.toLowerCase();
  const currentBase = mockPullRequestDetailState.destinationCommitHash?.toLowerCase();
  if (!currentHead || !currentBase) {
    throw new Error("The current pull request revision is unavailable in the browser mock.");
  }
  if (headSha !== currentHead || baseSha !== currentBase) {
    throw new Error(
      "The pull request changed after this review. Refresh and rerun the review before publishing.",
    );
  }

  const key = findingPublicationKey(request);
  const existing = mockPublishedFindings.get(key);
  if (existing) return existing;

  mockCommentId += 1;
  const published = {
    tenantId: request.tenantId,
    provider: request.provider,
    workspace: request.workspace,
    repository: request.repository,
    pullRequestId: request.pullRequestId,
    commentId: String(mockCommentId),
    findingMarker: `<!-- lachesi:finding:mock-${mockCommentId} -->`,
    path: request.anchor.path,
    startLine: request.anchor.startLine,
    endLine: request.anchor.endLine,
    side: request.anchor.side,
  } satisfies PublishedCommentIdentity;
  mockPublishedFindings.set(key, published);
  return published;
}

function reconcileMockReviewFindings(
  request: FindingReconciliationRequest,
): FindingReconciliationSummary {
  const currentHead = mockPullRequestDetailState.sourceCommitHash?.toLowerCase();
  const currentBase = mockPullRequestDetailState.destinationCommitHash?.toLowerCase();
  if (
    request.pullRequestId !== mockPullRequestDetailState.id ||
    request.headSha.toLowerCase() !== currentHead ||
    request.baseSha.toLowerCase() !== currentBase
  ) {
    throw new Error(
      "The pull request changed after this review. Refresh and rerun the review before reconciling.",
    );
  }
  if (
    request.currentFindings.some(
      (finding) =>
        finding.tenantId !== request.tenantId ||
        finding.provider !== request.provider ||
        finding.workspace.toLowerCase() !== request.workspace.toLowerCase() ||
        finding.repository.toLowerCase() !== request.repository.toLowerCase() ||
        finding.pullRequestId !== request.pullRequestId ||
        finding.baseSha.toLowerCase() !== request.baseSha.toLowerCase() ||
        finding.headSha.toLowerCase() !== request.headSha.toLowerCase(),
    )
  ) {
    throw new Error("Current findings do not match the reconciliation target.");
  }
  const tracked = new Map(
    request.trackedComments.map((comment) => [comment.findingFingerprint, comment.commentId]),
  );
  const actions: FindingReconciliationAction[] = [];
  for (const finding of request.currentFindings) {
    const previousCommentId = tracked.get(finding.findingFingerprint) ?? null;
    tracked.delete(finding.findingFingerprint);
    if (previousCommentId) {
      actions.push({
        findingFingerprint: finding.findingFingerprint,
        kind: "updated",
        previousCommentId,
        commentId: previousCommentId,
        providerMutated: true,
        error: null,
      });
      continue;
    }
    const published = publishMockReviewFinding(finding);
    actions.push({
      findingFingerprint: finding.findingFingerprint,
      kind: "created",
      previousCommentId: null,
      commentId: published.commentId,
      providerMutated: true,
      error: null,
    });
  }
  for (const [findingFingerprint, commentId] of tracked) {
    actions.push({
      findingFingerprint,
      kind: "resolved",
      previousCommentId: commentId,
      commentId,
      providerMutated: true,
      error: null,
    });
  }
  const count = (kind: FindingReconciliationAction["kind"]) =>
    actions.filter((action) => action.kind === kind).length;
  return {
    schemaVersion: "v1",
    status: "succeeded",
    tenantId: request.tenantId,
    provider: request.provider,
    workspace: request.workspace,
    repository: request.repository,
    pullRequestId: request.pullRequestId,
    baseSha: request.baseSha,
    headSha: request.headSha,
    counts: {
      unchanged: count("unchanged"),
      created: count("created"),
      updated: count("updated"),
      resolved: count("resolved"),
      reopened: count("reopened"),
      failed: count("failed"),
    },
    actions,
  };
}

/**
 * Mock implementations of the Rust IPC commands, used when running outside
 * Tauri (browser dev, Storybook, Vitest). Keep the keys in sync with the
 * `#[tauri::command]` names registered in `src-tauri`.
 */
export const mockHandlers: Record<string, Handler> = {
  load_config: () => mockConfig,
  validate_repo_review_config: (args) => ({
    repoPath: String(args?.repoPath ?? ""),
    configPath: `${String(args?.repoPath ?? "")}/.norn.yaml`,
    exists: false,
    config: null,
    selectedProfile: null,
    loadedPolicyPacks: [],
    warnings: [],
    errors: [],
  }),
  list_repository_worktrees: () => [
    {
      workspace: "example-workspace",
      repo: "frontend-app",
      localPath: "/Users/alex/dev/example/frontend-app",
      status: "clean",
      currentBranch: "feature/review-panel",
      detachedHead: null,
      dirty: false,
      branches: [
        { name: "develop", reference: "develop", kind: "local", isCurrent: false },
        {
          name: "feature/review-panel",
          reference: "feature/review-panel",
          kind: "local",
          isCurrent: true,
        },
        { name: "origin/main", reference: "origin/main", kind: "remote", isCurrent: false },
      ],
      error: null,
    },
    {
      workspace: "example-workspace",
      repo: "backend-api",
      localPath: "/Users/alex/dev/example/backend-api",
      status: "dirty",
      currentBranch: "develop",
      detachedHead: null,
      dirty: true,
      branches: [
        { name: "develop", reference: "develop", kind: "local", isCurrent: true },
        { name: "origin/main", reference: "origin/main", kind: "remote", isCurrent: false },
      ],
      error: null,
    },
  ],
  list_repository_files: () => mockRepositoryFiles,
  read_repository_file: (args) => {
    const path = String(args?.path ?? "");
    const content = mockRepositoryFileContents[path];
    if (content == null) {
      throw new Error(`Mock file not found: ${path}`);
    }
    return {
      path,
      content,
      size: new TextEncoder().encode(content).length,
      truncated: false,
    } satisfies RepositoryFileContent;
  },
  get_repository_file_blame: (args) => {
    const path = String(args?.path ?? "");
    if (mockRepositoryFileContents[path] == null) {
      throw new Error(`Mock file not found: ${path}`);
    }
    return mockBlameForPath(path);
  },
  get_repository_file_diff: (args) => {
    const path = String(args?.path ?? "");
    return {
      path,
      rawDiff: mockRepositoryFileDiffs[path] ?? "",
    } satisfies RepositoryFileDiff;
  },
  open_repository_file_external: (args) => {
    const path = String(args?.path ?? "");
    if (mockRepositoryFileContents[path] == null) {
      throw new Error(`Mock file not found: ${path}`);
    }
    return null;
  },
  checkout_repository_branch: (args) => ({
    workspace: String(args?.workspace ?? "example-workspace"),
    repo: String(args?.repo ?? "frontend-app"),
    localPath: "/Users/alex/dev/example/frontend-app",
    status: "clean",
    currentBranch: String(args?.branchRef ?? "develop").replace(/^origin\//, ""),
    detachedHead: null,
    dirty: false,
    branches: [
      { name: "develop", reference: "develop", kind: "local", isCurrent: false },
      { name: "origin/main", reference: "origin/main", kind: "remote", isCurrent: false },
    ],
    error: null,
  }),
  fetch_repository: (args) => ({
    workspace: String(args?.workspace ?? "example-workspace"),
    repo: String(args?.repo ?? "frontend-app"),
    localPath: "/Users/alex/dev/example/frontend-app",
    status: "clean",
    currentBranch: "feature/review-panel",
    detachedHead: null,
    dirty: false,
    branches: [
      { name: "develop", reference: "develop", kind: "local", isCurrent: false },
      {
        name: "feature/review-panel",
        reference: "feature/review-panel",
        kind: "local",
        isCurrent: true,
      },
      { name: "origin/main", reference: "origin/main", kind: "remote", isCurrent: false },
      {
        name: "origin/feature/new-branch",
        reference: "origin/feature/new-branch",
        kind: "remote",
        isCurrent: false,
      },
    ],
    error: null,
  }),
  pull_repository: (args) => ({
    workspace: String(args?.workspace ?? "example-workspace"),
    repo: String(args?.repo ?? "frontend-app"),
    localPath: "/Users/alex/dev/example/frontend-app",
    status: "clean",
    currentBranch: "feature/review-panel",
    detachedHead: null,
    dirty: false,
    branches: [
      { name: "develop", reference: "develop", kind: "local", isCurrent: false },
      {
        name: "feature/review-panel",
        reference: "feature/review-panel",
        kind: "local",
        isCurrent: true,
      },
      { name: "origin/main", reference: "origin/main", kind: "remote", isCurrent: false },
    ],
    error: null,
  }),
  stash_repository: (args) => ({
    workspace: String(args?.workspace ?? "example-workspace"),
    repo: String(args?.repo ?? "backend-api"),
    localPath: "/Users/alex/dev/example/backend-api",
    status: "clean",
    currentBranch: "develop",
    detachedHead: null,
    dirty: false,
    branches: [
      { name: "develop", reference: "develop", kind: "local", isCurrent: true },
      { name: "origin/main", reference: "origin/main", kind: "remote", isCurrent: false },
    ],
    error: null,
  }),
  list_ai_review_jobs: () => mockReviewJobs,
  create_ai_review_job: (args) => {
    const now = new Date().toISOString();
    const job: AiReviewJob = {
      id: nowId("job"),
      workspace: String(args?.workspace ?? "example-workspace"),
      repo: String(args?.repo ?? "frontend-app"),
      prId: Number(args?.prId ?? args?.id ?? 0),
      prTitle: String(args?.prTitle ?? `PR #${String(args?.prId ?? args?.id ?? "")}`),
      sourceBranch: String(args?.sourceBranch ?? "feature/review-panel"),
      destinationBranch: String(args?.destinationBranch ?? "develop"),
      status: "queued",
      trigger: String(args?.trigger ?? "manual"),
      threadId: null,
      error: null,
      createdAt: now,
      startedAt: null,
      finishedAt: null,
    };
    mockReviewJobs = [job, ...mockReviewJobs];
    return job;
  },
  update_ai_review_job_status: (args) => {
    const jobId = String(args?.jobId ?? "");
    const next = mockReviewJobs.find((job) => job.id === jobId);
    if (!next) throw new Error(`Unknown review job: ${jobId}`);
    const status = String(args?.status ?? next.status) as AiReviewJob["status"];
    const updated: AiReviewJob = {
      ...next,
      status,
      threadId: args?.threadId == null ? next.threadId : String(args.threadId),
      error: args?.error == null ? null : String(args.error),
      startedAt: next.startedAt ?? (status === "running" ? new Date().toISOString() : null),
      finishedAt:
        status === "succeeded" || status === "failed" || status === "cancelled"
          ? new Date().toISOString()
          : next.finishedAt,
    };
    mockReviewJobs = mockReviewJobs.map((job) => (job.id === jobId ? updated : job));
    return updated;
  },
  has_credentials: () => true,
  test_connection: () => ({ displayName: "Alex Reviewer" }),
  get_current_user: () => ({ displayName: "Alex Reviewer", accountId: "alex" }),
  save_config: () => null,
  save_credentials: () => null,
  save_github_token: () => null,
  clear_credentials: () => null,
  list_review_terminals: () => [
    { id: "wezterm", label: "WezTerm", available: true },
    { id: "iterm", label: "iTerm2", available: true },
    { id: "terminal", label: "Terminal", available: true },
  ],

  list_pull_requests: (args) => {
    const opts = (args?.opts ?? {}) as { state?: PrListFilter };
    const filter = opts.state ?? "OPEN";
    const values =
      filter === "ALL" ? mockPullRequests : mockPullRequests.filter((pr) => pr.state === filter);
    const page: PullRequestPage = {
      values,
      size: values.length,
      page: 1,
      hasNext: false,
    };
    return page;
  },

  list_closed_pr_metrics: () => ({ metrics: mockClosedPrMetrics, syncedCount: 0 }),
  get_review_effectiveness_metrics: (args) => {
    const filter = (args?.filter ??
      mockReviewEffectivenessReport.filter) as ReviewEffectivenessFilter;
    const rangeMs =
      filter.fromMs == null || filter.toMs == null ? null : filter.toMs - filter.fromMs;
    const fixture =
      rangeMs != null && rangeMs <= SEVEN_DAYS_MS
        ? mockReviewEffectivenessLast7DaysReport
        : mockReviewEffectivenessReport;
    const repositories = fixture.repositories.filter(
      (repository) =>
        (filter.provider == null || repository.provider === filter.provider) &&
        (filter.workspace == null || repository.workspace === filter.workspace) &&
        (filter.repo == null || repository.repo === filter.repo),
    );
    const summary =
      repositories.length === fixture.repositories.length
        ? fixture.summary
        : (repositories[0]?.summary ?? mockEmptyReviewEffectivenessSummary);
    return {
      ...fixture,
      filter,
      summary,
      repositories,
    };
  },
  sync_closed_pr_metrics: (args) => {
    const updatedAfter = String(
      (args?.options as { updatedAfter?: string } | undefined)?.updatedAfter ?? "",
    );
    const since = updatedAfter ? new Date(updatedAfter).getTime() : Number.NEGATIVE_INFINITY;
    const metrics = Number.isFinite(since)
      ? mockClosedPrMetrics.filter((metric) => new Date(metric.updatedOn).getTime() >= since)
      : mockClosedPrMetrics;
    return {
      metrics,
      syncedCount: metrics.length,
    };
  },

  get_pull_request: () => mockPullRequestDetailState,
  approve_pull_request: () => {
    mockPullRequestDetailState = {
      ...mockPullRequestDetailState,
      reviewers: mockPullRequestDetailState.reviewers.map((reviewer) =>
        reviewer.accountId === "alex" ? { ...reviewer, approved: true } : reviewer,
      ),
    };
    return mockPullRequestDetailState;
  },
  get_branch_status: (args) =>
    mockBranchStatuses.get(branchKey(args)) ?? {
      behind: 4,
      ahead: 2,
      behindCapped: false,
      aheadCapped: false,
    },
  get_diffstat: () => mockDiffstat,
  get_pr_diff: () => mockRawDiff,
  get_pr_file_preview: (args) => {
    const path = String(args?.path ?? "public/review-preview.svg");
    const mimeType = path.toLowerCase().endsWith(".svg") ? "image/svg+xml" : "image/png";
    const dataUrl =
      mimeType === "image/svg+xml"
        ? `data:image/svg+xml;base64,${btoa(mockSvgPreview)}`
        : "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+/p9sAAAAASUVORK5CYII=";
    return {
      path,
      mimeType,
      dataUrl,
      size: dataUrl.length,
    } satisfies PrFilePreview;
  },
  list_comments: () => mockComments,

  publish_review_finding: (args) => {
    const request = args?.request as FindingPublicationRequest | undefined;
    if (!request) {
      throw new Error("Structured finding publication requires a request.");
    }
    return publishMockReviewFinding(request);
  },
  reconcile_review_findings: (args) => {
    const request = args?.request as FindingReconciliationRequest | undefined;
    if (!request) {
      throw new Error("Structured finding reconciliation requires a request.");
    }
    return reconcileMockReviewFindings(request);
  },
  create_inline_comment: (args) => {
    const req = (args?.req ?? {}) as NewInlineCommentArgs;
    mockCommentId += 1;
    const comment: PrComment = {
      id: String(mockCommentId),
      parentId: req.parentId ?? null,
      contentRaw: req.raw,
      userDisplayName: "Alex Reviewer",
      createdOn: new Date(0).toISOString(),
      deleted: false,
      inline: { path: req.path, to: req.to ?? null, from: req.from ?? null },
    };
    return comment;
  },
  create_general_comment: (args) => {
    const raw = (args?.raw ?? "") as string;
    mockCommentId += 1;
    const comment: PrComment = {
      id: String(mockCommentId),
      parentId: (args?.parentId as string | null) ?? null,
      contentRaw: raw,
      userDisplayName: "Alex Reviewer",
      createdOn: new Date(0).toISOString(),
      deleted: false,
      inline: null,
    };
    return comment;
  },
  delete_comment: () => null,
  launch_claude_review: () => null,
  run_inline_review: (args) => {
    const payload = String(args?.payload ?? "").trim();
    const content =
      payload.length > 0
        ? defaultInitialReviewContent()
        : "## AI Review\n\nNo review payload was provided.";
    return createSavedReviewStore(args, content);
  },
  load_saved_review: (args) => loadSavedReviewFromStore(args),
  delete_saved_review: (args) => {
    deleteReviewStore(reviewKey(args));
    return null;
  },
  load_ai_review_store: (args) => getReviewStore(reviewKey(args)) ?? null,
  create_ai_review_thread: (args) => {
    const key = reviewKey(args);
    const threadId = nowId("thread");
    const now = String(Date.now());
    const initialMessage = String(args?.initialMessage ?? "").trim();
    const current = getReviewStore(key);
    const next: AiReviewStore = {
      activeThreadId: threadId,
      threads: [
        ...(current?.threads ?? []),
        {
          id: threadId,
          title: String(args?.title ?? "Ask"),
          createdAt: now,
          updatedAt: now,
          claudeSessionId: null,
          messages: initialMessage ? [createMessage("user", initialMessage)] : [],
        },
      ],
      reviewRuns: current?.reviewRuns ?? [],
    };
    setReviewStore(key, next);
    return next;
  },
  set_active_ai_review_thread: (args) => {
    const key = reviewKey(args);
    const current = getReviewStore(key);
    if (!current) return null;
    const threadId = String(args?.threadId ?? "");
    const next = { ...current, activeThreadId: threadId };
    setReviewStore(key, next);
    return next;
  },
  delete_ai_review_thread: (args) => {
    const key = reviewKey(args);
    const current = getReviewStore(key);
    if (!current) return null;
    const threadId = String(args?.threadId ?? "");
    const threads = current.threads.filter((thread) => thread.id !== threadId);
    if (threads.length === 0) {
      deleteReviewStore(key);
      return null;
    }
    const next: AiReviewStore = {
      activeThreadId: threads[0]?.id ?? null,
      threads,
      reviewRuns: current.reviewRuns?.filter((run) => run.threadId !== threadId),
    };
    setReviewStore(key, next);
    return next;
  },
  record_ai_review_finding_publication: (args) => {
    const key = reviewKey(args);
    const current = getReviewStore(key);
    if (!current) return null;
    const events = (args?.events as ReviewFindingPublicationEvent[] | undefined) ?? [];
    const next = recordMockFindingPublicationEvents(current, events);
    setReviewStore(key, next);
    return next;
  },
  cleanup_stale_reviews: (args) => {
    const keepKeys = (args?.keepKeys as string[] | undefined) ?? [];
    pruneReviews(keepKeys);
  },
  get_ai_review_run_state: (args) => mockReviewRunStates.get(runKey(args)) ?? null,
  start_inline_review: (args) => {
    const key = runKey(args);
    const title = String(args?.title ?? `PR #${String(args?.id ?? "")}`);
    const workspace = String(args?.workspace ?? "");
    const repo = String(args?.repo ?? "");
    const prId = Number(args?.id ?? 0);
    const storeKey = reviewKey(args);
    const threadId = nowId("thread");
    const now = String(Date.now());
    const displayMessage = String(args?.displayMessage ?? "").trim();
    const aiProvider = String(args?.aiProvider ?? "claude") === "codex" ? "codex" : "claude";
    const providerLabel = aiProvider === "codex" ? "Codex" : "Claude";
    const reviewProfile = String(args?.reviewProfile ?? "").trim();
    const nextStore: AiReviewStore = {
      activeThreadId: threadId,
      threads: [
        ...(getReviewStore(storeKey)?.threads ?? []),
        {
          id: threadId,
          title: "AI review",
          createdAt: now,
          updatedAt: now,
          claudeSessionId: null,
          messages: displayMessage ? [createMessage("user", displayMessage)] : [],
        },
      ],
      reviewRuns: getReviewStore(storeKey)?.reviewRuns ?? [],
    };
    setReviewStore(storeKey, nextStore);
    const next = updateReviewRunState(key, {
      prKey: key,
      prTitle: title,
      threadId,
      turnKind: "initial",
      status: "running",
      logs: [
        "Starting AI review…",
        `Reviewing PR: ${title}`,
        `Saving output to review thread ${threadId}.`,
        `AI provider: ${providerLabel}`,
        ...(reviewProfile ? [`Review profile: ${reviewProfile}`] : []),
        ...(aiProvider === "codex" && args?.codexModel
          ? [`Codex model: ${String(args.codexModel)}`]
          : []),
        ...(aiProvider === "codex" && args?.codexEffort
          ? [`Codex effort: ${String(args.codexEffort)}`]
          : []),
        ...(aiProvider === "claude" && args?.claudeModel
          ? [`Claude model: ${String(args.claudeModel)}`]
          : []),
        ...(aiProvider === "claude" && args?.claudeEffort
          ? [`Claude effort: ${String(args.claudeEffort)}`]
          : []),
      ],
      startedAt: String(Date.now()),
      finishedAt: null,
      generatedAt: null,
      error: null,
    });
    enqueueMockInlineReviewSuccess(
      key,
      title,
      storeKey,
      threadId,
      defaultInitialReviewContent(),
      "initial",
      {
        provider: mockReviewProvider(workspace, repo),
        workspace,
        repo,
        prId,
        sourceBranch: String(args?.sourceBranch ?? ""),
        destinationBranch: String(args?.destinationBranch ?? ""),
        reviewedBaseSha: String(args?.reviewedBaseSha ?? "").trim() || null,
        reviewedHeadSha: String(args?.reviewedHeadSha ?? "").trim() || null,
        reviewProfile: reviewProfile || null,
      },
    );
    return next;
  },
  reply_inline_review: (args) => {
    const key = runKey(args);
    const title = String(args?.title ?? `PR #${String(args?.id ?? "")}`);
    const workspace = String(args?.workspace ?? "");
    const repo = String(args?.repo ?? "");
    const prId = Number(args?.id ?? 0);
    const storeKey = reviewKey(args);
    const threadId = String(args?.threadId ?? "");
    const userMessage = String(args?.userMessage ?? "").trim();
    const currentStore = getReviewStore(storeKey);
    const thread = currentStore?.threads.find((candidate) => candidate.id === threadId);
    if (!currentStore || !thread || !userMessage) {
      return updateReviewRunState(key, {
        prKey: key,
        prTitle: title,
        threadId,
        turnKind: "reply",
        status: "failed",
        finishedAt: String(Date.now()),
        error: "Unable to append a reply to the active AI review thread.",
      });
    }
    const userEntry = createMessage("user", userMessage);
    thread.messages = [...thread.messages, userEntry];
    thread.updatedAt = userEntry.createdAt;
    currentStore.activeThreadId = threadId;
    setReviewStore(storeKey, currentStore);
    const next = updateReviewRunState(key, {
      prKey: key,
      prTitle: title,
      threadId,
      turnKind: "reply",
      status: "running",
      logs: [
        "Continuing AI review chat…",
        `Reviewing PR: ${title}`,
        `Saving output to review thread ${threadId}.`,
      ],
      startedAt: String(Date.now()),
      finishedAt: null,
      generatedAt: null,
      error: null,
    });
    enqueueMockInlineReviewSuccess(
      key,
      title,
      storeKey,
      threadId,
      defaultReplyContent(userMessage),
      "reply",
      {
        provider: mockReviewProvider(workspace, repo),
        workspace,
        repo,
        prId,
        sourceBranch: String(args?.sourceBranch ?? ""),
        destinationBranch: String(args?.destinationBranch ?? ""),
        reviewedBaseSha: String(args?.reviewedBaseSha ?? "").trim() || null,
        reviewedHeadSha: String(args?.reviewedHeadSha ?? "").trim() || null,
        reviewProfile: null,
      },
    );
    return next;
  },
  cancel_inline_review: (args) => {
    const key = runKey(args);
    clearMockReviewRunTimer(key);
    return updateReviewRunState(key, {
      status: "cancelled",
      finishedAt: String(Date.now()),
      error: null,
      logs: [...(mockReviewRunStates.get(key)?.logs ?? []), "Review cancelled by the user."],
    });
  },
  draft_ai_review_comments: () =>
    [
      {
        path: "src/app/views/utils/buildRecordsUrlFromSavedView.ts",
        to: 17,
        from: null,
        raw: "Please add a regression test covering the `filterId` path used for the generated records URL.",
      },
    ] satisfies AiReviewDraftCommentSuggestion[],
  get_ai_review_fix_state: (args) => mockFixStates.get(fixKey(args)) ?? null,
  start_ai_review_fix: (args) => {
    const key = fixKey(args);
    const prKey = runKey(args);
    const next = updateFixState(key, {
      prKey,
      threadId: (args?.threadId as string | null) ?? null,
      repoPath: mockConfig.repos[0]?.localPath ?? null,
      status: "running",
      phase: "preflight",
      logs: ["Starting AI review fix pipeline…"],
      startedAt: String(Date.now()),
      finishedAt: null,
      suggestedCommitMessage: null,
      summary: null,
      commitSha: null,
      error: null,
      filesTouched: [],
      tests: [],
      claudeDurationMs: null,
      claudeSessionId: null,
    });
    enqueueMockFixSuccess(key);
    return next;
  },
  start_ai_review_commit: (args) => {
    const key = fixKey(args);
    const current = mockFixStates.get(key);
    const message = String(args?.message ?? "");
    const next = updateFixState(key, {
      status: "running",
      phase: "committing",
      suggestedCommitMessage: message,
      logs: [...(current?.logs ?? []), "Creating commit. Pre-commit hooks may be running…"],
      error: null,
    });
    enqueueMockCommitSuccess(key, message);
    return next;
  },
  start_ai_review_push: (args) => {
    const key = fixKey(args);
    const current = mockFixStates.get(key);
    const next = updateFixState(key, {
      status: "running",
      phase: "pushing",
      logs: [...(current?.logs ?? []), "Pushing the branch. Pre-push hooks may be running…"],
      error: null,
    });
    enqueueMockPushSuccess(key);
    return next;
  },
  reset_ai_review_fix_state: (args) => {
    mockFixStates.delete(fixKey(args));
    return null;
  },
  sync_pr_branch: (args) => {
    const source = String(args?.sourceBranch ?? "");
    const destination = String(args?.destinationBranch ?? "");
    mockBranchStatuses.set(`${args?.workspace}/${args?.repo}/${source}/${destination}`, {
      behind: 0,
      ahead: 3,
      behindCapped: false,
      aheadCapped: false,
    });
    return {
      status: "success",
      repoPath: mockConfig.repos[0]?.localPath ?? "/mock/repo",
      sourceBranch: source,
      destinationBranch: destination,
      summary: `Merged ${destination} into ${source} and pushed the updated branch to origin.`,
      syncCommitSha: "sync123abc456",
      warning: null,
      conflictFiles: [],
      logs: [
        "Fetching latest commits from origin.",
        `Merging origin/${destination} into ${source}.`,
        "Pushing synced branch to origin.",
      ],
    } satisfies BranchSyncResult;
  },
  start_ai_conflict_resolution: (args) => {
    const key = fixKey(args);
    const prKey = runKey(args);
    const tips =
      typeof args?.tips === "string" && args.tips.trim().length > 0 ? args.tips.trim() : null;
    const next = updateFixState(key, {
      prKey,
      threadId: (args?.threadId as string | null) ?? null,
      repoPath: mockConfig.repos[0]?.localPath ?? null,
      status: "running",
      phase: "resolvingConflicts",
      logs: [
        "Starting AI conflict resolution pipeline…",
        "Recreating the merge state with the destination branch.",
        ...(tips ? [`Applying reviewer tips: ${tips}`] : []),
        "Claude is resolving the merge conflicts…",
      ],
      startedAt: String(Date.now()),
      finishedAt: null,
      suggestedCommitMessage: null,
      summary: null,
      commitSha: null,
      error: null,
      filesTouched: [],
      tests: [],
      claudeDurationMs: null,
      claudeSessionId: null,
    });
    window.setTimeout(() => {
      updateFixState(key, {
        status: "succeeded",
        phase: "readyToCommit",
        finishedAt: String(Date.now()),
        summary:
          "Claude resolved the merge conflicts and staged the result. Review the merge commit and commit when ready.",
        suggestedCommitMessage: "Merge develop into feature/user-notes-mock-endpoints",
        filesTouched: [
          "src/app/modules/user-notes/user-notes.service.ts",
          "src/app/modules/user-notes/dtos/query-user-notes-body.dto.ts",
        ],
        tests: ["pnpm test -- user-notes"],
        claudeDurationMs: 820,
        claudeSessionId: "mock-conflict-session-1",
        logs: [
          "Starting AI conflict resolution pipeline…",
          "Recreating the merge state with the destination branch.",
          ...(tips ? [`Applying reviewer tips: ${tips}`] : []),
          "Claude is resolving the merge conflicts…",
          "Claude resolved the merge conflicts. Review the merge commit message and commit when ready.",
        ],
        error: null,
      });
    }, 800);
    return next;
  },
  save_jira_token: () => null,
  save_notion_token: () => null,
  get_jira_issue: (args) => ({
    key: (args?.key as string) ?? "APP-0000",
    summary: "Add status and owner filters for records",
    status: "In Progress",
    descriptionText:
      "As a user I want to filter records by status and owner.\nAcceptance: filters combine (AND); empty values are ignored.",
    notionUrls: ["https://www.notion.so/example/Record-filters-abc123def4567890abc123def45678"],
  }),
  get_notion_page: () => ({
    title: "Record filters - spec",
    text: "## Goal\nLet users narrow records by status and owner.\n- Filters combine (AND)\n- An empty filter is ignored",
  }),
};
