import type {
  AuthProviderParams,
  CommentsListResult,
  DraftPreparation,
  DraftPrepareParams,
  DraftStageParams,
  DraftStageResult,
  DraftTarget,
  LocalBrowserParams,
  LocalBrowserResult,
  LocalCancelParams,
  LocalCancelResult,
  LocalCaptureParams,
  LocalCaptureResult,
  LocalComparison,
  LocalDiffPage,
  LocalDiffParams,
  LocalFilesPage,
  LocalFilesParams,
  LocalImagePage,
  LocalImageParams,
  LocalInfoResult,
  LocalListPage,
  LocalListParams,
  LocalLookupParams,
  LocalOptionsParams,
  LocalOptionsResult,
  LocalRecheckParams,
  LocalRecheckResult,
  LocalReviewAccepted,
  LocalReviewFindingsPage,
  LocalReviewFindingsParams,
  LocalReviewHistoryPage,
  LocalReviewHistoryParams,
  LocalReviewOutput,
  LocalReviewProfilesResult,
  LocalReviewQueryParams,
  LocalReviewStartParams,
  LocalReviewThreads,
  LocalSnapshotInfo,
  LocalSnapshotRef,
  ProviderApprovalConfirmParams,
  ProviderApprovalPreview,
  ProviderApprovalProof,
  ProviderApprovalResult,
  ProviderBrowserResult,
  ProviderCurrentUser,
  ProviderPrDetail,
  ProviderPrDetailParams,
  ProviderPrListParams,
  ProviderPrPage,
  ProviderSnapshotBrowserResult,
  ProviderSnapshotDiffPage,
  ProviderSnapshotDiffParams,
  ProviderSnapshotFile,
  ProviderSnapshotFilesPage,
  ProviderSnapshotFilesParams,
  ProviderSnapshotImagePage,
  ProviderSnapshotImageParams,
  ProviderSnapshotInfo,
  ProviderSnapshotRef,
  PrRepositoryIdentity,
  PublicationItemResult,
  PublicationPrepareResult,
  PublicationScope,
  PublicationStatusResult,
  RecordMutation,
  RecordMutationResult,
  RecordPage,
  RepositoryEditor,
  RepositoryEditorOpened,
  RepositoryEditorOpenParams,
  RepositoryEditorPrepared,
  RepositoryEditorPrepareParams,
  RepositoryEditorStatus,
  RepositoryEditorStatusParams,
  RepositoryExploreBlamePage,
  RepositoryExploreBlameParams,
  RepositoryExploreContentPage,
  RepositoryExploreContentParams,
  RepositoryExploreDiffPage,
  RepositoryExploreDiffParams,
  RepositoryExploreFilesPage,
  RepositoryExploreFilesParams,
  RepositoryExploreFindPage,
  RepositoryExploreFindParams,
  RepositoryExploreInfo,
  RepositoryExploreInfoParams,
  RepositoryOperation,
  RepositoryOperationApplied,
  RepositoryOperationApplyParams,
  RepositoryOperationListParams,
  RepositoryOperationPage,
  RepositoryOperationPrepared,
  RepositoryOperationPreviewParams,
  RepositoryOperationStatus,
  RepositoryOperationStatusParams,
  ReviewProfileCatalog,
  ScopePage,
  WorkspaceReadiness,
  WorkspaceScope,
  WorkspaceSettings,
} from "../../../src/protocol/generated/protocol";
import { validEditorParams, validEditorResult } from "./editorValidation";
import { validExploreParams, validExploreResult } from "./explorationValidation";
import {
  localPath,
  sameLocalValue,
  validLocalComparison,
  validLocalInfo,
  validLocalRef,
} from "./localValidation";
import { validOperationParams, validOperationResult } from "./operationsValidation";
import { samePublicationScope, validPublicationDTO } from "./publicationValidation";

// Typed facade over a BackendTransport. Domain methods are added as later
// migration steps land; the first slice exposes repository listing and diffs.

import type { TargetIdentity } from "./protocol";
import type { BackendExit, BackendOperationEvent, BackendTransport } from "./types";
import { validDraft } from "./workspaceValidation";

function sameDraftTarget(value: unknown, expected: DraftTarget): value is DraftTarget {
  if (!value || typeof value !== "object" || Array.isArray(value)) return false;
  const actual = value as DraftTarget;
  const fields = ["kind", "provider", "workspace", "repo", "prId", "runId"] as const;
  return (
    Object.keys(value).length === fields.length &&
    fields.every((field) => actual[field] === expected[field])
  );
}

export interface RepositorySummary {
  provider: "github" | "bitbucket";
  workspace: string;
  repo: string;
  localPath: string | null;
}
export type { WorkspaceReadiness };

export interface DiffFileResult {
  target: TargetIdentity;
  path: string;
  diff: string;
  truncated: boolean;
}

export type ChangedFileStatus =
  | "added"
  | "modified"
  | "deleted"
  | "renamed"
  | "copied"
  | "untracked";

export interface ChangedFile {
  localLayer?: "committed" | "staged" | "unstaged";
  binary?: boolean;
  path: string;
  status: ChangedFileStatus;
  additions: number;
  deletions: number;
  oldPath: string | null;
}

export interface FindingAnchor {
  path: string;
  startLine: number;
  endLine: number | null;
  side: "new" | "old";
}

export interface FindingSummary {
  candidateLayers?: Array<"committed" | "staged" | "unstaged">;
  id: string;
  title: string;
  severity: "info" | "low" | "medium" | "high" | "critical";
  summary: string;
  anchor: FindingAnchor | null;
}

export interface ReviewRunSummary {
  runId: string;
  createdAt: string;
  status: string;
  turnKind: string;
  findingsCount: number;
}

export interface ReviewThreadSummary {
  id: string;
  title: string;
  latestRunId: string | null;
  selected?: boolean;
  messages: Array<{ id: string; role: "user" | "assistant"; content: string; truncated: boolean }>;
  truncated: boolean;
}

export interface ReviewTarget {
  provider: "github" | "bitbucket";
  workspace: string;
  repo: string;
  prId: number;
  runId: string;
  title: string;
  status: string;
}

export interface OperationStatus {
  operationId: string;
  state: string;
  sequence: number;
  error: { code: string; message: string; retryable?: boolean } | null;
}

export interface FixPreview {
  action?: "fix" | "commit" | "push" | "sync" | "conflicts";
  commitMessage?: string | null;
  filesToCommit?: string[];
  commitSha?: string | null;
  target: TargetIdentity;
  approvalToken: string;
  repoPath: string;
  currentBranch: string;
  sourceBranch: string;
  destinationBranch: string;
  dirtyFiles: string[];
  dirtyFileCount?: number;
  dirtyFilesTruncated?: boolean;
  effects: string[];
  snapshotSha256: string;
  expiresInSeconds: number;
}

export interface FixState {
  commitSha?: string | null;
  status: string;
  phase: string;
  summary: string | null;
  suggestedCommitMessage: string | null;
  filesTouched: string[];
  tests: string[];
}

export interface SettingsSnapshot {
  version: string;
  settings: WorkspaceSettings;
}
export interface CredentialStatus {
  provider: AuthProviderParams["provider"];
  available: boolean;
  source: "keychain" | "environment" | "none";
}

export interface FilePreview {
  unavailableReason?: LocalImagePage["reason"];
  path: string;
  side: "old" | "new";
  available: boolean;
  mimeType: string;
  size: number;
  dataBase64: string;
  width: number | null;
  height: number | null;
  contentSha256: string | null;
}
export interface DataImportPreview {
  inputVersion: string;
  destinationVersion: string;
  additionsCount: number;
  unchangedCount: number;
  conflictsCount: number;
  rejectedCount: number;
  detailsTruncated: boolean;
  additions: string[];
  unchanged: string[];
  conflicts: string[];
  rejected: Array<{ entry: number; reason: string }>;
}
export interface DataImportResult {
  imported: number;
  unchanged: number;
  conflicts: number;
  rejectedCount: number;
  detailsTruncated: boolean;
  rejected: Array<{ entry: number; reason: string }>;
  backupPath: string | null;
  destinationVersion: string;
}
function snapshotPath(path: string): boolean {
  return (
    path.length > 0 &&
    Buffer.byteLength(path) <= 4096 &&
    !path.startsWith("/") &&
    !path.includes("\\") &&
    !/^[a-z]:/i.test(path) &&
    !/\p{Cc}/u.test(path) &&
    !path.split("/").some((part) => !part || part === "." || part === "..")
  );
}
export class BackendClient {
  private readonly reviewSources = new Map<string, ProviderSnapshotInfo>();
  private reviewSourceKey(target: TargetIdentity): string {
    return JSON.stringify(
      target.kind === "provider"
        ? [
            target.kind,
            target.provider,
            target.workspace,
            target.repo,
            target.prId ?? null,
            target.runId ?? null,
          ]
        : [target.kind, target.localSnapshotSha256],
    );
  }
  reviewSourceSnapshot(target: TargetIdentity): ProviderSnapshotInfo | null {
    return this.reviewSources.get(this.reviewSourceKey(target)) ?? null;
  }

