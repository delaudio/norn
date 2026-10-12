// AUTO-GENERATED FILE. DO NOT EDIT.
// Source: protocol/norn-protocol.schema.json
// Regenerate with `pnpm run protocol:generate`.

/**
 * Newline-delimited UTF-8 JSON protocol between the OpenTUI client and the Rust backend. Standard output carries these messages only; standard error carries diagnostics. Protocol version 1.
 */
export type NornStdioProtocol = Hello | Ready | Request | ResponseOk | ResponseError | Event;
/**
 * The only supported protocol version for this contract.
 */
export type ProtocolVersion = 1;
export type Request =
  | {
      type: "request";
      id: RequestId;
      method: "diff.file";
      params: DiffFileParams;
    }
  | {
      type: "request";
      id: RequestId;
      method: "review.start";
      params: ReviewStartParams;
    }
  | {
      type: "request";
      id: RequestId;
      method: "repository.status" | "operation.status" | "operation.cancel" | "shutdown";
      params: OperationControlParams;
    }
  | {
      type: "request";
      id: RequestId;
      method: "review.files";
      params: TargetParams;
    }
  | {
      type: "request";
      id: RequestId;
      method: "review.findings";
      params: TargetParams;
    }
  | {
      type: "request";
      id: RequestId;
      method: "review.targets";
      params: EmptyResult;
    }
  | {
      type: "request";
      id: RequestId;
      method: "browser.open";
      params: TargetParams;
    }
  | {
      type: "request";
      id: RequestId;
      method: "file.preview";
      params: FilePreviewParams;
    }
  | {
      type: "request";
      id: RequestId;
      method: "review.history";
      params: TargetParams;
    }
  | {
      type: "request";
      id: RequestId;
      method: "review.reply";
      params: ReviewReplyParams;
    }
  | {
      type: "request";
      id: RequestId;
      method: "review.threads";
      params: TargetParams;
    }
  | {
      type: "request";
      id: RequestId;
      method: "review.fix.preview";
      params: TargetParams;
    }
  | {
      type: "request";
      id: RequestId;
      method: "review.fix";
      params: {
        target: TargetIdentity;
        approvalToken: string;
      };
    }
  | {
      type: "request";
      id: RequestId;
      method: "review.fix.state";
      params: TargetParams;
    }
  | {
      type: "request";
      id: RequestId;
      method: "review.commit.preview";
      params: {
        target: TargetIdentity;
        message: string;
      };
    }
  | {
      type: "request";
      id: RequestId;
      method: "review.commit";
      params: {
        target: TargetIdentity;
        approvalToken: string;
      };
    }
  | {
      type: "request";
      id: RequestId;
      method: "review.push.preview";
      params: TargetParams;
    }
  | {
      type: "request";
      id: RequestId;
      method: "review.push";
      params: {
        target: TargetIdentity;
        approvalToken: string;
      };
    }
  | {
      type: "request";
      id: RequestId;
      method: "review.sync.preview";
      params: TargetParams;
    }
  | {
      type: "request";
      id: RequestId;
      method: "review.sync";
      params: {
        target: TargetIdentity;
        approvalToken: string;
      };
    }
  | {
      type: "request";
      id: RequestId;
      method: "review.conflicts.preview";
      params: TargetParams;
    }
  | {
      type: "request";
      id: RequestId;
      method: "review.conflicts";
      params: {
        target: TargetIdentity;
        approvalToken: string;
      };
    }
  | {
      type: "request";
      id: RequestId;
      method: "settings.get";
      params: EmptyParams;
    }
  | {
      type: "request";
      id: RequestId;
      method: "settings.save";
      params: SettingsSaveParams;
    }
  | {
      type: "request";
      id: RequestId;
      method: "auth.status";
      params: EmptyParams;
    }
  | {
      type: "request";
      id: RequestId;
      method: "auth.set";
      params: AuthSetParams;
    }
  | {
      type: "request";
      id: RequestId;
      method: "auth.clear";
      params: AuthProviderParams;
    }
  | {
      type: "request";
      id: RequestId;
      method: "data.preview";
      params: DataPreviewParams;
    }
  | {
      type: "request";
      id: RequestId;
      method: "data.import";
      params: DataImportParams;
    }
  | {
      type: "request";
      id: RequestId;
      method: "data.export";
      params: DataExportParams;
    }
  | {
      type: "request";
      id: RequestId;
      method: "workspace.read";
      params: RecordReadParams;
    }
  | {
      type: "request";
      id: RequestId;
      method: "workspace.mutate";
      params: RecordMutateParams;
    }
  | {
      type: "request";
      id: RequestId;
      method: "review.output";
      params: TargetParams;
    }
  | {
      type: "request";
      id: RequestId;
      method: "workspace.readiness";
      params: EmptyParams;
    }
  | {
      type: "request";
      id: RequestId;
      method: "draft.prepare";
      params: DraftPrepareParams;
    }
  | {
      type: "request";
      id: RequestId;
      method: "draft.stage";
      params: DraftStageParams;
    }
  | {
      type: "request";
      id: RequestId;
      method: "publication.prepare";
      params: PublicationPrepareParams;
    }
  | {
      type: "request";
      id: RequestId;
      method: "publication.apply";
      params: PublicationApplyParams;
    }
  | {
      type: "request";
      id: RequestId;
      method: "publication.cancel";
      params: PublicationCancelParams;
    }
  | {
      type: "request";
      id: RequestId;
      method: "publication.status";
      params: PublicationStatusParams;
    }
  | {
      type: "request";
      id: RequestId;
      method: "publication.reconcile";
      params: PublicationReconcileParams;
    }
  | {
      type: "request";
      id: RequestId;
      method: "comments.list";
      params: CommentsListParams;
    }
  | {
      type: "request";
      id: RequestId;
      method: "workspace.scopes";
      params: ScopeReadParams;
    }
  | {
      type: "request";
      id: RequestId;
      method: "provider.prs";
      params: ProviderPrListParams;
    }
  | {
      type: "request";
      id: RequestId;
      method: "provider.pr.detail";
      params: ProviderPrDetailParams;
    }
  | {
      type: "request";
      id: RequestId;
      method: "provider.user";
      params: ProviderUserParams;
    }
  | {
      type: "request";
      id: RequestId;
      method: "provider.snapshot";
      params: ProviderPrDetailParams;
    }
  | {
      type: "request";
      id: RequestId;
      method: "provider.snapshot.files";
      params: ProviderSnapshotFilesParams;
    }
  | {
      type: "request";
      id: RequestId;
      method: "provider.snapshot.diff";
      params: ProviderSnapshotDiffParams;
    }
  | {
      type: "request";
      id: RequestId;
      method: "provider.snapshot.image";
      params: ProviderSnapshotImageParams;
    }
  | {
      type: "request";
      id: RequestId;
      method: "provider.snapshot.browser";
      params: ProviderSnapshotBrowserParams;
    }
  | {
      type: "request";
      id: RequestId;
      method: "provider.browser";
      params: ProviderPrDetailParams;
    }
  | {
      type: "request";
      id: RequestId;
      method: "provider.approval.preview";
      params: ProviderPrDetailParams;
    }
  | {
      type: "request";
      id: RequestId;
      method: "provider.approval.confirm";
      params: ProviderApprovalConfirmParams;
    }
  | {
      type: "request";
      id: RequestId;
      method: "provider.approval.status";
      params: ProviderPrDetailParams;
    }
  | {
      type: "request";
      id: RequestId;
      method: "provider.approval.reconcile";
      params: ProviderPrDetailParams;
    }
  | {
      type: "request";
      id: RequestId;
      method: "review.profiles";
      params: ReviewProfilesParams;
    }
  | {
      type: "request";
      id: RequestId;
      method: "local.options";
      params: LocalOptionsParams;
    }
  | {
      type: "request";
      id: RequestId;
      method: "local.snapshot";
      params: LocalCaptureParams;
    }
  | {
      type: "request";
      id: RequestId;
      method: "local.snapshot.cancel";
      params: LocalCancelParams;
    }
  | {
      type: "request";
      id: RequestId;
      method: "local.snapshot.info";
      params: LocalLookupParams;
    }
  | {
      type: "request";
      id: RequestId;
      method: "local.snapshot.files";
      params: LocalFilesParams;
    }
  | {
      type: "request";
      id: RequestId;
      method: "local.snapshot.diff";
      params: LocalDiffParams;
    }
  | {
      type: "request";
      id: RequestId;
      method: "local.snapshot.recheck";
      params: LocalRecheckParams;
    }
  | {
      type: "request";
      id: RequestId;
      method: "local.review.start";
      params: LocalReviewStartParams;
    }
  | {
      type: "request";
      id: RequestId;
      method: "local.review.history";
      params: LocalReviewHistoryParams;
    }
  | {
      type: "request";
      id: RequestId;
      method: "local.review.findings";
      params: LocalReviewFindingsParams;
    }
  | {
      type: "request";
      id: RequestId;
      method: "local.review.output";
      params: LocalReviewQueryParams;
    }
  | {
      type: "request";
      id: RequestId;
      method: "local.review.threads";
      params: LocalReviewQueryParams;
    };
