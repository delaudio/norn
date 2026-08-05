import type {
  AppConfig,
  ClosedPrMetric,
  DiffstatEntry,
  PrComment,
  PullRequestDetail,
  PullRequestSummary,
  ReviewEffectivenessReport,
  ReviewEffectivenessSummary,
} from "@/types";

export const mockConfig: AppConfig = {
  repos: [
    {
      provider: "bitbucket",
      workspace: "example-workspace",
      repo: "frontend-app",
      localPath: "/Users/alex/dev/example/frontend-app",
    },
    { provider: "bitbucket", workspace: "example-workspace", repo: "backend-api", localPath: null },
    { provider: "github", workspace: "delaudio", repo: "norn", localPath: null },
  ],
  reviewProvider: "bitbucket",
  defaultDiffView: "unified",
  theme: "dark",
  reviewTerminal: null,
  aiProvider: "claude",
  claudeModel: "sonnet",
  claudeEffort: "high",
  codexModel: null,
  codexEffort: null,
  jiraBaseUrl: "https://example.atlassian.net",
  automaticSyncIntervalSeconds: null,
  menuBarSyncEnabled: true,
  notificationsEnabled: false,
  configured: true,
  hasCredentials: true,
  hasGithubCredentials: true,
  hasJira: true,
  hasNotion: true,
};

const RAW_PULL_REQUESTS: Omit<PullRequestSummary, "workspace" | "repo" | "draft">[] = [
  {
    id: 1732,
    title: "[Draft] feat(settings): add notification preference column",
    authorDisplayName: "Sam Author",
    sourceBranch: "feat/notification-preferences",
    destinationBranch: "develop",
    state: "OPEN",
    commentCount: 3,
    createdOn: "2026-06-17T08:00:00.000Z",
    updatedOn: "2026-06-17T09:12:00.000Z",
    reviewers: [
      { displayName: "Alex Reviewer", accountId: "alex", approved: false },
      { displayName: "Jamie Reviewer", accountId: "jamie", approved: true },
    ],
  },
  {
    id: 1731,
    title: "APP-2066 - fix saved-view filter returning empty results",
    authorDisplayName: "Alex Reviewer",
    sourceBranch: "APP-2066-saved-view-filter",
    destinationBranch: "develop",
    state: "OPEN",
    commentCount: 0,
    createdOn: "2026-06-16T09:00:00.000Z",
    updatedOn: "2026-06-16T15:40:00.000Z",
    reviewers: [{ displayName: "Alex Reviewer", accountId: "alex", approved: true }],
  },
  {
    id: 1729,
    title: "APP-000 - refactor profile-card stories to MSW string patterns",
    authorDisplayName: "Alex Reviewer",
    sourceBranch: "APP-000-msw-stories",
    destinationBranch: "develop",
    state: "OPEN",
    commentCount: 5,
    createdOn: "2026-06-13T10:30:00.000Z",
    updatedOn: "2026-06-15T11:02:00.000Z",
  },
  {
    id: 1728,
    title: "APP-000 - fix i18n loader and auth token in Storybook runtime",
    authorDisplayName: "Alex Reviewer",
    sourceBranch: "APP-000-storybook-i18n",
    destinationBranch: "develop",
    state: "OPEN",
    commentCount: 1,
    createdOn: "2026-06-10T07:45:00.000Z",
    updatedOn: "2026-06-14T08:25:00.000Z",
  },
  {
    id: 1702,
    title: "APP-1791 - connect activity feed and detail panel component",
    authorDisplayName: "Alex Reviewer",
    sourceBranch: "APP-1791-activity-feed-details",
    destinationBranch: "develop",
    state: "OPEN",
    commentCount: 12,
    createdOn: "2026-06-03T14:20:00.000Z",
    updatedOn: "2026-06-10T17:55:00.000Z",
  },
];

export const mockPullRequests: PullRequestSummary[] = RAW_PULL_REQUESTS.map((pr) => ({
  ...pr,
  draft: pr.title.startsWith("[Draft]"),
  workspace: "example-workspace",
  repo: "frontend-app",
}));

