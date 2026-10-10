// Deterministic in-memory transport for tests, browser/dev mode and Storybook.
// It implements the same contract as the subprocess transport.

import type { TargetIdentity } from "./protocol";
import type { BackendExit, BackendOperationEvent, BackendTransport, ReadyInfo } from "./types";

export interface FakeTransportOptions {
  serverVersion?: string;
  repositories?: Array<Record<string, unknown>>;
  targets?: Array<Record<string, unknown>>;
  files?: Array<Record<string, unknown>>;
  findings?: Array<Record<string, unknown>>;
  /** Overrides the synthetic diff returned by `diff.file`. */
  diffText?: string;
  /** Artificial latency in milliseconds for every request. */
  delayMs?: number;
}

interface FakeOperation {
  target: TargetIdentity;
  state: string;
  sequence: number;
}

const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

export class FakeBackendTransport implements BackendTransport {
  private readonly options: FakeTransportOptions;
  private readonly operations = new Map<string, FakeOperation>();
  private readonly eventListeners = new Set<(event: BackendOperationEvent) => void>();
  private readonly exitListeners = new Set<(exit: BackendExit) => void>();
  private nextOperation = 1;
  private closed = false;

  constructor(options: FakeTransportOptions = {}) {
    this.options = options;
  }

  async start(): Promise<ReadyInfo> {
    await this.delay();
    return {
      serverVersion: this.options.serverVersion ?? "fake-0.0.0",
      capabilities: ["repository", "diff", "review"],
    };
  }

  async request(method: string, params: Record<string, unknown>): Promise<Record<string, unknown>> {
    if (this.closed) {
      throw new Error("backend transport is closed");
    }
    await this.delay();
    switch (method) {
      case "repository.status":
        return { repos: this.options.repositories ?? [] };
      case "review.targets":
        return { targets: this.options.targets ?? [] };
      case "browser.open":
        return { url: `http://127.0.0.1:0/session/${"0".repeat(64)}/` };
      case "review.files":
        return { target: params.target, files: this.options.files ?? [] };
      case "review.findings":
        return { target: params.target, findings: this.options.findings ?? [] };
      case "diff.file":
        return {
          target: params.target,
          path: params.path,
          diff: this.options.diffText ?? "@@ -1 +1 @@\n-old\n+new\n",
          truncated: false,
        };
      case "review.start": {
        const operationId = `fake-op-${this.nextOperation++}`;
        const target = params.target as TargetIdentity;
        this.operations.set(operationId, { target, state: "accepted", sequence: 0 });
        // Emit after the response is delivered, matching the real backend where
        // the accepted response precedes operation events.
        setTimeout(() => {
          const operation = this.operations.get(operationId);
          if (!operation || operation.state === "cancelled" || this.closed) {
            return;
          }
          this.emit(operationId, target, 1, { kind: "state", state: "running" });
          this.emit(operationId, target, 2, {
            kind: "progress",
            message: "collected file",
            completed: 1,
            total: 1,
          });
          this.emit(operationId, target, 3, { kind: "state", state: "succeeded" });
          operation.state = "succeeded";
          operation.sequence = 3;
        }, 0);
        return { target, operationId, state: "accepted" };
      }
      case "operation.status":
      case "operation.cancel": {
        const operationId = params.operationId as string;
        const operation = this.operations.get(operationId);
        if (!operation) {
          throw new Error("unknown operation");
        }
        if (method === "operation.cancel") {
          operation.state = "cancelled";
          operation.sequence += 1;
          this.emit(operationId, operation.target, operation.sequence, {
            kind: "state",
            state: "cancelled",
          });
        }
        return {
          operationId,
          state: operation.state,
          sequence: operation.sequence,
          error: null,
        };
      }
      case "shutdown":
        return {};
      default:
        throw new Error(`unknown method ${method}`);
    }
  }

  onOperationEvent(listener: (event: BackendOperationEvent) => void): () => void {
    this.eventListeners.add(listener);
    return () => this.eventListeners.delete(listener);
  }

  onExit(listener: (exit: BackendExit) => void): () => void {
    this.exitListeners.add(listener);
    return () => this.exitListeners.delete(listener);
  }

  close(): void {
    if (this.closed) {
      return;
    }
    this.closed = true;
    for (const listener of this.exitListeners) {
      listener({ code: 0 });
    }
  }

  private emit(
    operationId: string,
    target: TargetIdentity,
    sequence: number,
    event: unknown,
  ): void {
    for (const listener of this.eventListeners) {
      listener({ operationId, sequence, target, event });
    }
  }

  private async delay(): Promise<void> {
    if (this.options.delayMs) {
      await sleep(this.options.delayMs);
    }
  }
}