export type RequestId = string;
/**
 * Identity of the review target a request or event applies to.
 */
export type TargetIdentity = ProviderTarget | LocalTarget;
export type ReviewStartParams = {
  target: TargetIdentity;
  profile?: string | null;
  snapshot?: ProviderSnapshotRef | null;
};
export type PrRepositoryIdentity =
  | {
      provider: "github";
      workspace: string;
      repo: string;
    }
  | {
      provider: "bitbucket";
      workspace: string;
      repo: string;
    };
export type FilePreviewParams = {
  target: TargetIdentity;
  path: string;
  side?: "old" | "new" | null;
  offset?: number | null;
  maxBytes?: number | null;
  contentSha256?: string | null;
};
export type AuthSetParams = {
  provider: "github" | "bitbucket" | "jira" | "notion";
  username?: string | null;
  token: string;
};
export type RecordReadParams = {
  key: string;
  offset?: number;
  limit?: number;
  recordVersion?: string | null;
};
export type RecordMutation =
  | {
      kind: "setPrompt";
      value: string;
    }
  | {
      kind: "setTheme";
      value: "dark" | "light";
    }
  | {
      kind: "addViewed";
      path: string;
    }
  | {
      kind: "removeViewed";
      path: string;
    }
  | {
      kind: "putReference";
      reference: WorkspaceReference;
    }
  | {
      kind: "removeReference";
      id: string;
    }
  | {
      kind: "putDraft";
      draft: WorkspaceDraft;
    }
  | {
      kind: "removeDraft";
      localId: string;
    };
export type ScopeReadParams = {
  offset?: number;
  limit?: number;
  inventoryVersion?: string | null;
};
export type ProviderSnapshotImageParams = {
  snapshot: ProviderSnapshotRef;
  path: string;
  side: "old" | "new";
  offset: number;
  maxBytes: number;
  contentSha256: string | null;
};
/**
 * Strict target for the read-only native profile catalog.
 */