export const mockPullRequestDetail: PullRequestDetail = {
  id: 1731,
  title: "APP-2066 - fix saved-view filter returning empty results",
  descriptionRaw:
    "Saved-view navigation was passing a display label instead of the stable filter id.\n\nThis PR adds `filterId` to the saved-view row and uses it for the records lookup.",
  state: "OPEN",
  draft: false,
  authorDisplayName: "Alex Reviewer",
  reviewers: [
    { displayName: "Alex Reviewer", accountId: "alex", approved: false },
    { displayName: "Jamie Reviewer", accountId: "jamie", approved: true },
  ],
  sourceBranch: "APP-2066-saved-view-filter",
  destinationBranch: "develop",
  sourceCommitHash: "2222222222222222222222222222222222222222",
  destinationCommitHash: "1111111111111111111111111111111111111111",
  createdOn: "2026-06-16T09:00:00.000Z",
  updatedOn: "2026-06-16T15:40:00.000Z",
};

export const mockDiffstat: DiffstatEntry[] = [
  {
    status: "modified",
    linesAdded: 21,
    linesRemoved: 4,
    oldPath: "src/app/views/utils/buildRecordsUrlFromSavedView.spec.ts",
    newPath: "src/app/views/utils/buildRecordsUrlFromSavedView.spec.ts",
  },
  {
    status: "modified",
    linesAdded: 19,
    linesRemoved: 4,
    oldPath: "src/app/views/utils/buildRecordsUrlFromSavedView.ts",
    newPath: "src/app/views/utils/buildRecordsUrlFromSavedView.ts",
  },
  {
    status: "added",
    linesAdded: 12,
    linesRemoved: 0,
    oldPath: null,
    newPath: "src/app/views/types/SavedViewRecord.ts",
  },
  {
    status: "modified",
    linesAdded: 0,
    linesRemoved: 0,
    oldPath: "public/review-preview.svg",
    newPath: "public/review-preview.svg",
  },
];

export const mockClosedPrMetrics: ClosedPrMetric[] = [
  {
    workspace: "example-workspace",
    repo: "frontend-app",
    prId: 1701,
    title: "Add activity timeline filters",
    authorDisplayName: "Sam Author",
    authorAccountId: "sam",
    state: "MERGED",
    sourceBranch: "feature/activity-timeline-filters",
    destinationBranch: "develop",
    createdOn: "2026-06-03T09:15:00.000Z",
    updatedOn: "2026-06-05T14:30:00.000Z",
    additions: 214,
    deletions: 58,
    filesChanged: 9,
    diffstatCached: true,
    risk: {
      hasAiReview: true,
      impact: "medium",
      totalFindings: 2,
      highOrCriticalFindings: 0,
      severityCounts: [
        { key: "medium", count: 1 },
        { key: "low", count: 1 },
      ],
      categoryCounts: [
        { key: "maintainability", count: 1 },
        { key: "test", count: 1 },
      ],
    },
    syncedAt: "1782920000000",
  },
  {
    workspace: "example-workspace",
    repo: "frontend-app",
    prId: 1694,
    title: "Fix auth callback token refresh",
    authorDisplayName: "Alex Reviewer",
    authorAccountId: "alex",
    state: "MERGED",
    sourceBranch: "fix/auth-refresh",
    destinationBranch: "main",
    createdOn: "2026-05-21T10:00:00.000Z",
    updatedOn: "2026-05-21T18:40:00.000Z",
    additions: 84,
    deletions: 31,
    filesChanged: 5,
    diffstatCached: true,
    risk: {
      hasAiReview: true,
      impact: "high",
      totalFindings: 1,
      highOrCriticalFindings: 1,
      severityCounts: [{ key: "high", count: 1 }],
      categoryCounts: [{ key: "security", count: 1 }],
    },
    syncedAt: "1782920000000",
  },
  {
    workspace: "example-workspace",
    repo: "backend-api",
    prId: 1188,
    title: "Remove legacy CSV export endpoint",
    authorDisplayName: "Jamie Reviewer",
    authorAccountId: "jamie",
    state: "DECLINED",
    sourceBranch: "remove/csv-export",
    destinationBranch: "main",
    createdOn: "2026-04-28T11:20:00.000Z",
    updatedOn: "2026-05-02T09:10:00.000Z",
    additions: 11,
    deletions: 462,
    filesChanged: 14,
    diffstatCached: true,
    risk: {
      hasAiReview: false,
      impact: "medium",
      totalFindings: 0,
      highOrCriticalFindings: 0,
      severityCounts: [],
      categoryCounts: [],
    },
    syncedAt: "1782920000000",
  },
  {
    workspace: "example-workspace",
    repo: "backend-api",
    prId: 1196,
    title: "Tighten payment webhook idempotency",
    authorDisplayName: "Sam Author",
    authorAccountId: "sam",
    state: "MERGED",
    sourceBranch: "feature/webhook-idempotency",
    destinationBranch: "main",
    createdOn: "2026-06-10T08:00:00.000Z",
    updatedOn: "2026-06-13T16:35:00.000Z",
    additions: 390,
    deletions: 74,
    filesChanged: 12,
    diffstatCached: true,
    risk: {
      hasAiReview: true,
      impact: "high",
      totalFindings: 3,
      highOrCriticalFindings: 1,
      severityCounts: [
        { key: "high", count: 1 },
        { key: "medium", count: 2 },
      ],
      categoryCounts: [
        { key: "bug", count: 1 },
        { key: "architecture", count: 1 },
        { key: "test", count: 1 },
      ],
    },
    syncedAt: "1782920000000",
  },
];

