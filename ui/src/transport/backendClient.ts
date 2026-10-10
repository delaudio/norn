// Typed facade over a BackendTransport. Domain methods are added as later
// migration steps land; the first slice exposes repository listing and diffs.

import type { TargetIdentity } from "./protocol";
import type { BackendExit, BackendOperationEvent, BackendTransport } from "./types";

export interface RepositorySummary {
  provider: "github" | "bitbucket";
  workspace: string;
  repo: string;
  localPath: string | null;
}

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

export class BackendClient {
  constructor(private readonly transport: BackendTransport) {}

  start() {
    return this.transport.start();
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

  async browserOpen(target: TargetIdentity): Promise<{ url: string }> {
    const result = await this.transport.request("browser.open", { target });
    return result as unknown as { url: string };
  }

  async filePreview(
    target: TargetIdentity,
    path: string,
  ): Promise<{ path: string; mimeType: string; size: number; dataBase64: string }> {
    const result = await this.transport.request("file.preview", { target, path });
    return result as unknown as {
      path: string;
      mimeType: string;
      size: number;
      dataBase64: string;
    };
  }

  async reviewFindings(target: TargetIdentity): Promise<FindingSummary[]> {
    const result = await this.transport.request("review.findings", { target });
    return (result.findings as FindingSummary[]) ?? [];
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