  private async providerRead<T>(
    method: string,
    params: Record<string, unknown>,
    requestType: string,
    resultType: string,
    current: () => boolean,
  ): Promise<T> {
    if (!current()) throw new Error("Provider selection changed.");
    if (!validPublicationDTO(requestType, params))
      throw new Error("Invalid provider read parameters.");
    const result = await this.transport.request(method, { ...params });
    if (!current()) throw new Error("Provider selection changed.");
    if (
      Buffer.byteLength(JSON.stringify(result)) > 512 * 1024 ||
      !validPublicationDTO(resultType, result)
    )
      throw new Error("Invalid or oversized provider PR response.");
    const echoed = resultType === "ProviderCurrentUser" ? result.repository : result.request;
    const expected = resultType === "ProviderCurrentUser" ? params.repository : params;
    const canonical = (value: unknown): string => {
      if (!value || typeof value !== "object" || Array.isArray(value)) return JSON.stringify(value);
      const object = value as Record<string, unknown>;
      return JSON.stringify(
        Object.keys(object)
          .sort()
          .map((key) => [key, canonical(object[key])]),
      );
    };
    if (canonical(echoed) !== canonical(expected))
      throw new Error("Provider response belongs to a different selection.");
    return result as unknown as T;
  }
  providerPrs(
    params: ProviderPrListParams,
    current: () => boolean = () => true,
  ): Promise<ProviderPrPage> {
    const normalized = {
      ...params,
      query: params.query ?? null,
      authorId: params.authorId ?? null,
    };
    return this.providerRead<ProviderPrPage>(
      "provider.prs",
      normalized,
      "ProviderPrListParams",
      "ProviderPrPage",
      current,
    ).then((page) => {
      if (
        (page.nextPage !== null && page.nextPage !== params.page + 1) ||
        new Set(page.values.map((pr) => pr.id)).size !== page.values.length ||
        page.values.length > params.limit
      )
        throw new Error("Invalid provider PR page continuation.");
      return page;
    });
  }
  private async approvalRequest<T extends ProviderApprovalPreview | ProviderApprovalResult>(
    method: string,
    params: ProviderPrDetailParams | ProviderApprovalConfirmParams,
    resultType: string,
    current: () => boolean,
  ): Promise<T> {
    if (!current()) throw new Error("Provider selection changed.");
    const confirm = "approvalId" in params;
    if (
      !validPublicationDTO(
        confirm ? "ProviderApprovalConfirmParams" : "ProviderPrDetailParams",
        params,
      )
    )
      throw new Error("Invalid approval parameters.");
    const value = await this.transport.request(method, { ...params });
    if (!current())
      throw new Error("Provider selection changed. Check approval status before sending again.");
    if (
      Buffer.byteLength(JSON.stringify(value)) > 128 * 1024 ||
      !validPublicationDTO(resultType, value)
    )
      throw new Error("Invalid approval response. Check provider status before sending again.");
    const result = value as unknown as T;
    const expected = confirm ? params.request : params;
    const same = (request: ProviderPrDetailParams) =>
      request.prId === expected.prId &&
      request.repository.provider === expected.repository.provider &&
      request.repository.workspace === expected.repository.workspace &&
      request.repository.repo === expected.repository.repo;
    if (
      !same(result.request) ||
      !same(result.proof.request) ||
      result.proof.headSha.length !== result.proof.destinationSha.length ||
      (result.proof.sourceRepository &&
        result.proof.sourceRepository.provider !== expected.repository.provider) ||
      (result.entry &&
        (!same(result.entry.proof.request) ||
          result.entry.proof.repositoryId !== result.proof.repositoryId ||
          result.entry.proof.actor.accountId !== result.proof.actor.accountId))
    )
      throw new Error("Approval response belongs to another PR or reviewer.");
    if (
      "approvalId" in result &&
      ((result.approvalId === null) !== (result.expiresAt === null) ||
        (result.approvalId !== null &&
          (result.proof.state !== "OPEN" ||
            result.proof.draft ||
            result.proof.alreadyApproved ||
            (result.entry !== null && result.entry.state !== "rejected"))))
    )
      throw new Error("Approval preview cannot be confirmed.");
    return result;
  }
  providerApprovalPreview(params: ProviderPrDetailParams, current: () => boolean = () => true) {
    return this.approvalRequest<ProviderApprovalPreview>(
      "provider.approval.preview",
      params,
      "ProviderApprovalPreview",
      current,
    );
  }
  providerApprovalConfirm(
    params: ProviderApprovalConfirmParams,
    expected: ProviderApprovalProof,
    current: () => boolean = () => true,
  ) {
    if (!validPublicationDTO("ProviderApprovalProof", expected))
      return Promise.reject(new Error("Invalid approval preview."));
    const signature = (proof: ProviderApprovalProof) =>
      JSON.stringify([
        proof.request.repository.provider,
        proof.request.repository.workspace,
        proof.request.repository.repo,
        proof.request.prId,
        proof.repositoryId,
        proof.actor.accountId,
        proof.actor.displayName,
        proof.title,
        proof.headSha,
        proof.destinationSha,
        proof.sourceBranch,
        proof.destinationBranch,
        proof.sourceRepository?.provider ?? null,
        proof.sourceRepository?.workspace ?? null,
        proof.sourceRepository?.repo ?? null,
        proof.state,
        proof.draft,
      ]);
    return this.approvalRequest<ProviderApprovalResult>(
      "provider.approval.confirm",
      params,
      "ProviderApprovalResult",
      current,
    ).then((result) => {
      if (signature(result.proof) !== signature(expected))
        throw new Error(
          "Approval receipt does not match its preview. Check provider status before sending again.",
        );
      return result;
    });
  }
  providerApprovalStatus(
    params: ProviderPrDetailParams,
    reconcile = false,
    current: () => boolean = () => true,
  ) {
    return this.approvalRequest<ProviderApprovalResult>(
      reconcile ? "provider.approval.reconcile" : "provider.approval.status",
      params,
      "ProviderApprovalResult",
      current,
    );
  }

  providerBrowser(
    params: ProviderPrDetailParams,
    current: () => boolean = () => true,
  ): Promise<ProviderBrowserResult> {
    return this.providerRead<ProviderBrowserResult>(
      "provider.browser",
      { ...params },
      "ProviderPrDetailParams",
      "ProviderBrowserResult",
      current,
    ).then((result) => {
      const resolved = result.repository;
      const original = params.repository;
      const uuid = (value: string) => value.startsWith("{");
      if (
        resolved.provider !== original.provider ||
        uuid(resolved.workspace) ||
        uuid(resolved.repo) ||
        (!uuid(original.workspace) &&
          original.workspace.toLowerCase() !== resolved.workspace.toLowerCase()) ||
        (!uuid(original.repo) && original.repo.toLowerCase() !== resolved.repo.toLowerCase())
      )
        throw new Error("Provider browser identity could not be verified.");
      const expected = `https://${resolved.provider === "github" ? "github.com" : "bitbucket.org"}/${encodeURIComponent(resolved.workspace)}/${encodeURIComponent(resolved.repo)}/${resolved.provider === "github" ? "pull" : "pull-requests"}/${params.prId}`;
      if (result.url !== expected || (!result.launchRequested && result.launchError !== null))
        throw new Error("Provider browser launch response could not be verified.");
      return result;
    });
  }

