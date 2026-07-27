// Core domain types for lachesi. Frontend DTOs mirror the Rust command outputs
// (serde `rename_all = "camelCase"`), so the IPC boundary stays type-safe.

export type ReviewProvider = "bitbucket" | "github";

/** A source-control repository the app tracks. */
export interface RepoRef {
  provider?: ReviewProvider;
  workspace: string;
  repo: string;
  localPath?: string | null;
}

export type RepositoryWorktreeStatus = "clean" | "dirty" | "missingPath" | "invalidRepo" | "error";

export type RepositoryBranchKind = "local" | "remote";

export interface RepositoryBranchOption {
  name: string;
  reference: string;
  kind: RepositoryBranchKind;
  isCurrent: boolean;
}

export interface RepositoryWorktreeState {
  workspace: string;
  repo: string;
  localPath: string | null;
  status: RepositoryWorktreeStatus;
  currentBranch: string | null;
  detachedHead: string | null;
  dirty: boolean;
  branches: RepositoryBranchOption[];
  error: string | null;
}

export interface RepositoryFileEntry {
  path: string;
  status: RepositoryFileStatus;
}

export type RepositoryFileStatus =
  | "unchanged"
  | "modified"
  | "added"
  | "deleted"
  | "renamed"
  | "untracked"
  | "conflicted";

export interface RepositoryFileContent {
  path: string;
  content: string;
  size: number;
  truncated: boolean;
}

export interface RepositoryFileDiff {
  path: string;
  rawDiff: string;
}

export interface RepositoryBlameLine {
  line: number;
  sha: string;
  shortSha: string;
  author: string | null;
  authorEmail: string | null;
  authorTime: number | null;
  summary: string | null;
  message: string | null;
}

export type AppSelection =
  | { kind: "pr-list" }
  | { kind: "overview" }
  | { kind: "closed-analytics" }
  | { kind: "settings" }
  | {
      kind: "pr";
      workspace: string;
      repo: string;
      prId: number;
      activeFilePath: string | null;
      activeFileLine?: number | null;
    };

export type PrState = "OPEN" | "MERGED" | "DECLINED" | "SUPERSEDED";

export interface PullRequestSummary {
  id: number;
  title: string;
  authorDisplayName: string;
  authorAccountId?: string | null;
  authorAvatar?: string | null;
  sourceBranch: string;
  destinationBranch: string;
  state: PrState;
  draft: boolean;
  commentCount: number;
  createdOn: string;
  updatedOn: string;
  reviewers?: Participant[];
  /** Origin repo, tagged on the frontend after fetching per repo. */
  workspace: string;
  repo: string;
}

export interface Participant {
  displayName: string;
  accountId?: string | null;
  role?: string | null;
  approved?: boolean;
}

export interface PullRequestDetail {
  id: number;
  title: string;
  descriptionRaw: string;
  state: PrState;
  draft: boolean;
  authorDisplayName: string;
  reviewers: Participant[];
  sourceBranch: string;
  destinationBranch: string;
  sourceCommitHash?: string | null;
  destinationCommitHash?: string | null;
  createdOn: string;
  updatedOn: string;
}

export type ReviewReferenceType = "pullRequest" | "repository" | "jira" | "notion" | "note";
export type ReviewReferenceSource = "detected" | "manual";

export interface ReviewReference {
  id: string;
  type: ReviewReferenceType;
  source: ReviewReferenceSource;
  title?: string;
  url?: string;
  key?: string;
  workspace?: string;
  repo?: string;
  prId?: number;
  localPath?: string;
  body?: string;
  createdAt: number;
  updatedAt: number;
}

export interface JiraIssue {
  key: string;
  summary: string;
  status: string;
  descriptionText: string;
  notionUrls: string[];
}

export interface NotionPage {
  title: string;
  text: string;
}

export interface BranchStatus {
  /** Commits on the destination branch not in the source (how far behind). */
  behind: number;
  /** Commits on the source branch not in the destination (the PR's own work). */
  ahead: number;
  behindCapped: boolean;
  aheadCapped: boolean;
}

export type BranchSyncStatus = "success" | "conflict";

export interface BranchSyncResult {
  status: BranchSyncStatus;
  repoPath: string;
  sourceBranch: string;
  destinationBranch: string;
  summary: string;
  syncCommitSha: string | null;
  warning: string | null;
  conflictFiles: string[];
  logs: string[];
}

export interface AiReviewContext {
  workspace: string;
  repo: string;
  pr: PullRequestDetail;
  branchStatus: BranchStatus | null;
  rawDiff: string;
  jiraKeys: string[];
  jiraBaseUrl: string | null;
  jiraContext: string | null;
}

export interface AiLineQuestionContext {
  path: string;
  side: "new" | "old";
  to: number | null;
  from: number | null;
  lineText: string;
}