export const mockEmptyReviewEffectivenessSummary: ReviewEffectivenessSummary = {
  reviewCount: 0,
  findingCount: 0,
  findingsBySeverity: [],
  findingsByCategory: [],
  feedback: {
    eligibleFindings: 0,
    findingsWithFeedback: 0,
    findingsWithoutFeedback: 0,
    acceptedFindings: 0,
    falsePositiveFindings: 0,
    fixedFindings: 0,
    dismissedFindings: 0,
    reopenedFindings: 0,
    coverageRate: { numerator: 0, denominator: 0, basisPoints: null },
    acceptanceRate: { numerator: 0, denominator: 0, basisPoints: null },
    falsePositiveRate: { numerator: 0, denominator: 0, basisPoints: null },
    fixedRate: { numerator: 0, denominator: 0, basisPoints: null },
  },
  timeToFirstReview: {
    sampleCount: 0,
    totalMs: 0,
    averageMs: null,
    medianMs: null,
    minimumMs: null,
    maximumMs: null,
  },
};

export const mockReviewEffectivenessReport: ReviewEffectivenessReport = {
  schemaVersion: "norn.review-effectiveness.v1",
  filter: {
    tenantId: "local",
    provider: "bitbucket",
  },
  summary: {
    reviewCount: 48,
    findingCount: 126,
    findingsBySeverity: [
      { key: "critical", count: 4 },
      { key: "high", count: 18 },
      { key: "medium", count: 57 },
      { key: "low", count: 47 },
    ],
    findingsByCategory: [
      { key: "correctness", count: 41 },
      { key: "maintainability", count: 32 },
      { key: "security", count: 18 },
      { key: "test", count: 35 },
    ],
    feedback: {
      eligibleFindings: 126,
      findingsWithFeedback: 101,
      findingsWithoutFeedback: 25,
      acceptedFindings: 72,
      falsePositiveFindings: 9,
      fixedFindings: 54,
      dismissedFindings: 11,
      reopenedFindings: 2,
      coverageRate: { numerator: 101, denominator: 126, basisPoints: 8015 },
      acceptanceRate: { numerator: 72, denominator: 126, basisPoints: 5714 },
      falsePositiveRate: { numerator: 9, denominator: 126, basisPoints: 714 },
      fixedRate: { numerator: 54, denominator: 126, basisPoints: 4285 },
    },
    timeToFirstReview: {
      sampleCount: 41,
      totalMs: 174_660_000,
      averageMs: 4_260_000,
      medianMs: 3_480_000,
      minimumMs: 420_000,
      maximumMs: 21_600_000,
    },
  },
  repositories: [
    {
      provider: "bitbucket",
      workspace: "example-workspace",
      repo: "frontend-app",
      summary: {
        reviewCount: 31,
        findingCount: 82,
        findingsBySeverity: [
          { key: "critical", count: 1 },
          { key: "high", count: 10 },
          { key: "medium", count: 39 },
          { key: "low", count: 32 },
        ],
        findingsByCategory: [
          { key: "correctness", count: 27 },
          { key: "maintainability", count: 24 },
          { key: "security", count: 8 },
          { key: "test", count: 23 },
        ],
        feedback: {
          eligibleFindings: 82,
          findingsWithFeedback: 70,
          findingsWithoutFeedback: 12,
          acceptedFindings: 53,
          falsePositiveFindings: 5,
          fixedFindings: 41,
          dismissedFindings: 7,
          reopenedFindings: 1,
          coverageRate: { numerator: 70, denominator: 82, basisPoints: 8536 },
          acceptanceRate: { numerator: 53, denominator: 82, basisPoints: 6463 },
          falsePositiveRate: { numerator: 5, denominator: 82, basisPoints: 609 },
          fixedRate: { numerator: 41, denominator: 82, basisPoints: 5000 },
        },
        timeToFirstReview: {
          sampleCount: 27,
          totalMs: 99_900_000,
          averageMs: 3_700_000,
          medianMs: 3_180_000,
          minimumMs: 420_000,
          maximumMs: 14_400_000,
        },
      },
    },
    {
      provider: "bitbucket",
      workspace: "example-workspace",
      repo: "backend-api",
      summary: {
        reviewCount: 17,
        findingCount: 44,
        findingsBySeverity: [
          { key: "critical", count: 3 },
          { key: "high", count: 8 },
          { key: "medium", count: 18 },
          { key: "low", count: 15 },
        ],
        findingsByCategory: [
          { key: "correctness", count: 14 },
          { key: "maintainability", count: 8 },
          { key: "security", count: 10 },
          { key: "test", count: 12 },
        ],
        feedback: {
          eligibleFindings: 44,
          findingsWithFeedback: 31,
          findingsWithoutFeedback: 13,
          acceptedFindings: 19,
          falsePositiveFindings: 4,
          fixedFindings: 13,
          dismissedFindings: 4,
          reopenedFindings: 1,
          coverageRate: { numerator: 31, denominator: 44, basisPoints: 7045 },
          acceptanceRate: { numerator: 19, denominator: 44, basisPoints: 4318 },
          falsePositiveRate: { numerator: 4, denominator: 44, basisPoints: 909 },
          fixedRate: { numerator: 13, denominator: 44, basisPoints: 2954 },
        },
        timeToFirstReview: {
          sampleCount: 14,
          totalMs: 74_760_000,
          averageMs: 5_340_000,
          medianMs: 4_560_000,
          minimumMs: 780_000,
          maximumMs: 21_600_000,
        },
      },
    },
  ],
};