  providerPrDetail(
    params: ProviderPrDetailParams,
    current: () => boolean = () => true,
  ): Promise<ProviderPrDetail> {
    return this.providerRead<ProviderPrDetail>(
      "provider.pr.detail",
      { ...params },
      "ProviderPrDetailParams",
      "ProviderPrDetail",
      current,
    ).then((result) => {
      if (result.detail.id !== params.prId)
        throw new Error("Provider detail belongs to a different PR.");
      const repo = params.repository;
      const expected = `https://${repo.provider === "github" ? "github.com" : "bitbucket.org"}/${encodeURIComponent(repo.workspace)}/${encodeURIComponent(repo.repo)}/${repo.provider === "github" ? "pull" : "pull-requests"}/${params.prId}`;
      if (result.browserUrl !== expected)
        throw new Error("Provider browser link belongs to a different PR.");
      return result;
    });
  }
  providerUser(
    repository: PrRepositoryIdentity,
    current: () => boolean = () => true,
  ): Promise<ProviderCurrentUser> {
    return this.providerRead(
      "provider.user",
      { repository },
      "ProviderUserParams",
      "ProviderCurrentUser",
      current,
    );
  }
  providerSnapshot(
    params: ProviderPrDetailParams,
    current: () => boolean = () => true,
  ): Promise<ProviderSnapshotInfo> {
    return this.providerRead<ProviderSnapshotInfo>(
      "provider.snapshot",
      { ...params },
      "ProviderPrDetailParams",
      "ProviderSnapshotInfo",
      current,
    ).then((info) => {
      const actual = info.snapshot;
      if (
        actual.prId !== params.prId ||
        actual.repository.provider !== params.repository.provider ||
        actual.repository.workspace !== params.repository.workspace ||
        actual.repository.repo !== params.repository.repo ||
        info.headSha.length !== info.baseSha.length ||
        info.headSha.length !== info.destinationSha.length ||
        (info.fileCount === 0) !== (info.patchBytes === 0)
      )
        throw new Error("Provider snapshot belongs to a different PR or range.");
      return info;
    });
  }
  providerSnapshotFiles(
    params: ProviderSnapshotFilesParams,
    current: () => boolean = () => true,
  ): Promise<ProviderSnapshotFilesPage> {
    return this.providerRead<ProviderSnapshotFilesPage>(
      "provider.snapshot.files",
      { ...params },
      "ProviderSnapshotFilesParams",
      "ProviderSnapshotFilesPage",
      current,
    ).then((page) => {
      const end = params.offset + page.files.length;
      if (
        page.files.length > params.limit ||
        end > page.total ||
        page.nextOffset !== (end < page.total ? end : null) ||
        (page.nextOffset !== null && end <= params.offset) ||
        new Set(page.files.map((file) => file.path)).size !== page.files.length ||
        page.files.some(
          (file) =>
            !snapshotPath(file.path) ||
            (file.oldPath !== null && (!snapshotPath(file.oldPath) || file.oldPath === file.path)),
        )
      )
        throw new Error("Invalid provider snapshot inventory continuation.");
      return page;
    });
  }
  providerSnapshotDiff(
    params: ProviderSnapshotDiffParams,
    current: () => boolean = () => true,
  ): Promise<ProviderSnapshotDiffPage> {
    if (!snapshotPath(params.path))
      return Promise.reject(new Error("Invalid provider snapshot path."));
    return this.providerRead<ProviderSnapshotDiffPage>(
      "provider.snapshot.diff",
      { ...params },
      "ProviderSnapshotDiffParams",
      "ProviderSnapshotDiffPage",
      current,
    ).then((page) => {
      const bytes = Buffer.byteLength(page.text);
      const end = params.offset + bytes;
      if (
        bytes > 65536 ||
        end > page.totalBytes ||
        page.nextOffset !== (end < page.totalBytes ? end : null) ||
        (page.nextOffset !== null && bytes === 0) ||
        Buffer.from(page.text).toString("utf8") !== page.text
      )
        throw new Error("Invalid provider snapshot diff continuation.");
      return page;
    });
  }
  async providerSnapshotInventory(
    info: ProviderSnapshotInfo,
    current: () => boolean = () => true,
  ): Promise<ProviderSnapshotFile[]> {
    const files: ProviderSnapshotFile[] = [];
    const seen = new Set<string>();
    let offset = 0;
    let bytes = 0;
    do {
      const page = await this.providerSnapshotFiles(
        { snapshot: info.snapshot, offset, limit: 100 },
        current,
      );
      if (page.total !== info.fileCount) throw new Error("Provider snapshot inventory changed.");
      for (const file of page.files) {
        bytes += Buffer.byteLength(JSON.stringify(file));
        if (seen.has(file.path) || files.length >= 10000 || bytes > 8 * 1024 * 1024)
          throw new Error("Invalid or oversized provider snapshot inventory.");
        seen.add(file.path);
        files.push(file);
      }
      if (page.nextOffset === null) break;
      offset = page.nextOffset;
    } while (offset < info.fileCount);
    return files;
  }
  async providerSnapshotPatch(
    snapshot: ProviderSnapshotRef,
    path: string,
    current: () => boolean = () => true,
  ): Promise<string> {
    const chunks: string[] = [];
    let offset = 0;
    let total: number | null = null;
    let hash: string | null = null;
    do {
      const page = await this.providerSnapshotDiff({ snapshot, path, offset }, current);
      if (
        (total !== null && total !== page.totalBytes) ||
        (hash !== null && hash !== page.contentSha256) ||
        chunks.length >= 130
      )
        throw new Error("Provider snapshot diff changed.");
      total = page.totalBytes;
      hash = page.contentSha256;
      chunks.push(page.text);
      if (page.nextOffset === null) break;
      offset = page.nextOffset;
    } while (total !== null && offset < total);
    const patch = chunks.join("");
    const actual = await crypto.subtle.digest("SHA-256", new TextEncoder().encode(patch));
    if (!current()) throw new Error("Provider selection changed.");
    if (Buffer.byteLength(patch) !== total || Buffer.from(actual).toString("hex") !== hash)
      throw new Error("Provider snapshot diff hash mismatch.");
    return patch;
  }
  async providerSnapshotBrowser(
    snapshot: ProviderSnapshotRef,
    current: () => boolean = () => true,
  ): Promise<ProviderSnapshotBrowserResult> {
    const result = await this.providerRead<ProviderSnapshotBrowserResult>(
      "provider.snapshot.browser",
      { snapshot },
      "ProviderSnapshotBrowserParams",
      "ProviderSnapshotBrowserResult",
      current,
    );
    const url = new URL(result.url);
    if (
      url.port === "" ||
      Number(url.port) > 65535 ||
      (!result.launchRequested && result.launchError !== null)
    )
      throw new Error("Invalid immutable browser launch response.");
    return result;
  }
  async providerSnapshotImage(
    snapshot: ProviderSnapshotRef,
    path: string,
    side: "old" | "new",
    current: () => boolean = () => true,
  ): Promise<FilePreview> {
    if (!snapshotPath(path)) throw new Error("Invalid provider snapshot path.");
    return this.loadRaster(path, side, current, async (offset, contentSha256) => {
      const params: ProviderSnapshotImageParams = {
        snapshot,
        path,
        side,
        offset,
        maxBytes: 192 * 1024,
        contentSha256,
      };
      const page = await this.providerRead<ProviderSnapshotImagePage>(
        "provider.snapshot.image",
        params,
        "ProviderSnapshotImageParams",
        "ProviderSnapshotImagePage",
        current,
      );
      if (
        (!page.available &&
          (page.size !== 0 ||
            page.dataBase64 !== "" ||
            page.mimeType !== "" ||
            page.nextOffset !== null ||
            page.contentSha256 !== null ||
            page.width !== null ||
            page.height !== null)) ||
        (page.available &&
          !["image/png", "image/jpeg", "image/gif", "image/webp", "image/bmp"].includes(
            page.mimeType,
          ))
      )
        throw new Error("Invalid immutable image availability or format.");
      return { ...page, path, side, offset };
    });
  }
  async workspaceScopes(current: () => boolean = () => true): Promise<WorkspaceScope[]> {
    const scopes: WorkspaceScope[] = [];
    const seen = new Set<string>();
    let first: ScopePage | undefined;
    let offset = 0;
    let bytes = 0;
    for (let pages = 0; pages <= 100; pages++) {
      if (!current()) throw new Error("Workspace selection changed.");
      const result = await this.transport.request("workspace.scopes", {
        offset,
        limit: 100,
        ...(first ? { inventoryVersion: first.inventoryVersion } : {}),
      });
      if (!current()) throw new Error("Workspace selection changed.");
      const size = Buffer.byteLength(JSON.stringify(result));
      bytes += size;
      if (!validPublicationDTO("ScopePage", result) || size > 256 * 1024 || bytes > 8 * 1024 * 1024)
        throw new Error("Invalid or oversized saved-scope page.");
      const page = result as unknown as ScopePage;
      if (
        page.offset !== offset ||
        (first &&
          (page.inventoryVersion !== first.inventoryVersion ||
            page.totalScopes !== first.totalScopes))
      )
        throw new Error("Saved-scope inventory changed; refresh the picker.");
      first ??= page;
      for (const scope of page.scopes) {
        const key = JSON.stringify([scope.provider, scope.workspace, scope.repo, scope.prId]);
        if (
          seen.has(key) ||
          scope.localOnly !== scope.prId >= 0x80000000 ||
          scope.draftCount + scope.referenceCount + scope.viewedCount === 0
        )
          throw new Error("Invalid or duplicate saved scope.");
        seen.add(key);
        scopes.push(scope);
      }
      offset += page.scopes.length;
      if (offset > page.totalScopes) throw new Error("Saved-scope inventory exceeds its total.");
      if (page.nextOffset === null) {
        if (offset !== page.totalScopes) throw new Error("Saved-scope inventory ended early.");
        return scopes;
      }
      if (page.nextOffset !== offset || page.scopes.length === 0)
        throw new Error("Saved-scope page did not advance.");
    }
    throw new Error("Saved-scope inventory requires too many pages.");
  }
  constructor(private readonly transport: BackendTransport) {}

