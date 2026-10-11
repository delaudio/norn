import type {
  AuthProviderParams,
  DraftPreparation,
  DraftPrepareParams,
  DraftStageParams,
  DraftStageResult,
  DraftTarget,
  RecordMutation,
  RecordMutationResult,
  RecordPage,
  WorkspaceReadiness,
  WorkspaceSettings,
} from "../../../src/protocol/generated/protocol";

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

export type ChangedFileStatus = "added" | "modified" | "deleted" | "renamed" | "untracked";

export interface ChangedFile {
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
export class BackendClient {
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

  async reviewFiles(target: TargetIdentity): Promise<ChangedFile[]> {
    const result = await this.transport.request("review.files", { target });
    return (result.files as ChangedFile[]) ?? [];
  }

  async reviewTargets(): Promise<ReviewTarget[]> {
    const result = await this.transport.request("review.targets", {});
    return (result.targets as ReviewTarget[]) ?? [];
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
    const chunks: Uint8Array[] = [];
    let offset = 0;
    let contentSha256: string | null = null;
    let first: FilePreview | null = null;
    for (let index = 0; index < 12; index++) {
      if (!current()) throw new Error("Image selection changed.");
      const result = await this.transport.request("file.preview", {
        target,
        path,
        side,
        offset,
        maxBytes: 192 * 1024,
        ...(contentSha256 ? { contentSha256 } : {}),
      });
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
  ): Promise<OperationStatus> {
    const result = await this.transport.request("review.start", { target, profile });
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

  close(): void {
    this.transport.close();
  }
}