export type AiReviewRunStatus = "idle" | "running" | "succeeded" | "failed" | "cancelled";
export type AiReviewTurnKind = "initial" | "reply";
export type AiReviewMessageRole = "user" | "assistant";

export interface AiReviewMessage {
  id: string;
  role: AiReviewMessageRole;
  content: string;
  createdAt: string;
}

export interface AiReviewThread {
  id: string;
  title: string;
  createdAt: string;
  updatedAt: string;
  claudeSessionId: string | null;
  messages: AiReviewMessage[];
}

export interface AiReviewStore {
  activeThreadId: string | null;
  threads: AiReviewThread[];
  reviewRuns?: ReviewRun[];
}

export interface AiReviewRunState {
  prKey: string;
  prTitle: string | null;
  threadId: string | null;
  turnKind: AiReviewTurnKind | null;
  status: AiReviewRunStatus;
  logs: string[];
  startedAt: string | null;
  finishedAt: string | null;
  generatedAt: string | null;
  error: string | null;
}

export type AiReviewJobStatus = "queued" | "running" | "succeeded" | "failed" | "cancelled";

export interface AiReviewJob {
  id: string;
  workspace: string;
  repo: string;
  prId: number;
  prTitle: string;
  sourceBranch: string;
  destinationBranch: string;
  status: AiReviewJobStatus;
  trigger: string;
  threadId: string | null;
  error: string | null;
  createdAt: string;
  startedAt: string | null;
  finishedAt: string | null;
}

export type ReviewSchemaVersion = "v0.1";
export type ReviewFindingSeverity = "info" | "low" | "medium" | "high" | "critical";
export type ReviewFindingConfidence = "low" | "medium" | "high";
export type ReviewFindingCategory =
  | "bug"
  | "security"
  | "performance"
  | "architecture"
  | "typing"
  | "test"
  | "maintainability"
  | "docs"
  | "other";
export type ReviewFindingStatus =
  | "new"
  | "dismissed"
  | "accepted"
  | "published"
  | "fixed"
  | "stale";
export type ReviewFindingSource = "llm" | "analyzer" | "merged";
export type ReviewEvidenceKind = "conversation" | "diff" | "analyzer" | "doc" | "manual";
export type ReviewEvidenceSource =
  | "claude"
  | "bitbucket-diff"
  | "jira"
  | "notion"
  | "tsc"
  | "biome"
  | "tests"
  | "semgrep"
  | "other";
export type ReviewAnchorSide = "new" | "old";
export type ReviewPublicationMode = "inline" | "file" | "general" | "localOnly";

export interface ReviewFindingAnchor {
  path: string;
  startLine: number;
  endLine: number | null;
  side: ReviewAnchorSide;
}

export interface FindingPublicationRequest {
  schemaVersion: "v1";
  tenantId: string;
  provider: ReviewProvider;
  workspace: string;
  repository: string;
  pullRequestId: number;
  headSha: string;
  findingFingerprint: string;
  anchor: {
    path: string;
    startLine: number;
    endLine: number;
    side: ReviewAnchorSide;
  };
  title: string;
  body: string;
  severity: ReviewFindingSeverity;
  suggestedFix?: string;
}

export interface PublishedCommentIdentity {
  tenantId: string;
  provider: ReviewProvider;
  workspace: string;
  repository: string;
  pullRequestId: number;
  commentId: string;
  findingMarker: string;
  path: string;
  startLine: number;
  endLine: number;
  side: ReviewAnchorSide;
}

export interface ReviewFindingPublication {
  mode: ReviewPublicationMode;
  draftIds: string[];
  remoteCommentIds: string[];
  publishedAt: string | null;
}

export type ReviewFindingPublicationEventKind = "stageDraft" | "removeDraft" | "publishDraft";

export interface ReviewFindingPublicationEvent {
  kind: ReviewFindingPublicationEventKind;
  reviewRunId: string;
  findingFingerprint: string;
  mode: ReviewPublicationMode;
  draftId: string | null;
  remoteCommentId: string | null;
  publishedAt: string | null;
}

export interface ReviewEvidenceArtifact {
  id: string;
  kind: ReviewEvidenceKind;
  source: ReviewEvidenceSource;
  title: string;
  summary: string | null;
  payload: string | null;
}

export interface ReviewFinding {
  id: string;
  fingerprint: string;
  title: string;
  severity: ReviewFindingSeverity;
  confidence: ReviewFindingConfidence;
  category: ReviewFindingCategory;
  status: ReviewFindingStatus;
  summary: string;
  rationale: string | null;
  ruleId: string | null;
  source: ReviewFindingSource;
  anchor: ReviewFindingAnchor | null;
  suggestedFix: string | null;
  evidenceIds: string[];
  publication: ReviewFindingPublication | null;
}