  async readWorkspace(key: string, current: () => boolean = () => true): Promise<RecordPage> {
    let first: RecordPage | null = null;
    let offset = 0;
    let bytes = 0;
    const values: unknown[] = [];
    const kind = key === "theme" ? "theme" : key.split(".")[0];
    for (let index = 0; index < 10001; index++) {
      if (!current()) throw new Error("Workspace selection changed.");
      const result = await this.transport.request("workspace.read", {
        key,
        offset,
        limit: 100,
        ...(first ? { recordVersion: first.recordVersion } : {}),
      });
      if (!current()) throw new Error("Workspace selection changed.");
      const page = result as unknown as RecordPage;
      const size = Buffer.byteLength(JSON.stringify(result));
      bytes += size;
      if (
        size > 256 * 1024 ||
        bytes > 20 * 1024 * 1024 ||
        page.key !== key ||
        !/^[a-f0-9]{64}$/.test(page.recordVersion ?? "") ||
        typeof page.available !== "boolean" ||
        page.offset !== offset ||
        !Number.isInteger(page.totalItems) ||
        page.totalItems < 0 ||
        page.totalItems > 10000 ||
        page.payload?.kind !== kind ||
        (first &&
          (page.recordVersion !== first.recordVersion ||
            page.totalItems !== first.totalItems ||
            page.available !== first.available))
      )
        throw new Error("Invalid or inconsistent workspace page.");
      first ??= page;
      if (Array.isArray(page.payload.value)) {
        const items = page.payload.value;
        if (
          !["drafts", "references", "viewed"].includes(kind ?? "") ||
          items.length > 100 ||
          offset + items.length > page.totalItems ||
          (kind === "viewed" &&
            items.some(
              (item) =>
                typeof item !== "string" || Buffer.byteLength(item) > 4096 || /\p{Cc}/u.test(item),
            ))
        )
          throw new Error("Invalid workspace page length.");
        values.push(...items);
        offset += items.length;
      } else {
        if (
          !["prompt", "theme"].includes(kind ?? "") ||
          typeof page.payload.value !== "string" ||
          offset !== 0 ||
          page.nextOffset !== null ||
          Buffer.byteLength(page.payload.value) > 65536 ||
          (kind === "theme" && !["dark", "light"].includes(page.payload.value))
        )
          throw new Error("Invalid workspace scalar.");
        return page;
      }
      if (page.nextOffset === null) {
        if (offset !== page.totalItems) throw new Error("Workspace page ended early.");
        return {
          ...first,
          nextOffset: null,
          payload: { kind, value: values } as RecordPage["payload"],
        };
      }
      if (page.nextOffset !== offset || page.offset === offset)
        throw new Error("Workspace page did not advance.");
    }
    throw new Error("Workspace record requires too many pages.");
  }

  async prepareFindingDraft(params: DraftPrepareParams): Promise<DraftPreparation> {
    const result = await this.transport.request(
      "draft.prepare",
      params as unknown as Record<string, unknown>,
    );
    const expectedKey = `drafts.${params.target.provider}:${params.target.workspace}/${params.target.repo}/${params.target.prId}`;
    if (
      !result ||
      typeof result !== "object" ||
      Array.isArray(result) ||
      Object.keys(result).length !== 6 ||
      !sameDraftTarget(result.target, params.target) ||
      result.key !== expectedKey ||
      typeof result.recordVersion !== "string" ||
      !/^[a-f0-9]{64}$/.test(result.recordVersion) ||
      typeof result.previewHash !== "string" ||
      !/^[a-f0-9]{64}$/.test(result.previewHash) ||
      typeof result.alreadyStaged !== "boolean" ||
      !validDraft(result.draft) ||
      result.draft.prId !== params.target.prId ||
      result.draft.findingRef?.reviewRunId !== params.target.runId ||
      result.draft.findingRef.findingId !== params.findingId ||
      !result.draft.findingRef.findingFingerprint ||
      (!result.alreadyStaged && result.draft.publicationMode !== params.publicationMode)
    )
      throw new Error("Invalid finding draft preview. No staging was requested.");
    return result as unknown as DraftPreparation;
  }
  private async publicationRequest<T>(
    method: string,
    params: Record<string, unknown>,
    requestType: string,
    resultType: string,
    scope?: PublicationScope,
  ): Promise<T> {
    if (!validPublicationDTO(requestType, params)) throw new Error("Invalid publication request.");
    const result = await this.transport.request(method, { ...params });
    if (
      !validPublicationDTO(resultType, result) ||
      (scope && !samePublicationScope(result.scope as PublicationScope, scope))
    )
      throw new Error("Invalid publication response; inspect status before sending again.");
    const comments =
      resultType === "PublicationPrepareResult"
        ? (result as unknown as PublicationPrepareResult).items.map((item) => item.comment)
        : resultType === "PublicationStatusResult"
          ? (result as unknown as PublicationStatusResult).entries.map((entry) => entry.comment)
          : resultType === "PublicationItemResult"
            ? [(result as unknown as PublicationItemResult).entry.comment]
            : [];
    if (scope && comments.some((comment) => !samePublicationScope(comment.scope, scope)))
      throw new Error("Publication entry belongs to another scope.");
    if (resultType === "PublicationPrepareResult") {
      const preview = result as unknown as PublicationPrepareResult;
      const ids = params.localIds as string[];
      const selectedIds = new Set(ids);
      if (
        preview.key !==
          `drafts.${scope!.provider}:${scope!.workspace}/${scope!.repo}/${scope!.prId}` ||
        preview.items.length !== ids.length ||
        new Set(comments.map((comment) => comment.localId)).size !== ids.length ||
        comments.some((comment) => !selectedIds.has(comment.localId))
      )
        throw new Error("Publication preview does not match the selected drafts.");
      for (const item of preview.items) {
        const comment = item.comment;
        const key = new Bun.CryptoHasher("sha256")
          .update(
            JSON.stringify([
              comment.scope.provider,
              comment.scope.workspace.replace(/[A-Z]/g, (letter) => letter.toLowerCase()),
              comment.scope.repo.replace(/[A-Z]/g, (letter) => letter.toLowerCase()),
              comment.scope.prId,
              comment.localId,
            ]),
          )
          .digest("hex");
        if (item.markdown !== `${comment.raw}\n\n<!-- norn-draft:${key} -->`)
          throw new Error("Publication preview does not match its approved body.");
      }
    }
    if (resultType === "PublicationItemResult") {
      const item = result as unknown as PublicationItemResult;
      if (
        item.entry.comment.localId !== params.localId ||
        (item.draftRemoved && (!item.localSaved || item.entry.state !== "published")) ||
        ["published", "publishedUnverified", "publishedSnapshotChanged"].includes(
          item.entry.state,
        ) !==
          (item.entry.remoteId !== null)
      )
        throw new Error("Publication result is inconsistent. Inspect status before sending again.");
    }
    return result as T;
  }
  preparePublication(scope: PublicationScope, localIds: string[]) {
    return this.publicationRequest<PublicationPrepareResult>(
      "publication.prepare",
      { scope, localIds },
      "PublicationPrepareParams",
      "PublicationPrepareResult",
      scope,
    );
  }
  applyPublication(scope: PublicationScope, approvalId: string, localId: string) {
    return this.publicationRequest<PublicationItemResult>(
      "publication.apply",
      { scope, approvalId, localId },
      "PublicationApplyParams",
      "PublicationItemResult",
      scope,
    );
  }
  cancelPublication(scope: PublicationScope, approvalId: string) {
    return this.publicationRequest<{ canceled: boolean }>(
      "publication.cancel",
      { scope, approvalId },
      "PublicationCancelParams",
      "PublicationCancelResult",
    );
  }
  publicationStatus(scope: PublicationScope, after: string | null = null, limit = 20) {
    return this.publicationRequest<PublicationStatusResult>(
      "publication.status",
      { scope, after, limit },
      "PublicationStatusParams",
      "PublicationStatusResult",
      scope,
    );
  }
  reconcilePublication(scope: PublicationScope, localId: string) {
    return this.publicationRequest<PublicationItemResult>(
      "publication.reconcile",
      { scope, localId },
      "PublicationReconcileParams",
      "PublicationItemResult",
      scope,
    );
  }
  remoteComments(scope: PublicationScope, offset = 0, version: string | null = null) {
    return this.publicationRequest<CommentsListResult>(
      "comments.list",
      { scope, offset, limit: 50, version },
      "CommentsListParams",
      "CommentsListResult",
      scope,
    );
  }

  async stageFindingDraft(params: DraftStageParams): Promise<DraftStageResult> {
    const result = await this.transport.request(
      "draft.stage",
      params as unknown as Record<string, unknown>,
    );
    const expectedKey = `drafts.${params.target.provider}:${params.target.workspace}/${params.target.repo}/${params.target.prId}`;
    if (
      !result ||
      typeof result !== "object" ||
      Array.isArray(result) ||
      Object.keys(result).length !== 6 ||
      !sameDraftTarget(result.target, params.target) ||
      result.key !== expectedKey ||
      typeof result.recordVersion !== "string" ||
      !/^[a-f0-9]{64}$/.test(result.recordVersion) ||
      typeof result.localId !== "string" ||
      !result.localId ||
      Buffer.byteLength(result.localId) > 256 ||
      /\p{Cc}/u.test(result.localId) ||
      typeof result.changed !== "boolean" ||
      typeof result.alreadyStaged !== "boolean" ||
      result.changed === result.alreadyStaged
    )
      throw new Error(
        "Draft staging acknowledgement is invalid. Inspect saved drafts before retrying.",
      );
    return result as unknown as DraftStageResult;
  }

  async mutateWorkspace(
    key: string,
    expectedVersion: string,
    mutation: RecordMutation,
  ): Promise<RecordMutationResult> {
    const result = await this.transport.request("workspace.mutate", {
      key,
      expectedVersion,
      mutation,
    });
    if (
      result.key !== key ||
      typeof result.changed !== "boolean" ||
      typeof result.recordVersion !== "string" ||
      !/^[a-f0-9]{64}$/.test(result.recordVersion)
    )
      throw new Error("Invalid workspace mutation acknowledgement; reload before retrying.");
    return result as unknown as RecordMutationResult;
  }

