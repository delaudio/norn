import type {
  PrRepositoryIdentity,
  RepositoryOperation,
  RepositoryOperationCheckout,
  RepositoryOperationPrepared,
} from "../../../src/protocol/generated/protocol";
import { ExplorerFixture } from "./explorer-backend";
export class RepositoryOperationsFixture extends ExplorerFixture {
  operationCalls: Array<{ method: string; params: Record<string, unknown> }> = [];
  operation: RepositoryOperation | null = null;
  prepared: RepositoryOperationPrepared | null = null;
  finalState: "completed" | "failed" | "canceled" | "uncertain" = "completed";
  hold = false;
  malformed = false;
  expiresInMs = 30000;
  waitList: Promise<void> | null = null;
  checkout(repository: PrRepositoryIdentity): RepositoryOperationCheckout {
    return {
      repository,
      context: { repository, rootId: (repository.provider === "github" ? "a" : "b").repeat(64) },
      state: "ready",
      branch: "main",
      headSha: "d".repeat(40),
      upstream: "refs/remotes/origin/main",
      stashSha: null,
      dirty: false,
      filtered: false,
      conflicted: false,
      pending: false,
      version: "1".repeat(64),
      message: null,
    };
  }
  override async start() {
    const ready = await super.start();
    return { ...ready, capabilities: [...ready.capabilities, "repository-operations"] };
  }
  override async request(method: string, params: Record<string, unknown>) {
    if (!method.startsWith("repository.operation.")) return super.request(method, params);
    this.operationCalls.push({ method, params: structuredClone(params) });
    if (method.endsWith(".list")) {
      await this.waitList;
      const repository = params.repository as PrRepositoryIdentity;
      return {
        request: params,
        checkout: this.checkout(repository),
        branches: [
          {
            reference: "refs/heads/main",
            name: "main",
            kind: "local",
            oid: "d".repeat(40),
            current: true,
          },
          {
            reference: "refs/remotes/origin/feature-ü;$(owned)",
            name: "origin/feature-ü;$(owned)",
            kind: "remote",
            oid: "e".repeat(40),
            current: false,
          },
        ],
        total: 2,
        nextOffset: null,
      };
    }
    if (method.endsWith(".preview")) {
      this.prepared = {
        request: params as unknown as RepositoryOperationPrepared["request"],
        token: "c".repeat(64),
        expiresInMs: this.expiresInMs,
        before: this.checkout((params.context as RepositoryOperation["context"]).repository),
        summary: "Checkout existing local branch main at the verified commit.",
        warnings: ["Configured Git helpers run only after approval."],
      };
      return { ...this.prepared };
    }
    if (method.endsWith(".apply") && this.prepared) {
      this.operation = {
        operationId: "f".repeat(64),
        context: this.prepared.request.context,
        action: this.prepared.request.action,
        before: this.prepared.before,
        after: null,
        state: "starting",
        exitCode: null,
        stateChanged: null,
        recoveryRequired: false,
        message: "Starting owned Git operation",
      };
      return { request: params, operation: this.operation };
    }
    if ((method.endsWith(".status") || method.endsWith(".cancel")) && this.operation) {
      const canceled = method.endsWith(".cancel");
      if (!this.hold || canceled)
        this.operation = {
          ...this.operation,
          state: canceled ? "canceled" : this.finalState,
          after: this.finalState === "uncertain" ? null : this.operation.before,
          stateChanged: this.finalState === "uncertain" ? null : false,
          exitCode: canceled ? null : this.finalState === "completed" ? 0 : 1,
          recoveryRequired: canceled || this.finalState !== "completed",
          message: "Observed actual state; no automatic retry",
        };
      const result = {
        request: params,
        operation: structuredClone(this.operation),
        cancelRequested: canceled,
      };
      if (this.malformed) result.operation.context.rootId = "9".repeat(64);
      return result;
    }
    throw new Error("Unexpected repository operation fixture");
  }
}