export interface ReviewRun {
  id: string;
  schemaVersion: ReviewSchemaVersion;
  provider: ReviewProvider;
  workspace: string;
  repo: string;
  prId: number;
  sourceBranch: string;
  destinationBranch: string;
  reviewedHeadSha?: string | null;
  status: AiReviewRunStatus;
  turnKind: AiReviewTurnKind;
  reviewProfile: string | null;
  createdAt: string;
  finishedAt: string | null;
  diffFingerprint: string;
  threadId: string | null;
  summaryMarkdown: string | null;
  evidence: ReviewEvidenceArtifact[];
  findings: ReviewFinding[];
}

export interface ReviewFindingRef {
  reviewRunId: string;
  findingId: string;
  findingFingerprint: string;
}

export interface AiReviewDraftCommentSuggestion {
  path: string;
  to: number | null;
  from: number | null;
  raw: string;
}

export type AiReviewFixStatus = "idle" | "running" | "succeeded" | "failed";

export type AiReviewFixPhase =
  | "idle"
  | "preflight"
  | "stashing"
  | "switchingBranch"
  | "syncing"
  | "mergingDestination"
  | "restoringStash"
  | "resolvingConflicts"
  | "runningClaude"
  | "verifyingChanges"
  | "readyToCommit"
  | "committing"
  | "readyToPush"
  | "pushing"
  | "completed";

export interface AiReviewFixState {
  prKey: string;
  threadId: string | null;
  repoPath: string | null;
  status: AiReviewFixStatus;
  phase: AiReviewFixPhase;
  logs: string[];
  startedAt: string | null;
  finishedAt: string | null;
  suggestedCommitMessage: string | null;
  summary: string | null;
  commitSha: string | null;
  error: string | null;
  filesTouched: string[];
  tests: string[];
  claudeDurationMs: number | null;
  claudeSessionId: string | null;
}

export type DiffFileStatus = "modified" | "added" | "removed" | "renamed";

export interface DiffstatEntry {
  status: DiffFileStatus;
  linesAdded: number;
  linesRemoved: number;
  oldPath: string | null;
  newPath: string | null;
}

export interface PrFilePreview {
  path: string;
  mimeType: string;
  dataUrl: string;
  size: number;
}

export interface InlineAnchor {
  path: string;
  /** Line in the new file (additions / context). */
  to: number | null;
  /** Line in the old file (deletions). */
  from: number | null;
}

export interface PrComment {
  id: number;
  parentId: number | null;
  contentRaw: string;
  contentHtml?: string | null;
  userDisplayName: string;
  createdOn: string;
  deleted: boolean;
  inline: InlineAnchor | null;
}

export interface PullRequestPage {
  values: PullRequestSummary[];
  size: number;
  page: number;
  hasNext: boolean;
}

export type PrListFilter = "OPEN" | "DRAFT" | "MERGED" | "ALL";

export type ClosedPrImpact = "low" | "medium" | "high";

export interface ClosedPrCount {
  key: string;
  count: number;
}

export interface ClosedPrRiskSummary {
  hasAiReview: boolean;
  impact: ClosedPrImpact;
  totalFindings: number;
  highOrCriticalFindings: number;
  severityCounts: ClosedPrCount[];
  categoryCounts: ClosedPrCount[];
}

export interface ClosedPrMetric {
  workspace: string;
  repo: string;
  prId: number;
  title: string;
  authorDisplayName: string;
  authorAccountId: string | null;
  state: PrState;
  sourceBranch: string;
  destinationBranch: string;
  createdOn: string;
  updatedOn: string;
  additions: number;
  deletions: number;
  filesChanged: number;
  diffstatCached: boolean;
  risk: ClosedPrRiskSummary;
  syncedAt: string;
}

export interface ClosedPrAnalyticsSnapshot {
  metrics: ClosedPrMetric[];
  syncedCount: number;
}

export interface ClosedPrAnalyticsSyncOptions {
  daysBack: number;
  limitPerState: number;
}

export interface ClosedPrAnalyticsSyncResult extends ClosedPrAnalyticsSyncOptions {
  syncedCount: number;
}

export type DiffViewMode = "unified" | "split" | "conversation";
export type ReviewTerminal = "wezterm" | "iterm" | "terminal";
export type AiProvider = "claude" | "codex";
export type ClaudeReviewModel = "sonnet" | "opus" | "fable";
export type ClaudeReviewEffort = "low" | "medium" | "high" | "xhigh" | "max";
export type CodexReviewEffort = "low" | "medium" | "high";
export type AutomaticSyncIntervalSeconds = 30 | 60 | 300 | 600;

export interface ReviewTerminalOption {
  id: ReviewTerminal;
  label: string;
  available: boolean;
}