  start() {
    return this.transport.start();
  }

  async previewData(
    path: string,
    provider: "github" | "bitbucket" | null,
  ): Promise<DataImportPreview> {
    return (await this.transport.request("data.preview", {
      path,
      provider,
    })) as unknown as DataImportPreview;
  }
  async importData(
    path: string,
    provider: "github" | "bitbucket" | null,
    preview: DataImportPreview,
  ): Promise<DataImportResult> {
    return (await this.transport.request("data.import", {
      path,
      provider,
      inputVersion: preview.inputVersion,
      destinationVersion: preview.destinationVersion,
      confirm: true,
    })) as unknown as DataImportResult;
  }
  async exportData(path: string): Promise<number> {
    return (await this.transport.request("data.export", { path })).exported as number;
  }
  async settings(): Promise<SettingsSnapshot> {
    return (await this.transport.request("settings.get", {})) as unknown as SettingsSnapshot;
  }
  async saveSettings(
    expectedVersion: string,
    settings: WorkspaceSettings,
  ): Promise<SettingsSnapshot> {
    return (await this.transport.request("settings.save", {
      expectedVersion,
      settings,
    })) as unknown as SettingsSnapshot;
  }
  async credentialStatus(): Promise<CredentialStatus[]> {
    const result = await this.transport.request("auth.status", {});
    return result.credentials as unknown as CredentialStatus[];
  }
  async setCredential(
    provider: CredentialStatus["provider"],
    username: string | null,
    token: string,
  ): Promise<void> {
    await this.transport.request("auth.set", { provider, username, token });
  }
  async clearCredential(provider: CredentialStatus["provider"]): Promise<void> {
    await this.transport.request("auth.clear", { provider });
  }

  async repositories(): Promise<RepositorySummary[]> {
    const result = await this.transport.request("repository.status", {});
    return (result.repos as RepositorySummary[]) ?? [];
  }

  async diffFile(
    target: TargetIdentity,
    path: string,
    options: { oldPath?: string | null; contextLines?: number } = {},
  ): Promise<DiffFileResult> {
    const result = await this.transport.request("diff.file", {
      target,
      path,
      ...(options.oldPath ? { oldPath: options.oldPath } : {}),
      ...(options.contextLines === undefined ? {} : { contextLines: options.contextLines }),
    });
    return result as unknown as DiffFileResult;
  }

  async reviewFiles(
    target: TargetIdentity,
    current: () => boolean = () => true,
  ): Promise<ChangedFile[]> {
    if (!current()) throw new Error("Review selection changed.");
    const result = await this.transport.request("review.files", { target });
    if (!current()) throw new Error("Review selection changed.");
    if (!validPublicationDTO("ReviewFilesResult", result))
      throw new Error("Invalid saved review file response.");
    const echoed = result.target as TargetIdentity;
    const key = this.reviewSourceKey(target);
    if (this.reviewSourceKey(echoed) !== key)
      throw new Error("Saved files belong to a different review.");
    this.reviewSources.delete(key);
    if (result.providerSnapshot !== undefined) {
      const info = result.providerSnapshot as ProviderSnapshotInfo;
      if (
        target.kind !== "provider" ||
        !validPublicationDTO("ProviderSnapshotInfo", info) ||
        [info.request, info.snapshot].some(
          (source) =>
            source.prId !== target.prId ||
            source.repository.provider !== target.provider ||
            source.repository.workspace !== target.workspace ||
            source.repository.repo !== target.repo,
        ) ||
        info.headSha.length !== info.baseSha.length ||
        info.headSha.length !== info.destinationSha.length ||
        (info.fileCount === 0) !== (info.patchBytes === 0)
      ) {
        throw new Error("Saved provider evidence belongs to a different review or range.");
      }
      const files = await this.providerSnapshotInventory(info, current);
      if (!current()) throw new Error("Review selection changed.");
      while (this.reviewSources.size >= 64)
        this.reviewSources.delete(this.reviewSources.keys().next().value!);
      this.reviewSources.set(key, info);
      return files.map((file) => ({
        path: file.path,
        oldPath: file.oldPath,
        status: file.status,
        additions: file.additions,
        deletions: file.deletions,
      }));
    }
    return (result.files as ChangedFile[]) ?? [];
  }

  async reviewTargets(): Promise<ReviewTarget[]> {
    const result = await this.transport.request("review.targets", {});
    return (result.targets as ReviewTarget[]) ?? [];
  }

  private async exploreRequest<T>(
    method: string,
    params: object,
    requestType: string,
    resultType: string,
  ): Promise<T> {
    if (!validExploreParams(requestType, params))
      throw new Error("Invalid repository preview request.");
    const result = await this.transport.request(method, params as Record<string, unknown>);
    if (!validExploreResult(resultType, result, params))
      throw new Error("Invalid or mismatched repository preview response.");
    return result as T;
  }
  async repositoryExploreInfo(params: RepositoryExploreInfoParams): Promise<RepositoryExploreInfo> {
    return this.exploreRequest(
      "repository.explore.info",
      params,
      "RepositoryExploreInfoParams",
      "RepositoryExploreInfo",
    );
  }
  private async editorRequest<T>(
    method: string,
    params: object,
    requestType: string,
    resultType: string,
    expected?: RepositoryEditorPrepareParams | RepositoryEditor,
  ): Promise<T> {
    if (!validEditorParams(requestType, params))
      throw new Error("Invalid repository editor request.");
    const result = await this.transport.request(method, params as Record<string, unknown>);
    if (!validEditorResult(resultType, result, params, expected))
      throw new Error("Invalid or mismatched repository editor response.");
    return result as T;
  }
  private async operationsRequest<T>(
    method: string,
    params: object,
    requestType: string,
    resultType: string,
    expected?: RepositoryOperationPrepared | RepositoryOperation,
  ): Promise<T> {
    if (!validOperationParams(requestType, params))
      throw new Error("Invalid repository operation request.");
    const result = await this.transport.request(method, params as Record<string, unknown>);
    if (!validOperationResult(resultType, result, params, expected))
      throw new Error("Invalid or mismatched repository operation response.");
    return result as T;
  }
  repositoryOperationsList(
    params: RepositoryOperationListParams,
  ): Promise<RepositoryOperationPage> {
    return this.operationsRequest(
      "repository.operation.list",
      params,
      "RepositoryOperationListParams",
      "RepositoryOperationPage",
    );
  }
  repositoryOperationsPreview(
    params: RepositoryOperationPreviewParams,
  ): Promise<RepositoryOperationPrepared> {
    return this.operationsRequest(
      "repository.operation.preview",
      params,
      "RepositoryOperationPreviewParams",
      "RepositoryOperationPrepared",
    );
  }
  repositoryOperationsApply(
    params: RepositoryOperationApplyParams,
    expected: RepositoryOperationPrepared,
  ): Promise<RepositoryOperationApplied> {
    return this.operationsRequest(
      "repository.operation.apply",
      params,
      "RepositoryOperationApplyParams",
      "RepositoryOperationApplied",
      expected,
    );
  }
  repositoryOperationsStatus(
    params: RepositoryOperationStatusParams,
    expected: RepositoryOperation,
  ): Promise<RepositoryOperationStatus> {
    return this.operationsRequest(
      "repository.operation.status",
      params,
      "RepositoryOperationStatusParams",
      "RepositoryOperationStatus",
      expected,
    );
  }
  repositoryOperationsCancel(
    params: RepositoryOperationStatusParams,
    expected: RepositoryOperation,
  ): Promise<RepositoryOperationStatus> {
    return this.operationsRequest(
      "repository.operation.cancel",
      params,
      "RepositoryOperationStatusParams",
      "RepositoryOperationStatus",
      expected,
    );
  }
  repositoryEditorPrepare(
    params: RepositoryEditorPrepareParams,
  ): Promise<RepositoryEditorPrepared> {
    return this.editorRequest(
      "repository.editor.prepare",
      params,
      "RepositoryEditorPrepareParams",
      "RepositoryEditorPrepared",
    );
  }
  repositoryEditorOpen(
    params: RepositoryEditorOpenParams,
    expected: RepositoryEditorPrepareParams,
  ): Promise<RepositoryEditorOpened> {
    return this.editorRequest(
      "repository.editor.open",
      params,
      "RepositoryEditorOpenParams",
      "RepositoryEditorOpened",
      expected,
    );
  }
  repositoryEditorStatus(
    params: RepositoryEditorStatusParams,
    expected: RepositoryEditor,
  ): Promise<RepositoryEditorStatus> {
    return this.editorRequest(
      "repository.editor.status",
      params,
      "RepositoryEditorStatusParams",
      "RepositoryEditorStatus",
      expected,
    );
  }
  repositoryEditorCancel(
    params: RepositoryEditorStatusParams,
    expected: RepositoryEditor,
  ): Promise<RepositoryEditorStatus> {
    return this.editorRequest(
      "repository.editor.cancel",
      params,
      "RepositoryEditorStatusParams",
      "RepositoryEditorStatus",
      expected,
    );
  }
  async repositoryExploreFiles(
    params: RepositoryExploreFilesParams,
  ): Promise<RepositoryExploreFilesPage> {
    return this.exploreRequest(
      "repository.explore.files",
      params,
      "RepositoryExploreFilesParams",
      "RepositoryExploreFilesPage",
    );
  }
  async repositoryExploreContent(
    params: RepositoryExploreContentParams,
  ): Promise<RepositoryExploreContentPage> {
    return this.exploreRequest(
      "repository.explore.content",
      params,
      "RepositoryExploreContentParams",
      "RepositoryExploreContentPage",
    );
  }
  async repositoryExploreDiff(
    params: RepositoryExploreDiffParams,
  ): Promise<RepositoryExploreDiffPage> {
    return this.exploreRequest(
      "repository.explore.diff",
      params,
      "RepositoryExploreDiffParams",
      "RepositoryExploreDiffPage",
    );
  }
  async repositoryExploreBlame(
    params: RepositoryExploreBlameParams,
  ): Promise<RepositoryExploreBlamePage> {
    return this.exploreRequest(
      "repository.explore.blame",
      params,
      "RepositoryExploreBlameParams",
      "RepositoryExploreBlamePage",
    );
  }
  async repositoryExploreFind(
    params: RepositoryExploreFindParams,
  ): Promise<RepositoryExploreFindPage> {
    return this.exploreRequest(
      "repository.explore.find",
      params,
      "RepositoryExploreFindParams",
      "RepositoryExploreFindPage",
    );
  }