export type ReviewProfileTarget = ReviewProfileProviderTarget | ReviewProfileLocalTarget;
export type LocalSource =
  | {
      kind: "current";
    }
  | {
      kind: "configured";
      repository: PrRepositoryIdentity;
    };
export type LocalComparison =
  | {
      source: LocalSource;
      mode: "workingTree";
      baseRef: null;
      /**
       * @minItems 1
       * @maxItems 3
       */
      layers:
        | ["committed" | "staged" | "unstaged"]
        | ["committed" | "staged" | "unstaged", "committed" | "staged" | "unstaged"]
        | [
            "committed" | "staged" | "unstaged",
            "committed" | "staged" | "unstaged",
            "committed" | "staged" | "unstaged"
          ];
    }
  | {
      source: LocalSource;
      mode: "branch";
      baseRef: string;
      /**
       * @minItems 1
       * @maxItems 1
       */
      layers: ["committed"];
    };
export type LocalRelativePath = string;
export type LocalRunId = string;
export type OperationState = "accepted" | "running" | "succeeded" | "failed" | "cancelled";
export type ErrorCode =
  | "invalidRequest"
  | "unsupportedVersion"
  | "unknownMethod"
  | "invalidParams"
  | "cancelled"
  | "notFound"
  | "targetStale"
  | "conflict"
  | "internal";
export type WorkspaceValue =
  | {
      kind: "drafts";
      /**
       * @maxItems 1000
       */
      value: WorkspaceDraft[];
    }
  | {
      kind: "references";
      /**
       * @maxItems 1000
       */
      value: WorkspaceReference[];
    }
  | {
      kind: "viewed";
      /**
       * @maxItems 10000
       */
      value: string[];
    }
  | {
      kind: "prompt";
      value: string;
    }
  | {
      kind: "theme";
      value: "dark" | "light";
    };
export type PublicationMode = "inline" | "file" | "general";
export type PublicationState =
  "writing" | "uncertain" | "rejected" | "published" | "publishedSnapshotChanged" | "publishedUnverified";
export type ProviderSnapshotImagePage = {
  request: ProviderSnapshotImageParams;
  mimeType: string;
  size: number;
  dataBase64: string;
  available: boolean;
  nextOffset: number | null;
  contentSha256: string | null;
  width: number | null;
  height: number | null;
};
export type OperationEvent =
  | {
      kind: "progress";
      message: string;
      completed?: number;
      total?: number;
    }
  | {
      kind: "log";
      level: "debug" | "info" | "warn" | "error";
      message: string;
    }
  | {
      kind: "state";
      state: OperationState;
      error?: ProtocolError;
    };