export const mockReviewEffectivenessLast7DaysReport: ReviewEffectivenessReport = {
  ...mockReviewEffectivenessReport,
  summary: mockReviewEffectivenessReport.repositories[0]!.summary,
  repositories: [mockReviewEffectivenessReport.repositories[0]!],
};

/** A small, real-shaped unified diff used for diff-viewer stories/tests. */
export const mockRawDiff = `diff --git a/src/app/views/utils/buildRecordsUrlFromSavedView.ts b/src/app/views/utils/buildRecordsUrlFromSavedView.ts
index 5fdfc9cb3..154929261 100644
--- a/src/app/views/utils/buildRecordsUrlFromSavedView.ts
+++ b/src/app/views/utils/buildRecordsUrlFromSavedView.ts
@@ -13,9 +13,12 @@ export function buildRecordsUrlFromSavedView(
   const params = new URLSearchParams();
   params.set("tab", "records");

-  params.set("view", row.label);
+  // Use the stable filter id, not the display label.
+  params.set("view", row.filterId);
+  params.set("filter", row.filterId);
   return \`/\${lang}/dashboard/records?\${params.toString()}\`;
 }
`;

export const mockComments: PrComment[] = [
  {
    id: "9001",
    parentId: null,
    contentRaw: "Should we guard against `filterId` being undefined here?",
    userDisplayName: "Sam Author",
    createdOn: "2026-06-16T16:10:00.000Z",
    deleted: false,
    inline: {
      path: "src/app/views/utils/buildRecordsUrlFromSavedView.ts",
      to: 17,
      from: null,
    },
  },
  {
    id: "9002",
    parentId: "9001",
    contentRaw: "Good catch - the row type now makes it required, so it can't be undefined.",
    userDisplayName: "Alex Reviewer",
    createdOn: "2026-06-16T16:25:00.000Z",
    deleted: false,
    inline: {
      path: "src/app/views/utils/buildRecordsUrlFromSavedView.ts",
      to: 17,
      from: null,
    },
  },
];