  private async localRequest<T>(
    method: string,
    input: object,
    requestType: string,
    resultType: string,
  ): Promise<T> {
    const params: Record<string, unknown> = { ...input };
    if (
      !validPublicationDTO(requestType, params) ||
      (params.comparison !== undefined &&
        !validLocalComparison(params.comparison as LocalComparison)) ||
      (params.snapshot !== undefined && !validLocalRef(params.snapshot as LocalSnapshotRef)) ||
      (params.path !== undefined && !localPath(params.path as string))
    )
      throw new Error("Invalid local evidence request.");
    const result = await this.transport.request(method, params);
    if (
      Buffer.byteLength(JSON.stringify(result)) > 1024 * 1024 ||
      !validPublicationDTO(resultType, result) ||
      !sameLocalValue(result.request, params)
    )
      throw new Error("Invalid or mismatched local evidence response.");
    if (result.info !== undefined) {
      const info = result.info as LocalSnapshotInfo;
      if (
        !validLocalInfo(info) ||
        (params.snapshot !== undefined && !sameLocalValue(info.snapshot, params.snapshot)) ||
        (params.comparison !== undefined && !sameLocalValue(info.comparison, params.comparison))
      )
        throw new Error("Local evidence does not match the selected comparison.");
    }
    return result as T;
  }
  async localOptions(params: LocalOptionsParams): Promise<LocalOptionsResult> {
    const result = await this.localRequest<LocalOptionsResult>(
      "local.options",
      params,
      "LocalOptionsParams",
      "LocalOptionsResult",
    );
    const end = params.offset + result.baseRefs.length;
    if (
      end > result.total ||
      result.baseRefs.length > params.limit ||
      (result.nextOffset !== null
        ? result.nextOffset !== end || end >= result.total
        : end !== result.total) ||
      (params.source.kind === "configured" &&
        !sameLocalValue(params.source.repository, result.repository)) ||
      result.baseRefs.some(
        (ref, index) =>
          Buffer.byteLength(ref) > 256 ||
          /\p{Cc}/u.test(ref) ||
          (index > 0 &&
            Buffer.compare(Buffer.from(ref), Buffer.from(result.baseRefs[index - 1]!)) <= 0),
      ) ||
      (result.branch !== null &&
        (Buffer.byteLength(result.branch) > 256 || /\p{Cc}/u.test(result.branch)))
    )
      throw new Error("Invalid local source options page.");
    return result;
  }
  localCapture(params: LocalCaptureParams): Promise<LocalCaptureResult> {
    return this.localRequest("local.snapshot", params, "LocalCaptureParams", "LocalCaptureResult");
  }
  localCancel(params: LocalCancelParams): Promise<LocalCancelResult> {
    return this.localRequest(
      "local.snapshot.cancel",
      params,
      "LocalCancelParams",
      "LocalCancelResult",
    );
  }
  localInfo(params: LocalLookupParams): Promise<LocalInfoResult> {
    return this.localRequest("local.snapshot.info", params, "LocalLookupParams", "LocalInfoResult");
  }
  localRecheck(params: LocalRecheckParams): Promise<LocalRecheckResult> {
    return this.localRequest(
      "local.snapshot.recheck",
      params,
      "LocalRecheckParams",
      "LocalRecheckResult",
    );
  }
  async localFiles(params: LocalFilesParams): Promise<LocalFilesPage> {
    const result = await this.localRequest<LocalFilesPage>(
      "local.snapshot.files",
      params,
      "LocalFilesParams",
      "LocalFilesPage",
    );
    const end = params.offset + result.files.length;
    if (
      end > result.total ||
      result.files.length > params.limit ||
      (result.nextOffset !== null
        ? result.nextOffset !== end || end >= result.total
        : end !== result.total) ||
      new Set(result.files.map((file) => JSON.stringify([file.layer, file.path]))).size !==
        result.files.length ||
      result.files.some(
        (file) => !localPath(file.path) || (file.oldPath !== null && !localPath(file.oldPath)),
      )
    )
      throw new Error("Invalid local evidence file page.");
    return result;
  }
  async localDiff(params: LocalDiffParams): Promise<LocalDiffPage> {
    const result = await this.localRequest<LocalDiffPage>(
      "local.snapshot.diff",
      params,
      "LocalDiffParams",
      "LocalDiffPage",
    );
    const bytes = Buffer.byteLength(result.text),
      end = params.offset + bytes;
    if (
      bytes > 64 * 1024 ||
      end > result.totalBytes ||
      (result.nextOffset !== null
        ? result.nextOffset !== end || end >= result.totalBytes || bytes === 0
        : end !== result.totalBytes) ||
      (params.offset === 0 &&
        result.nextOffset === null &&
        new Bun.CryptoHasher("sha256").update(result.text).digest("hex") !== result.contentSha256)
    )
      throw new Error("Invalid local evidence diff page.");
    return result;
  }

  async localSnapshotImage(
    snapshot: LocalSnapshotRef,
    layer: LocalImageParams["layer"],
    path: string,
    side: "old" | "new",
    current: () => boolean = () => true,
  ): Promise<FilePreview> {
    return this.loadRaster(path, side, current, async (offset, contentSha256) => {
      const params: LocalImageParams = {
        snapshot,
        layer,
        path,
        side,
        offset,
        maxBytes: 192 * 1024,
        contentSha256,
      };
      const page = await this.localRequest<LocalImagePage>(
        "local.snapshot.image",
        params,
        "LocalImageParams",
        "LocalImagePage",
      );
      if (
        (!page.available &&
          (offset !== 0 ||
            page.size !== 0 ||
            page.dataBase64 !== "" ||
            page.mimeType !== "" ||
            page.nextOffset !== null ||
            page.contentSha256 !== null ||
            page.width !== null ||
            page.height !== null ||
            page.reason === null)) ||
        (page.available &&
          (page.reason !== null ||
            !["image/png", "image/jpeg", "image/gif", "image/webp", "image/bmp"].includes(
              page.mimeType,
            ))) ||
        Buffer.from(page.dataBase64, "base64").toString("base64") !== page.dataBase64
      )
        throw new Error("Invalid captured local image availability or content.");
      return { ...page, path, side, offset, unavailableReason: page.reason };
    });
  }

  async localSnapshotBrowser(
    snapshot: LocalSnapshotRef,
    layer: LocalBrowserParams["layer"],
    current: () => boolean = () => true,
  ): Promise<LocalBrowserResult> {
    if (!current()) throw new Error("Local selection changed.");
    const result = await this.localRequest<LocalBrowserResult>(
      "local.snapshot.browser",
      { snapshot, layer },
      "LocalBrowserParams",
      "LocalBrowserResult",
    );
    if (!current()) throw new Error("Local selection changed.");
    const url = new URL(result.url);
    if (
      url.port === "" ||
      Number(url.port) > 65535 ||
      (!result.launchRequested && result.launchError !== null)
    )
      throw new Error("Invalid saved local browser launch response.");
    return result;
  }

  async localSnapshots(params: LocalListParams): Promise<LocalListPage> {
    if (params.offset > 0 && params.catalogVersion === null)
      throw new Error("Saved snapshot continuation requires its catalog version.");
    const page = await this.localRequest<LocalListPage>(
      "local.snapshot.list",
      params,
      "LocalListParams",
      "LocalListPage",
    );
    const end = params.offset + page.snapshots.length;
    if (
      end > page.total ||
      page.snapshots.length > params.limit ||
      (params.catalogVersion !== null && params.catalogVersion !== page.catalogVersion) ||
      (page.nextOffset !== null
        ? page.nextOffset !== end || end >= page.total || page.snapshots.length === 0
        : end !== page.total) ||
      page.snapshots.some((item, index) =>
        page.snapshots
          .slice(0, index)
          .some((previous) => sameLocalValue(previous.snapshot, item.snapshot)),
      ) ||
      page.snapshots.some(
        (item) =>
          !validLocalRef(item.snapshot) ||
          /\p{Cc}/u.test(item.createdAt) ||
          BigInt(item.createdAt) > 9223372036854775807n ||
          /\p{Cc}/u.test(item.branch) ||
          new Set(item.layers).size !== item.layers.length ||
          (item.mode === "branch" && (item.layers.length !== 1 || item.layers[0] !== "committed")),
      )
    )
      throw new Error("Invalid saved local snapshot page.");
    return page;
  }