export interface Hello {
  type: "hello";
  protocolVersion: ProtocolVersion;
  client: {
    name: string;
    version: string;
  };
}
export interface Ready {
  type: "ready";
  protocolVersion: ProtocolVersion;
  serverVersion: string;
  capabilities: string[];
}
export interface DiffFileParams {
  target: TargetIdentity;
  path: string;
  oldPath?: string | null;
  contextLines?: number;
}
export interface ProviderTarget {
  kind: "provider";
  provider: "github" | "bitbucket";
  workspace: string;
  repo: string;
  prId?: number;
  runId?: string | null;
}
export interface LocalTarget {
  kind: "local";
  localSnapshotSha256: string;
}
export interface ProviderSnapshotRef {
  repository: PrRepositoryIdentity;
  prId: number;
  snapshotId: string;
}
export interface OperationControlParams {
  operationId?: string;
}
export interface TargetParams {
  target: TargetIdentity;
}
export interface EmptyResult {}
export interface ReviewReplyParams {
  target: TargetIdentity;
  message: string;
}
export interface EmptyParams {}
export interface SettingsSaveParams {
  expectedVersion: string;
  settings: WorkspaceSettings;
}
export interface WorkspaceSettings {
  /**
   * @maxItems 100
   */
  repos: {
    provider: "github" | "bitbucket";
    workspace: string;
    repo: string;
    localPath?: string | null;
  }[];
  reviewProvider: "github" | "bitbucket";
  aiProvider: "claude" | "codex" | "opencode";
  defaultDiffView: "unified" | "split";
  theme: "dark" | "light";
  jiraBaseUrl?: string | null;
  automaticSyncIntervalSeconds?: number | null;
  claudeModel?: string | null;
  claudeEffort?: string | null;
  codexModel?: string | null;
  codexEffort?: string | null;
  opencodeModel?: string | null;
  opencodeEffort?: string | null;
}
export interface AuthProviderParams {
  provider: "github" | "bitbucket" | "jira" | "notion";
}
export interface DataPreviewParams {
  path: string;
  provider?: "github" | "bitbucket" | null;
}
export interface DataImportParams {
  path: string;
  provider?: "github" | "bitbucket" | null;
  inputVersion: string;
  destinationVersion: string;
  confirm: true;
}
export interface DataExportParams {
  path: string;
}
export interface RecordMutateParams {
  key: string;
  expectedVersion: string;
  mutation: RecordMutation;
}
export interface WorkspaceReference {
  id: string;
  type: "pullRequest" | "repository" | "jira" | "notion" | "note";
  source: "detected" | "manual";
  title?: string | null;
  url?: string | null;
  key?: string | null;
  workspace?: string | null;
  repo?: string | null;
  prId?: number | null;
  localPath?: string | null;
  body?: string | null;
  createdAt: number;
  updatedAt: number;
}
export interface WorkspaceDraft {
  localId: string;
  prId: number;
  path: string;
  to?: number | null;
  from?: number | null;
  raw: string;
  parentId?: string | null;
  createdAt: number;
  source?: ("manual" | "aiFinding") | null;
  findingRef?: WorkspaceFindingReference | null;
  publicationMode?: ("inline" | "file" | "general" | "localOnly") | null;
  reviewBaseSha?: string | null;
  reviewHeadSha?: string | null;
}
export interface WorkspaceFindingReference {
  reviewRunId: string;
  findingId: string;
  findingFingerprint: string;
}
export interface DraftPrepareParams {
  target: DraftTarget;
  findingId: string;
  publicationMode: "inline" | "file" | "general" | "localOnly";
}
export interface DraftTarget {
  kind: "provider";
  provider: "github" | "bitbucket";
  workspace: string;
  repo: string;
  prId: number;
  runId: string;
}
export interface DraftStageParams {
  target: DraftTarget;
  findingId: string;
  publicationMode: "inline" | "file" | "general" | "localOnly";
  expectedVersion: string;
  previewHash: string;
}
export interface PublicationPrepareParams {
  scope: PublicationScope;
  /**
   * @minItems 1
   * @maxItems 20
   */
  localIds:
    | [string]
    | [string, string]
    | [string, string, string]
    | [string, string, string, string]
    | [string, string, string, string, string]
    | [string, string, string, string, string, string]
    | [string, string, string, string, string, string, string]
    | [string, string, string, string, string, string, string, string]
    | [string, string, string, string, string, string, string, string, string]
    | [string, string, string, string, string, string, string, string, string, string]
    | [string, string, string, string, string, string, string, string, string, string, string]
    | [string, string, string, string, string, string, string, string, string, string, string, string]
    | [string, string, string, string, string, string, string, string, string, string, string, string, string]
    | [string, string, string, string, string, string, string, string, string, string, string, string, string, string]
    | [
        string,
        string,
        string,
        string,
        string,
        string,
        string,
        string,
        string,
        string,
        string,
        string,
        string,
        string,
        string
      ]
    | [
        string,
        string,
        string,
        string,
        string,
        string,
        string,
        string,
        string,
        string,
        string,
        string,
        string,
        string,
        string,
        string
      ]
    | [
        string,
        string,
        string,
        string,
        string,
        string,
        string,
        string,
        string,
        string,
        string,
        string,
        string,
        string,
        string,
        string,
        string
      ]
    | [
        string,
        string,
        string,
        string,
        string,
        string,
        string,
        string,
        string,
        string,
        string,
        string,
        string,
        string,
        string,
        string,
        string,
        string
      ]
    | [
        string,
        string,
        string,
        string,
        string,
        string,
        string,
        string,
        string,
        string,
        string,
        string,
        string,
        string,
        string,
        string,
        string,
        string,
        string
      ]
    | [
        string,
        string,
        string,
        string,
        string,
        string,
        string,
        string,
        string,
        string,
        string,
        string,
        string,
        string,
        string,
        string,
        string,
        string,
        string,
        string
      ];
}
export interface PublicationScope {
  provider: "github" | "bitbucket";
  workspace: string;
  repo: string;
  prId: number;
}
export interface PublicationApplyParams {
  scope: PublicationScope;
  approvalId: string;
  localId: string;
}
export interface PublicationCancelParams {
  scope: PublicationScope;
  approvalId: string;
}
export interface PublicationStatusParams {
  scope: PublicationScope;
  after: string | null;
  limit: number;
}
export interface PublicationReconcileParams {
  scope: PublicationScope;
  localId: string;
}
export interface CommentsListParams {
  scope: PublicationScope;
  offset: number;
  limit: number;
  version: string | null;
}
export interface ProviderPrListParams {
  repository: PrRepositoryIdentity;
  filter: "OPEN" | "DRAFT" | "MERGED" | "ALL";
  page: number;
  limit: number;
  query?: string | null;
  authorId?: string | null;
}
export interface ProviderPrDetailParams {
  repository: PrRepositoryIdentity;
  prId: number;
}
export interface ProviderUserParams {
  repository: PrRepositoryIdentity;
}
export interface ProviderSnapshotFilesParams {
  snapshot: ProviderSnapshotRef;
  offset: number;
  limit: number;
}
export interface ProviderSnapshotDiffParams {
  snapshot: ProviderSnapshotRef;
  path: string;
  offset: number;
}
export interface ProviderSnapshotBrowserParams {
  snapshot: ProviderSnapshotRef;
}
export interface ProviderApprovalConfirmParams {
  request: ProviderPrDetailParams;
  approvalId: string;
}
export interface ReviewProfilesParams {
  target: ReviewProfileTarget;
}
export interface ReviewProfileProviderTarget {
  kind: "provider";
  provider: "github" | "bitbucket";
  workspace: string;
  repo: string;
  prId?: number;
  runId?: string;
}
export interface ReviewProfileLocalTarget {
  kind: "local";
  localSnapshotSha256: string;
}
export interface LocalOptionsParams {
  source: LocalSource;
  offset: number;
  limit: number;
}
export interface LocalCaptureParams {
  captureId: string;
  comparison: LocalComparison;
}
export interface LocalCancelParams {
  source: LocalSource;
  captureId: string;
}
export interface LocalLookupParams {
  snapshot: LocalSnapshotRef;
}
export interface LocalSnapshotRef {
  source: LocalSource;
  repository: PrRepositoryIdentity;
  rootId: string;
  snapshotId: string;
}
export interface LocalFilesParams {
  snapshot: LocalSnapshotRef;
  offset: number;
  limit: number;
}
export interface LocalDiffParams {
  snapshot: LocalSnapshotRef;
  layer: "committed" | "staged" | "unstaged";
  path: LocalRelativePath;
  offset: number;
}
export interface LocalRecheckParams {
  snapshot: LocalSnapshotRef;
  captureId: string;
}
export interface LocalReviewStartParams {
  snapshot: LocalSnapshotRef;
  profile?: string | null;
}
export interface LocalReviewHistoryParams {
  snapshot: LocalSnapshotRef;
  offset: number;
  limit: number;
}
export interface LocalReviewFindingsParams {
  snapshot: LocalSnapshotRef;
  runId?: LocalRunId | null;
  offset: number;
  limit: number;
}
export interface LocalReviewQueryParams {
  snapshot: LocalSnapshotRef;
  runId?: LocalRunId | null;
}
export interface ResponseOk {
  type: "response";
  id: RequestId;
  ok: true;
  result:
    | DiffFileResult
    | ReviewStartResult
    | OperationStatusResult
    | RepositoriesResult
    | EmptyResult
    | ReviewFilesResult
    | ReviewFindingsResult
    | ReviewTargetsResult
    | BrowserOpenResult
    | FilePreviewResult
    | ReviewHistoryResult
    | RecordPage
    | RecordMutationResult
    | ReviewOutputResult
    | WorkspaceReadiness
    | DraftPreparation
    | DraftStageResult
    | PublicationPrepareResult
    | PublicationItemResult
    | PublicationCancelResult
    | PublicationStatusResult
    | CommentsListResult
    | ScopePage
    | ProviderPrPage
    | ProviderPrDetail
    | ProviderCurrentUser
    | ProviderSnapshotInfo
    | ProviderSnapshotFilesPage
    | ProviderSnapshotDiffPage
    | ProviderSnapshotImagePage
    | ProviderSnapshotBrowserResult
    | ProviderBrowserResult
    | ProviderApprovalPreview
    | ProviderApprovalResult
    | ReviewProfileCatalog
    | LocalOptionsResult
    | LocalCaptureResult
    | LocalCancelResult
    | LocalInfoResult
    | LocalFilesPage
    | LocalDiffPage
    | LocalRecheckResult
    | LocalReviewAccepted
    | LocalReviewHistoryPage
    | LocalReviewFindingsPage
    | LocalReviewOutput
    | LocalReviewThreads;
}
export interface DiffFileResult {
  target: TargetIdentity;
  path: string;
  diff: string;
  truncated: boolean;
}
export interface ReviewStartResult {
  target: TargetIdentity;
  operationId: string;
  state: OperationState;
}
export interface OperationStatusResult {
  operationId: string;
  state: OperationState;
  sequence: number;
  error?: ProtocolError;
}
export interface ProtocolError {
  code: ErrorCode;
  message: string;
  retryable?: boolean;
}
export interface RepositoriesResult {
  repos: RepoSummary[];
}
export interface RepoSummary {
  provider: "github" | "bitbucket";
  workspace: string;
  repo: string;
  localPath?: string | null;
}
export interface ReviewFilesResult {
  target: TargetIdentity;
  files: ChangedFile[];
  providerSnapshot?: ProviderSnapshotInfo;
}
export interface ChangedFile {
  path: string;
  status: "added" | "modified" | "deleted" | "renamed" | "untracked";
  additions: number;
  deletions: number;
  oldPath?: string | null;
}
export interface ProviderSnapshotInfo {
  request: ProviderPrDetailParams;
  snapshot: ProviderSnapshotRef;
  title: string;
  sourceBranch: string;
  destinationBranch: string;
  headSha: string;
  destinationSha: string;
  baseSha: string;
  patchSha256: string;
  fileCount: number;
  patchBytes: number;
  expiresAt: number;
}
export interface ReviewFindingsResult {
  target: TargetIdentity;
  findings: FindingSummary[];
}
export interface FindingSummary {
  id: string;
  title: string;
  severity: "info" | "low" | "medium" | "high" | "critical";
  summary: string;
  anchor?: FindingAnchorSummary | null;
}
export interface FindingAnchorSummary {
  path: string;
  startLine: number;
  endLine?: number | null;
  side: "new" | "old";
}
export interface ReviewTargetsResult {
  targets: ReviewTarget[];
}
export interface ReviewTarget {
  provider: "github" | "bitbucket";
  workspace: string;
  repo: string;
  prId: number;
  title: string;
  status: string;
  runId: string;
}
export interface BrowserOpenResult {
  url: string;
}
export interface FilePreviewResult {
  path: string;
  mimeType: string;
  size: number;
  dataBase64: string;
  side: "old" | "new";
  available: boolean;
  offset: number;
  nextOffset: number | null;
  contentSha256: string | null;
  width: number | null;
  height: number | null;
}
export interface ReviewHistoryResult {
  runs: ReviewRunSummary[];
}
export interface ReviewRunSummary {
  runId: string;
  createdAt: string;
  status: string;
  turnKind: string;
  findingsCount: number;
}
export interface RecordPage {
  key: string;
  recordVersion: string;
  available: boolean;
  offset: number;
  nextOffset: number | null;
  totalItems: number;
  payload: WorkspaceValue;
}
export interface RecordMutationResult {
  key: string;
  recordVersion: string;
  changed: boolean;
}
export interface ReviewOutputResult {
  target: TargetIdentity;
  runId: string;
  text: string;
}
export interface WorkspaceReadiness {
  configuredRepos: number;
  selectedAgent: "claude" | "codex" | "opencode";
  /**
   * @minItems 4
   * @maxItems 4
   */
  tools: [
    {
      name: "git" | "claude" | "codex" | "opencode";
      available: boolean;
    },
    {
      name: "git" | "claude" | "codex" | "opencode";
      available: boolean;
    },
    {
      name: "git" | "claude" | "codex" | "opencode";
      available: boolean;
    },
    {
      name: "git" | "claude" | "codex" | "opencode";
      available: boolean;
    }
  ];
  configRootAvailable: boolean;
  dataRootAvailable: boolean;
  credentialStore: "native" | "unsupported";
}
export interface DraftPreparation {
  target: DraftTarget;
  key: string;
  recordVersion: string;
  previewHash: string;
  draft: WorkspaceDraft;
  alreadyStaged: boolean;
}
export interface DraftStageResult {
  target: DraftTarget;
  key: string;
  recordVersion: string;
  localId: string;
  changed: boolean;
  alreadyStaged: boolean;
}
export interface PublicationPrepareResult {
  scope: PublicationScope;
  approvalId: string;
  expiresAt: number;
  key: string;
  recordVersion: string;
  /**
   * @minItems 1
   * @maxItems 20
   */
  items:
    | [PublicationPreviewItem]
    | [PublicationPreviewItem, PublicationPreviewItem]
    | [PublicationPreviewItem, PublicationPreviewItem, PublicationPreviewItem]
    | [PublicationPreviewItem, PublicationPreviewItem, PublicationPreviewItem, PublicationPreviewItem]
    | [
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem
      ]
    | [
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem
      ]
    | [
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem
      ]
    | [
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem
      ]
    | [
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem
      ]
    | [
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem
      ]
    | [
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem
      ]
    | [
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem
      ]
    | [
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem
      ]
    | [
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem
      ]
    | [
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem
      ]
    | [
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem
      ]
    | [
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem
      ]
    | [
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem
      ]
    | [
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem
      ]
    | [
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem,
        PublicationPreviewItem
      ];
}
export interface PublicationPreviewItem {
  comment: ApprovedComment;
  markdown: string;
}
export interface ApprovedComment {
  scope: PublicationScope;
  localId: string;
  headSha: string;
  baseSha: string;
  mode: PublicationMode;
  path: string;
  to: number | null;
  from: number | null;
  parentId: string | null;
  raw: string;
  findingRef?: PublicationFindingAssociation;
  originalDraftHash?: string;
}
export interface PublicationFindingAssociation {
  reviewRunId: string;
  findingId: string;
  findingFingerprint: string;
}
export interface PublicationItemResult {
  scope: PublicationScope;
  entry: PublicationJournalEntry;
  localSaved: boolean;
  draftRemoved: boolean;
}
export interface PublicationJournalEntry {
  comment: ApprovedComment;
  payloadHash: string;
  attempt: string;
  state: PublicationState;
  remoteId: string | null;
  confirmedAt?: number;
  startedAt?: number;
}
export interface PublicationCancelResult {
  canceled: boolean;
}
export interface PublicationStatusResult {
  scope: PublicationScope;
  /**
   * @maxItems 50
   */
  entries: PublicationJournalEntry[];
  nextAfter: string | null;
}
export interface CommentsListResult {
  scope: PublicationScope;
  /**
   * @maxItems 100
   */
  comments: RemoteComment[];
  version: string;
  offset: number;
  totalItems: number;
  nextOffset: number | null;
}
export interface RemoteComment {
  id: string;
  parentId: string | null;
  authorId: string;
  authorName: string;
  raw: string;
  createdAt: string;
  deleted: boolean;
  mode: PublicationMode;
  path: string;
  to: number | null;
  from: number | null;
  commitId: string | null;
  rangeStart: number | null;
}
export interface ScopePage {
  inventoryVersion: string;
  offset: number;
  nextOffset: number | null;
  totalScopes: number;
  /**
   * @maxItems 100
   */
  scopes: WorkspaceScope[];
}
export interface WorkspaceScope {
  provider: "github" | "bitbucket";
  workspace: string;
  repo: string;
  prId: number;
  draftCount: number;
  referenceCount: number;
  viewedCount: number;
  localOnly: boolean;
}
export interface ProviderPrPage {
  request: ProviderPrListParams;
  /**
   * @maxItems 100
   */
  values: ProviderPrSummary[];
  nextPage: number | null;
}
export interface ProviderPrSummary {
  id: number;
  title: string;
  authorDisplayName: string;
  sourceBranch: string;
  destinationBranch: string;
  state: "OPEN" | "MERGED" | "DECLINED" | "SUPERSEDED";
  draft: boolean;
  createdOn: string;
  updatedOn: string;
  /**
   * @maxItems 100
   */
  reviewers: ProviderPrReviewer[];
  authorAccountId: string | null;
  commentCount: number;
}
export interface ProviderPrReviewer {
  displayName: string;
  accountId: string | null;
  role: string;
  approved: boolean;
}
export interface ProviderPrDetail {
  request: ProviderPrDetailParams;
  detail: ProviderPrMetadata;
  authorId: string | null;
  browserUrl: string;
}
export interface ProviderPrMetadata {
  id: number;
  title: string;
  authorDisplayName: string;
  sourceBranch: string;
  destinationBranch: string;
  state: "OPEN" | "MERGED" | "DECLINED" | "SUPERSEDED";
  draft: boolean;
  createdOn: string;
  updatedOn: string;
  /**
   * @maxItems 100
   */
  reviewers: ProviderPrReviewer[];
  descriptionRaw: string;
  sourceCommitHash: string | null;
  destinationCommitHash: string | null;
}
export interface ProviderCurrentUser {
  repository: PrRepositoryIdentity;
  accountId: string;
  displayName: string;
}
export interface ProviderSnapshotFilesPage {
  request: ProviderSnapshotFilesParams;
  /**
   * @maxItems 100
   */
  files: ProviderSnapshotFile[];
  total: number;
  nextOffset: number | null;
}
export interface ProviderSnapshotFile {
  path: string;
  oldPath: string | null;
  status: "added" | "deleted" | "renamed" | "copied" | "modified";
  additions: number;
  deletions: number;
  binary: boolean;
}
export interface ProviderSnapshotDiffPage {
  request: ProviderSnapshotDiffParams;
  text: string;
  totalBytes: number;
  contentSha256: string;
  nextOffset: number | null;
}
export interface ProviderSnapshotBrowserResult {
  request: ProviderSnapshotBrowserParams;
  url: string;
  launchRequested: boolean;
  launchError: string | null;
}
export interface ProviderBrowserResult {
  request: ProviderPrDetailParams;
  repository: PrRepositoryIdentity;
  url: string;
  launchRequested: boolean;
  launchError: string | null;
}
export interface ProviderApprovalPreview {
  request: ProviderPrDetailParams;
  proof: ProviderApprovalProof;
  approvalId: string | null;
  expiresAt: number | null;
  entry: ProviderApprovalEntry | null;
}
export interface ProviderApprovalProof {
  request: ProviderPrDetailParams;
  repositoryId: string;
  actor: ProviderApprovalActor;
  title: string;
  headSha: string;
  destinationSha: string;
  sourceBranch: string;
  destinationBranch: string;
  sourceRepository: PrRepositoryIdentity | null;
  state: "OPEN" | "MERGED" | "DECLINED" | "SUPERSEDED";
  draft: boolean;
  alreadyApproved: boolean;
}
export interface ProviderApprovalActor {
  accountId: string;
  displayName: string;
}
export interface ProviderApprovalEntry {
  formatVersion: 1;
  proof: ProviderApprovalProof;
  state: "writing" | "approved" | "rejected" | "uncertain";
  remoteReviewId: string | null;
  updatedAt: number;
  attempt: number;
}
export interface ProviderApprovalResult {
  request: ProviderPrDetailParams;
  proof: ProviderApprovalProof;
  entry: ProviderApprovalEntry | null;
  observedApproved: boolean;
  outdated: boolean;
  localSaved: boolean;
}
export interface ReviewProfileCatalog {
  target: ReviewProfileTarget;
  /**
   * @maxItems 100
   */
  profiles: string[];
  defaultProfile: string | null;
  source: "policy" | "base";
  notice: string | null;
}
export interface LocalOptionsResult {
  request: LocalOptionsParams;
  repository: PrRepositoryIdentity;
  rootId: string;
  branch: string | null;
  headSha: string | null;
  /**
   * @minItems 0
   * @maxItems 32
   */
  baseRefs: string[];
  total: number;
  nextOffset: number | null;
}
export interface LocalCaptureResult {
  request: LocalCaptureParams;
  info: LocalSnapshotInfo;
}
export interface LocalSnapshotInfo {
  comparison: LocalComparison;
  snapshot: LocalSnapshotRef;
  branch: string;
  headSha: string | null;
  baseSha: string;
  nativeSnapshotSha256: string;
  localReviewId: number;
  /**
   * @minItems 1
   * @maxItems 3
   */
  layers: [LocalLayerInfo] | [LocalLayerInfo, LocalLayerInfo] | [LocalLayerInfo, LocalLayerInfo, LocalLayerInfo];
  fileCount: number;
  patchBytes: number;
  /**
   * @minItems 0
   * @maxItems 128
   */
  warnings: string[];
  warningsTruncated: boolean;
}
export interface LocalLayerInfo {
  kind: "committed" | "staged" | "unstaged";
  fileCount: number;
  patchBytes: number;
}
export interface LocalCancelResult {
  request: LocalCancelParams;
  canceled: boolean;
}
export interface LocalInfoResult {
  request: LocalLookupParams;
  info: LocalSnapshotInfo;
}
export interface LocalFilesPage {
  request: LocalFilesParams;
  /**
   * @minItems 0
   * @maxItems 32
   */
  files: LocalSnapshotFile[];
  total: number;
  nextOffset: number | null;
}
export interface LocalSnapshotFile {
  layer: "committed" | "staged" | "unstaged";
  path: LocalRelativePath;
  oldPath: LocalRelativePath | null;
  status: "added" | "modified" | "deleted" | "renamed" | "copied";
  additions: number;
  deletions: number;
  binary: boolean;
}
export interface LocalDiffPage {
  request: LocalDiffParams;
  text: string;
  totalBytes: number;
  contentSha256: string;
  nextOffset: number | null;
}
export interface LocalRecheckResult {
  request: LocalRecheckParams;
  info: LocalSnapshotInfo;
  unchanged: true;
}
export interface LocalReviewAccepted {
  request: LocalReviewStartParams;
  target: {
    kind: "local";
    localSnapshotSha256: string;
  };
  operationId: string;
  state: "accepted";
}
export interface LocalReviewHistoryPage {
  request: LocalReviewHistoryParams;
  /**
   * @maxItems 32
   */
  runs: LocalRunSummary[];
  total: number;
  nextOffset: number | null;
}
export interface LocalRunSummary {
  runId: LocalRunId;
  createdAt: string;
  status: "idle" | "running" | "succeeded" | "failed" | "cancelled";
  turnKind: "initial" | "reply";
  findingsCount: number;
}
export interface LocalReviewFindingsPage {
  request: LocalReviewFindingsParams;
  runId: LocalRunId | null;
  /**
   * @maxItems 32
   */
  findings: LocalReviewFinding[];
  total: number;
  nextOffset: number | null;
}
export interface LocalReviewFinding {
  id: LocalRunId;
  title: string;
  severity: "info" | "low" | "medium" | "high" | "critical";
  summary: string;
  anchor: LocalFindingAnchor | null;
  /**
   * @maxItems 3
   */
  candidateLayers:
    | []
    | ["committed" | "staged" | "unstaged"]
    | ["committed" | "staged" | "unstaged", "committed" | "staged" | "unstaged"]
    | ["committed" | "staged" | "unstaged", "committed" | "staged" | "unstaged", "committed" | "staged" | "unstaged"];
}
export interface LocalFindingAnchor {
  path: LocalRelativePath;
  startLine: number;
  endLine: number | null;
  side: "new" | "old";
}
export interface LocalReviewOutput {
  request: LocalReviewQueryParams;
  runId: LocalRunId;
  text: string;
}
export interface LocalReviewThreads {
  request: LocalReviewQueryParams;
  runId: LocalRunId | null;
  /**
   * @maxItems 12
   */
  threads:
    | []
    | [LocalReviewThread]
    | [LocalReviewThread, LocalReviewThread]
    | [LocalReviewThread, LocalReviewThread, LocalReviewThread]
    | [LocalReviewThread, LocalReviewThread, LocalReviewThread, LocalReviewThread]
    | [LocalReviewThread, LocalReviewThread, LocalReviewThread, LocalReviewThread, LocalReviewThread]
    | [LocalReviewThread, LocalReviewThread, LocalReviewThread, LocalReviewThread, LocalReviewThread, LocalReviewThread]
    | [
        LocalReviewThread,
        LocalReviewThread,
        LocalReviewThread,
        LocalReviewThread,
        LocalReviewThread,
        LocalReviewThread,
        LocalReviewThread
      ]
    | [
        LocalReviewThread,
        LocalReviewThread,
        LocalReviewThread,
        LocalReviewThread,
        LocalReviewThread,
        LocalReviewThread,
        LocalReviewThread,
        LocalReviewThread
      ]
    | [
        LocalReviewThread,
        LocalReviewThread,
        LocalReviewThread,
        LocalReviewThread,
        LocalReviewThread,
        LocalReviewThread,
        LocalReviewThread,
        LocalReviewThread,
        LocalReviewThread
      ]
    | [
        LocalReviewThread,
        LocalReviewThread,
        LocalReviewThread,
        LocalReviewThread,
        LocalReviewThread,
        LocalReviewThread,
        LocalReviewThread,
        LocalReviewThread,
        LocalReviewThread,
        LocalReviewThread
      ]
    | [
        LocalReviewThread,
        LocalReviewThread,
        LocalReviewThread,
        LocalReviewThread,
        LocalReviewThread,
        LocalReviewThread,
        LocalReviewThread,
        LocalReviewThread,
        LocalReviewThread,
        LocalReviewThread,
        LocalReviewThread
      ]
    | [
        LocalReviewThread,
        LocalReviewThread,
        LocalReviewThread,
        LocalReviewThread,
        LocalReviewThread,
        LocalReviewThread,
        LocalReviewThread,
        LocalReviewThread,
        LocalReviewThread,
        LocalReviewThread,
        LocalReviewThread,
        LocalReviewThread
      ];
  truncated: boolean;
}
export interface LocalReviewThread {
  id: LocalRunId;
  title: string;
  latestRunId: LocalRunId;
  selected: boolean;
  /**
   * @maxItems 8
   */
  messages:
    | []
    | [LocalReviewMessage]
    | [LocalReviewMessage, LocalReviewMessage]
    | [LocalReviewMessage, LocalReviewMessage, LocalReviewMessage]
    | [LocalReviewMessage, LocalReviewMessage, LocalReviewMessage, LocalReviewMessage]
    | [LocalReviewMessage, LocalReviewMessage, LocalReviewMessage, LocalReviewMessage, LocalReviewMessage]
    | [
        LocalReviewMessage,
        LocalReviewMessage,
        LocalReviewMessage,
        LocalReviewMessage,
        LocalReviewMessage,
        LocalReviewMessage
      ]
    | [
        LocalReviewMessage,
        LocalReviewMessage,
        LocalReviewMessage,
        LocalReviewMessage,
        LocalReviewMessage,
        LocalReviewMessage,
        LocalReviewMessage
      ]
    | [
        LocalReviewMessage,
        LocalReviewMessage,
        LocalReviewMessage,
        LocalReviewMessage,
        LocalReviewMessage,
        LocalReviewMessage,
        LocalReviewMessage,
        LocalReviewMessage
      ];
  truncated: boolean;
}
export interface LocalReviewMessage {
  id: LocalRunId;
  role: "user" | "assistant";
  content: string;
  truncated: boolean;
}
export interface ResponseError {
  type: "response";
  id: RequestId;
  ok: false;
  error: ProtocolError;
}
export interface Event {
  type: "event";
  operationId: string;
  sequence: number;
  target: TargetIdentity;
  event: OperationEvent;
}

export const PROTOCOL_VERSION = 1;
export const MAX_FRAME_BYTES = 1048576;
