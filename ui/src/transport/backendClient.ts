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
    contextLines?: number,
  ): Promise<DiffFileResult> {
    const result = await this.transport.request("diff.file", {
      target,
      path,
      ...(contextLines === undefined ? {} : { contextLines }),
    });
    return result as unknown as DiffFileResult;
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