  async localReviewProfiles(params: LocalLookupParams): Promise<LocalReviewProfilesResult> {
    const result = await this.localRequest<LocalReviewProfilesResult>(
      "local.review.profiles",
      params,
      "LocalLookupParams",
      "LocalReviewProfilesResult",
    );
    const catalog = result.catalog;
    if (
      catalog.target.kind !== "local" ||
      catalog.target.localSnapshotSha256 !== params.snapshot.snapshotId ||
      catalog.profiles.length > 100 ||
      new Set(catalog.profiles).size !== catalog.profiles.length ||
      catalog.profiles.some(
        (name) =>
          name.trim() !== name ||
          /\p{Cc}/u.test(name) ||
          [...name].length > 128 ||
          Buffer.byteLength(name) > 512,
      ) ||
      (catalog.defaultProfile !== null && !catalog.profiles.includes(catalog.defaultProfile))
    )
      throw new Error("Invalid qualified local profile catalog.");
    return result;
  }

  async localReviewStart(params: LocalReviewStartParams): Promise<LocalReviewAccepted> {
    const request = { ...params, profile: params.profile ?? null };
    if (
      request.profile !== null &&
      (request.profile.trim() !== request.profile ||
        /\p{Cc}/u.test(request.profile) ||
        [...request.profile].length > 128 ||
        Buffer.byteLength(request.profile) > 512)
    )
      throw new Error("Invalid local review profile.");
    const result = await this.localRequest<LocalReviewAccepted>(
      "local.review.start",
      request,
      "LocalReviewStartParams",
      "LocalReviewAccepted",
    );
    if (result.target.localSnapshotSha256 !== request.snapshot.snapshotId)
      throw new Error("Local operation does not match the selected snapshot.");
    return result;
  }
  async localReviewHistory(params: LocalReviewHistoryParams): Promise<LocalReviewHistoryPage> {
    const result = await this.localRequest<LocalReviewHistoryPage>(
      "local.review.history",
      params,
      "LocalReviewHistoryParams",
      "LocalReviewHistoryPage",
    );
    this.localPage(params, result.runs.length, result.total, result.nextOffset);
    if (
      new Set(result.runs.map((run) => run.runId)).size !== result.runs.length ||
      result.runs.some((run) => Buffer.byteLength(run.runId) > 256 || /\p{Cc}/u.test(run.runId))
    )
      throw new Error("Invalid local history page.");
    return result;
  }
  private localPage(
    params: { offset: number; limit: number },
    count: number,
    total: number,
    next: number | null,
  ): void {
    const end = params.offset + count;
    if (
      count > params.limit ||
      end > total ||
      (next !== null ? next !== end || end >= total || count === 0 : end !== total)
    )
      throw new Error("Invalid local review page.");
  }
  private localRun(request: { runId?: string | null }, actual: string | null): void {
    if (
      (request.runId != null && actual !== request.runId) ||
      (actual !== null && (Buffer.byteLength(actual) > 256 || /\p{Cc}/u.test(actual)))
    )
      throw new Error("Local response does not match the selected run.");
  }
  async localReviewFindings(params: LocalReviewFindingsParams): Promise<LocalReviewFindingsPage> {
    const request = { ...params, runId: params.runId ?? null };
    this.localRun(request, request.runId);
    const result = await this.localRequest<LocalReviewFindingsPage>(
      "local.review.findings",
      request,
      "LocalReviewFindingsParams",
      "LocalReviewFindingsPage",
    );
    this.localRun(request, result.runId);
    this.localPage(params, result.findings.length, result.total, result.nextOffset);
    const rank = { committed: 0, staged: 1, unstaged: 2 };
    if (
      (result.runId === null && result.total !== 0) ||
      new Set(result.findings.map((finding) => finding.id)).size !== result.findings.length ||
      result.findings.some(
        (finding) =>
          Buffer.byteLength(finding.id) > 256 ||
          /\p{Cc}/u.test(finding.id) ||
          Buffer.byteLength(finding.title) > 4096 ||
          Buffer.byteLength(finding.summary) > 8192 ||
          (finding.anchor === null
            ? finding.candidateLayers.length !== 0
            : !localPath(finding.anchor.path) ||
              (finding.anchor.endLine !== null &&
                finding.anchor.endLine < finding.anchor.startLine)) ||
          finding.candidateLayers.some(
            (layer, index, layers) => index > 0 && rank[layer] <= rank[layers[index - 1]!],
          ),
      )
    )
      throw new Error("Invalid local finding page.");
    return result;
  }
  async localReviewOutput(params: LocalReviewQueryParams): Promise<LocalReviewOutput> {
    const request = { ...params, runId: params.runId ?? null };
    this.localRun(request, request.runId);
    const result = await this.localRequest<LocalReviewOutput>(
      "local.review.output",
      request,
      "LocalReviewQueryParams",
      "LocalReviewOutput",
    );
    this.localRun(request, result.runId);
    if (Buffer.byteLength(result.text) > 64 * 1024 || !result.text.trim())
      throw new Error("Local output exceeds the terminal limit.");
    return result;
  }
  async localReviewThreads(params: LocalReviewQueryParams): Promise<LocalReviewThreads> {
    const request = { ...params, runId: params.runId ?? null };
    this.localRun(request, request.runId);
    const result = await this.localRequest<LocalReviewThreads>(
      "local.review.threads",
      request,
      "LocalReviewQueryParams",
      "LocalReviewThreads",
    );
    this.localRun(request, result.runId);
    if (
      new Set(result.threads.map((thread) => thread.id)).size !== result.threads.length ||
      result.threads.filter((thread) => thread.selected).length > 1 ||
      (result.runId === null && result.threads.some((thread) => thread.selected)) ||
      result.threads.some(
        (thread) =>
          Buffer.byteLength(thread.id) > 256 ||
          /\p{Cc}/u.test(thread.id) ||
          Buffer.byteLength(thread.latestRunId) > 256 ||
          /\p{Cc}/u.test(thread.latestRunId) ||
          new Set(thread.messages.map((message) => message.id)).size !== thread.messages.length ||
          thread.messages.some(
            (message) => Buffer.byteLength(message.id) > 256 || /\p{Cc}/u.test(message.id),
          ),
      )
    )
      throw new Error("Invalid local conversation response.");
    return result;
  }

  async reviewProfiles(target: TargetIdentity): Promise<ReviewProfileCatalog> {
    if (
      !validPublicationDTO("ReviewProfilesParams", { target }) ||
      (target.kind === "provider" && target.runId !== undefined && target.prId === undefined)
    )
      throw new Error("Invalid review profile target.");
    const result = await this.transport.request("review.profiles", { target });
    if (
      JSON.stringify(result).length > 64 * 1024 ||
      !validPublicationDTO("ReviewProfileCatalog", result)
    )
      throw new Error("Invalid or oversized review profile catalog.");
    const catalog = result as unknown as ReviewProfileCatalog;
    if (
      this.reviewSourceKey(catalog.target) !== this.reviewSourceKey(target) ||
      catalog.profiles.some(
        (name) =>
          name.trim() !== name ||
          /\p{Cc}/u.test(name) ||
          new TextEncoder().encode(name).length > 512,
      ) ||
      (catalog.defaultProfile !== null && !catalog.profiles.includes(catalog.defaultProfile)) ||
      (catalog.source === "base" &&
        (catalog.profiles.length !== 0 || catalog.defaultProfile !== null)) ||
      (catalog.notice !== null && /\p{Cc}/u.test(catalog.notice))
    )
      throw new Error("Review profile catalog does not match the selected target or policy.");
    return catalog;
  }

  async reviewHistory(target: TargetIdentity): Promise<ReviewRunSummary[]> {
    const result = await this.transport.request("review.history", { target });
    return (result.runs as ReviewRunSummary[]) ?? [];
  }