export interface AppConfig {
  repos: RepoRef[];
  reviewProvider: ReviewProvider;
  defaultDiffView: DiffViewMode;
  theme: "light" | "dark";
  reviewTerminal: ReviewTerminal | null;
  aiProvider: AiProvider;
  claudeModel: ClaudeReviewModel | null;
  claudeEffort: ClaudeReviewEffort | null;
  codexModel: string | null;
  codexEffort: CodexReviewEffort | null;
  jiraBaseUrl: string | null;
  automaticSyncIntervalSeconds: AutomaticSyncIntervalSeconds | null;
  menuBarSyncEnabled: boolean;
  notificationsEnabled: boolean;
  configured: boolean;
  hasCredentials: boolean;
  hasGithubCredentials: boolean;
  hasJira: boolean;
  hasNotion: boolean;
}

export type RepoReviewMode = "fast" | "balanced" | "strict";
export type RepoReviewSeverity = "info" | "low" | "medium" | "high" | "critical";
export type RepoReviewConfidence = "low" | "medium" | "high";
export type RepoReviewPublicationMode = "inline" | "file" | "general" | "localOnly";
export type RepoPolicyEnforcement = "prompt" | "analyzer" | "ast" | "manual";
export type RepoProfileAnalyzerRequirement = "optional" | "required" | "disabled";

export interface RepoPathFilters {
  include: string[];
  exclude: string[];
}

export interface RepoReviewConfig {
  version: string;
  review?: {
    profile?: string | null;
    mode?: RepoReviewMode | null;
    prompt?: { extend?: string | null; replace?: string | null } | null;
    findings?: {
      minSeverity?: RepoReviewSeverity | null;
      requireAnchors?: boolean | null;
    } | null;
  } | null;
  profiles: Record<
    string,
    {
      mode?: RepoReviewMode | null;
      minSeverity?: RepoReviewSeverity | null;
      prompt?: { extend?: string | null; replace?: string | null } | null;
      policyPacks: string[];
      analyzers: Record<string, RepoProfileAnalyzerRequirement>;
    }
  >;
  paths?: RepoPathFilters | null;
  policy?: {
    packs: string[];
    sources: Array<{ type: string; path: string }>;
    rules: Array<{
      id: string;
      source?: string | null;
      severity: RepoReviewSeverity;
      confidence?: RepoReviewConfidence | null;
      appliesTo?: RepoPathFilters | null;
      instruction: string;
      rationale?: string | null;
      remediation?: string | null;
      enforcement?: RepoPolicyEnforcement | null;
    }>;
    pathRules: Array<{
      id: string;
      severity: RepoReviewSeverity;
      paths: RepoPathFilters;
      instruction: string;
      rationale?: string | null;
      remediation?: string | null;
    }>;
    astRules: Array<{
      id: string;
      language: string;
      severity: RepoReviewSeverity;
      selector: Record<string, unknown>;
      appliesTo?: RepoPathFilters | null;
      instruction: string;
      rationale?: string | null;
      remediation?: string | null;
    }>;
    suppressions: Array<{
      ruleId: string;
      paths: RepoPathFilters;
      reason: string;
      expiresAt?: string | null;
    }>;
  } | null;
  analyzers: Record<
    string,
    {
      enabled: boolean;
      command?: string | null;
      timeoutSeconds?: number | null;
      required: boolean;
      config?: unknown;
    }
  >;
  publish?: {
    defaultMode?: RepoReviewPublicationMode | null;
    requireManualSubmit?: boolean | null;
    allowGeneralComments?: boolean | null;
  } | null;
}

export interface RepoConfigValidationMessage {
  path: string;
  message: string;
}

export interface LoadedPolicyPack {
  id: string;
  name: string | null;
  path: string;
}

export interface RepoReviewConfigLoadResult {
  repoPath: string;
  configPath: string;
  exists: boolean;
  config: RepoReviewConfig | null;
  selectedProfile: string | null;
  loadedPolicyPacks: LoadedPolicyPack[];
  warnings: RepoConfigValidationMessage[];
  errors: RepoConfigValidationMessage[];
}

/** A locally-staged ("pending review") comment, not yet published to Bitbucket. */
export interface DraftComment {
  /** Local-only id. */
  localId: string;
  prId: number;
  path: string;
  to: number | null;
  from: number | null;
  raw: string;
  parentId: number | null;
  createdAt: number;
  source?: "manual" | "aiFinding";
  findingRef?: ReviewFindingRef | null;
  publicationMode?: ReviewPublicationMode | null;
  reviewHeadSha?: string | null;
}

/** Helper: a stable string key for a repo. */
export function repoKey(r: RepoRef): string {
  return `${r.workspace}/${r.repo}`;
}