  async reviewThreads(target: TargetIdentity): Promise<ReviewThreadSummary[]> {
    const result = await this.transport.request("review.threads", { target });
    return (result.threads as ReviewThreadSummary[]) ?? [];
  }
  async reviewOutput(target: TargetIdentity): Promise<string> {
    const result = await this.transport.request("review.output", { target });
    const actual = result.target as TargetIdentity | undefined;
    if (
      target.kind !== "provider" ||
      actual?.kind !== "provider" ||
      actual.provider !== target.provider ||
      actual.workspace !== target.workspace ||
      actual.repo !== target.repo ||
      actual.prId !== target.prId ||
      actual.runId !== target.runId ||
      typeof result.runId !== "string" ||
      !result.runId ||
      (target.runId !== undefined && target.runId !== null && result.runId !== target.runId) ||
      typeof result.text !== "string" ||
      !result.text.trim() ||
      Buffer.byteLength(result.text) > 64 * 1024
    )
      throw new Error(
        "Selected stored review output is unavailable or exceeds the clipboard limit.",
      );
    return result.text;
  }
  async readiness(): Promise<WorkspaceReadiness> {
    const result = await this.transport.request("workspace.readiness", {});
    const tools = result.tools as WorkspaceReadiness["tools"];
    if (
      Object.keys(result).some(
        (key) =>
          ![
            "configuredRepos",
            "selectedAgent",
            "tools",
            "configRootAvailable",
            "dataRootAvailable",
            "credentialStore",
          ].includes(key),
      ) ||
      !Number.isInteger(result.configuredRepos) ||
      Number(result.configuredRepos) < 0 ||
      Number(result.configuredRepos) > 100 ||
      !["claude", "codex", "opencode"].includes(String(result.selectedAgent)) ||
      !["native", "unsupported"].includes(String(result.credentialStore)) ||
      typeof result.configRootAvailable !== "boolean" ||
      typeof result.dataRootAvailable !== "boolean" ||
      !Array.isArray(tools) ||
      tools.length !== 4 ||
      new Set(tools.map((tool) => tool?.name)).size !== 4 ||
      tools.some(
        (tool) =>
          !tool ||
          Object.keys(tool).some((key) => !["name", "available"].includes(key)) ||
          !["git", "claude", "codex", "opencode"].includes(tool.name) ||
          typeof tool.available !== "boolean",
      )
    )
      throw new Error("Readiness status is unavailable or invalid.");
    return result as unknown as WorkspaceReadiness;
  }

  async browserOpen(target: TargetIdentity): Promise<{ url: string }> {
    const result = await this.transport.request("browser.open", { target });
    return result as unknown as { url: string };
  }

  async filePreview(
    target: TargetIdentity,
    path: string,
    side: "old" | "new" = "new",
    current: () => boolean = () => true,
  ): Promise<FilePreview> {
    return this.loadRaster(path, side, current, (offset, contentSha256) =>
      this.transport.request("file.preview", {
        target,
        path,
        side,
        offset,
        maxBytes: 192 * 1024,
        ...(contentSha256 ? { contentSha256 } : {}),
      }),
    );
  }

  private async loadRaster(
    path: string,
    side: "old" | "new",
    current: () => boolean,
    read: (offset: number, hash: string | null) => Promise<Record<string, unknown>>,
  ): Promise<FilePreview> {
    const chunks: Uint8Array[] = [];
    let offset = 0;
    let contentSha256: string | null = null;
    let first: FilePreview | null = null;
    for (let index = 0; index < 12; index++) {
      if (!current()) throw new Error("Image selection changed.");
      const result = await read(offset, contentSha256);
      if (!current()) throw new Error("Image selection changed.");
      if (result.path !== path || result.side !== side || typeof result.available !== "boolean")
        throw new Error("Image response does not match its requested side.");
      if (!result.available) return result as unknown as FilePreview;
      if (
        typeof result.size !== "number" ||
        !Number.isInteger(result.size) ||
        result.size <= 0 ||
        result.size > 2 * 1024 * 1024 ||
        result.offset !== offset ||
        typeof result.dataBase64 !== "string" ||
        result.dataBase64.length > 256 * 1024 ||
        typeof result.contentSha256 !== "string" ||
        !/^[a-f0-9]{64}$/.test(result.contentSha256) ||
        typeof result.width !== "number" ||
        !Number.isInteger(result.width) ||
        typeof result.height !== "number" ||
        !Number.isInteger(result.height) ||
        result.width <= 0 ||
        result.height <= 0 ||
        result.width > 8192 ||
        result.height > 8192 ||
        result.width * result.height > 40_000_000
      )
        throw new Error("Invalid or oversized image chunk.");
      if (contentSha256 && contentSha256 !== result.contentSha256)
        throw new Error("Image changed during loading.");
      contentSha256 = result.contentSha256;
      if (!first) first = result as unknown as FilePreview;
      if (
        result.size !== first.size ||
        result.mimeType !== first.mimeType ||
        result.width !== first.width ||
        result.height !== first.height
      )
        throw new Error("Image metadata changed during loading.");
      const bytes = Uint8Array.from(Buffer.from(result.dataBase64, "base64"));
      if (bytes.length === 0 || bytes.length > 192 * 1024 || offset + bytes.length > first.size)
        throw new Error("Invalid image chunk length.");
      chunks.push(bytes);
      offset += bytes.length;
      if (result.nextOffset === null) {
        if (offset !== first.size)
          throw new Error("Image transfer ended before all bytes arrived.");
        const combined = new Uint8Array(first.size);
        let write = 0;
        for (const chunk of chunks) {
          combined.set(chunk, write);
          write += chunk.length;
        }
        if (new Bun.CryptoHasher("sha256").update(combined).digest("hex") !== contentSha256)
          throw new Error("Image content did not match its approved version.");
        if (!current()) throw new Error("Image selection changed.");
        return { ...first, dataBase64: Buffer.from(combined).toString("base64") };
      }
      if (result.nextOffset !== offset)
        throw new Error("Image transfer did not advance to the expected chunk.");
    }
    throw new Error("Image requires too many chunks.");
  }

  async reviewFindings(target: TargetIdentity): Promise<FindingSummary[]> {
    const result = await this.transport.request("review.findings", { target });
    return (result.findings as FindingSummary[]) ?? [];
  }

  async reviewReply(target: TargetIdentity, message: string): Promise<OperationStatus> {
    const result = await this.transport.request("review.reply", { target, message });
    return result as unknown as OperationStatus;
  }

  async fixPreview(target: TargetIdentity): Promise<FixPreview> {
    return (await this.transport.request("review.fix.preview", {
      target,
    })) as unknown as FixPreview;
  }

  async reviewFix(target: TargetIdentity, approvalToken: string): Promise<OperationStatus> {
    return (await this.transport.request("review.fix", {
      target,
      approvalToken,
    })) as unknown as OperationStatus;
  }

  async commitPreview(target: TargetIdentity, message: string): Promise<FixPreview> {
    return (await this.transport.request("review.commit.preview", {
      target,
      message,
    })) as unknown as FixPreview;
  }

  async pushPreview(target: TargetIdentity): Promise<FixPreview> {
    return (await this.transport.request("review.push.preview", {
      target,
    })) as unknown as FixPreview;
  }

  async reviewCommit(target: TargetIdentity, approvalToken: string): Promise<OperationStatus> {
    return (await this.transport.request("review.commit", {
      target,
      approvalToken,
    })) as unknown as OperationStatus;
  }

  async reviewPush(target: TargetIdentity, approvalToken: string): Promise<OperationStatus> {
    return (await this.transport.request("review.push", {
      target,
      approvalToken,
    })) as unknown as OperationStatus;
  }

  async branchPreview(target: TargetIdentity, action: "sync" | "conflicts"): Promise<FixPreview> {
    return (await this.transport.request(
      action === "sync" ? "review.sync.preview" : "review.conflicts.preview",
      { target },
    )) as unknown as FixPreview;
  }

  async reviewBranch(
    target: TargetIdentity,
    approvalToken: string,
    action: "sync" | "conflicts",
  ): Promise<OperationStatus> {
    return (await this.transport.request(action === "sync" ? "review.sync" : "review.conflicts", {
      target,
      approvalToken,
    })) as unknown as OperationStatus;
  }

  async fixState(target: TargetIdentity): Promise<FixState | null> {
    const result = await this.transport.request("review.fix.state", { target });
    return (result.state as FixState | null) ?? null;
  }

  async startReview(
    target: TargetIdentity,
    profile: string | null = null,
    snapshot: ProviderSnapshotRef | null = null,
  ): Promise<OperationStatus> {
    const params = { target, profile, ...(snapshot ? { snapshot } : {}) };
    if (!validPublicationDTO("ReviewStartParams", params))
      throw new Error("Invalid review start parameters.");
    if (
      snapshot &&
      (target.kind !== "provider" ||
        target.runId != null ||
        snapshot.prId !== target.prId ||
        snapshot.repository.provider !== target.provider ||
        snapshot.repository.workspace !== target.workspace ||
        snapshot.repository.repo !== target.repo)
    )
      throw new Error("Review preview belongs to a different target.");
    const result = await this.transport.request("review.start", params);
    return result as unknown as OperationStatus;
  }

  operationStatus(operationId: string): Promise<OperationStatus> {
    return this.transport
      .request("operation.status", { operationId })
      .then((result) => result as unknown as OperationStatus);
  }

  operationCancel(operationId: string): Promise<OperationStatus> {
    return this.transport
      .request("operation.cancel", { operationId })
      .then((result) => result as unknown as OperationStatus);
  }

  shutdown(): Promise<Record<string, unknown>> {
    return this.transport.request("shutdown", {});
  }

  onOperationEvent(listener: (event: BackendOperationEvent) => void): () => void {
    return this.transport.onOperationEvent(listener);
  }

  onExit(listener: (exit: BackendExit) => void): () => void {
    return this.transport.onExit(listener);
  }

  async close(): Promise<void> {
    await this.transport.close();
  }
}
