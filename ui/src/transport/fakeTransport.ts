import type {
  WorkspaceReference,
  WorkspaceSettings,
  WorkspaceValue,
} from "../../../src/protocol/generated/protocol";
import type { CredentialStatus } from "./backendClient";
// Deterministic in-memory transport for tests, browser/dev mode and Storybook.
// It implements the same contract as the subprocess transport.

import type { TargetIdentity } from "./protocol";
import type { BackendExit, BackendOperationEvent, BackendTransport, ReadyInfo } from "./types";

export interface FakeTransportOptions {
  serverVersion?: string;
  repositories?: Array<Record<string, unknown>>;
  targets?: Array<Record<string, unknown>>;
  history?: Array<Record<string, unknown>>;
  threads?: Array<Record<string, unknown>>;
  files?: Array<Record<string, unknown>>;
  findings?: Array<Record<string, unknown>>;
  /** Overrides the synthetic diff returned by `diff.file`. */
  diffText?: string;
  reviewOutput?: string;
  /** Artificial latency in milliseconds for every request. */
  delayMs?: number;
  settings?: WorkspaceSettings;
  credentials?: CredentialStatus[];
  fixPhase?: "readyToCommit" | "readyToPush" | "completed";
  workspaceRecords?: Record<string, WorkspaceValue>;
  availableTools?: string[];
  credentialStore?: "native" | "unsupported";
}

interface FakeOperation {
  target: TargetIdentity;
  state: string;
  sequence: number;
}

const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

export class FakeBackendTransport implements BackendTransport {
  private readonly options: FakeTransportOptions;
  private records: Record<string, WorkspaceValue>;
  private readonly operations = new Map<string, FakeOperation>();
  private readonly eventListeners = new Set<(event: BackendOperationEvent) => void>();
  private readonly exitListeners = new Set<(exit: BackendExit) => void>();
  private settingsValue: WorkspaceSettings;
  private settingsVersion = 1;
  private credentials: CredentialStatus[];
  private readonly environmentCredentials = new Set<CredentialStatus["provider"]>();
  private nextOperation = 1;
  private closed = false;
  private nextApproval = 1;
  private fixPhase: "readyToCommit" | "readyToPush" | "completed";
  private readonly approvals = new Map<
    string,
    { target: string; action: string; message?: string }
  >();

  constructor(options: FakeTransportOptions = {}) {
    this.options = options;
    this.records = structuredClone(options.workspaceRecords ?? {});
    this.fixPhase = options.fixPhase ?? "readyToCommit";
    this.settingsValue = structuredClone(
      options.settings ?? {
        repos: (options.repositories ?? []).map((repo) => ({
          provider: repo.provider === "bitbucket" ? "bitbucket" : "github",
          workspace: String(repo.workspace ?? ""),
          repo: String(repo.repo ?? ""),
          localPath: typeof repo.localPath === "string" ? repo.localPath : null,
        })),
        reviewProvider: "github",
        aiProvider: "claude",
        defaultDiffView: "unified",
        theme: "dark",
        claudeModel: null,
        claudeEffort: null,
        codexModel: null,
        codexEffort: null,
        opencodeModel: null,
        opencodeEffort: null,
        jiraBaseUrl: null,
        automaticSyncIntervalSeconds: null,
      },
    );
    this.credentials = structuredClone(
      options.credentials ??
        ["github", "bitbucket", "jira", "notion"].map((provider) => ({
          provider: provider as CredentialStatus["provider"],
          available: false,
          source: "none" as const,
        })),
    );
    for (const status of this.credentials)
      if (status.source === "environment") this.environmentCredentials.add(status.provider);
  }

  async start(): Promise<ReadyInfo> {
    await this.delay();
    return {
      serverVersion: this.options.serverVersion ?? "fake-0.0.0",
      capabilities: [
        "repository",
        "diff",
        "review",
        "settings",
        "auth",
        "workspace-records",
        "workspace-readiness",
      ],
    };
  }

  async request(method: string, params: Record<string, unknown>): Promise<Record<string, unknown>> {
    if (this.closed) throw new Error("backend transport is closed");
    if (method.startsWith("workspace.")) await this.delay();
    if (method === "workspace.read" || method === "workspace.mutate") {
      const key = String(params.key);
      const stored = this.records[key];
      const version = new Bun.CryptoHasher("sha256")
        .update(stored ? JSON.stringify(stored) : "")
        .digest("hex");
      const kind = key === "theme" ? "theme" : key.split(".")[0];
      const payload =
        stored ??
        ({
          kind,
          value: kind === "theme" ? "dark" : kind === "prompt" ? "" : [],
        } as WorkspaceValue);
      if (method === "workspace.mutate") {
        if (params.expectedVersion !== version)
          throw new Error("User-data record changed; refresh before saving your edit.");
        const mutation = params.mutation as {
          kind: string;
          value?: string;
          path?: string;
          reference?: WorkspaceReference;
          id?: string;
        };
        let next: WorkspaceValue;
        if (payload.kind === "prompt" && mutation.kind === "setPrompt")
          next = { kind: "prompt", value: mutation.value ?? "" };
        else if (payload.kind === "viewed" && ["addViewed", "removeViewed"].includes(mutation.kind))
          next = {
            kind: "viewed",
            value:
              mutation.kind === "removeViewed"
                ? payload.value.filter((item) => item !== mutation.path)
                : [...new Set([...payload.value, mutation.path ?? ""])],
          };
        else if (
          payload.kind === "references" &&
          mutation.kind === "putReference" &&
          mutation.reference
        )
          next = {
            kind: "references",
            value: payload.value.some((item) => item.id === mutation.reference!.id)
              ? payload.value.map((item) =>
                  item.id === mutation.reference!.id ? structuredClone(mutation.reference!) : item,
                )
              : [...payload.value, structuredClone(mutation.reference)],
          };
        else if (payload.kind === "references" && mutation.kind === "removeReference")
          next = {
            kind: "references",
            value: payload.value.filter((item) => item.id !== mutation.id),
          };
        else throw new Error("Unsupported fixture mutation.");
        this.records[key] = next;
        const recordVersion = new Bun.CryptoHasher("sha256")
          .update(JSON.stringify(next))
          .digest("hex");
        return { key, recordVersion, changed: recordVersion !== version };
      }
      const offset = Number(params.offset ?? 0);
      if (params.recordVersion && params.recordVersion !== version)
        throw new Error("Fixture snapshot expired.");
      const value = Array.isArray(payload.value)
        ? payload.value.slice(offset, offset + 100)
        : payload.value;
      const totalItems = Array.isArray(payload.value) ? payload.value.length : stored ? 1 : 0;
      const end = offset + (Array.isArray(value) ? value.length : 1);
      return {
        key,
        recordVersion: version,
        available: !!stored,
        offset,
        nextOffset: end < totalItems ? end : null,
        totalItems,
        payload: { kind, value },
      };
    }
    if (this.closed) {
      throw new Error("backend transport is closed");
    }
    await this.delay();
    switch (method) {
      case "settings.get":
        return {
          settings: structuredClone(this.settingsValue),
          version: this.settingsVersion.toString(16).padStart(64, "0"),
        };
      case "settings.save":
        if (params.expectedVersion !== this.settingsVersion.toString(16).padStart(64, "0"))
          throw new Error("Settings changed; reload before saving.");
        this.settingsValue = structuredClone(params.settings as WorkspaceSettings);
        this.settingsVersion++;
        return {
          settings: structuredClone(this.settingsValue),
          version: this.settingsVersion.toString(16).padStart(64, "0"),
        };
      case "auth.status":
        return { credentials: structuredClone(this.credentials) };
      case "auth.set": {
        if (typeof params.token !== "string" || !params.token.trim())
          throw new Error("A credential is required.");
        const status = this.credentials.find((status) => status.provider === params.provider);
        if (!status) throw new Error("Unsupported credential provider.");
        status.available = true;
        status.source = "keychain";
        // The fake stores only presence, never the input secret.
        return { saved: true, provider: params.provider };
      }
      case "auth.clear": {
        const status = this.credentials.find((status) => status.provider === params.provider);
        if (!status) throw new Error("Unsupported credential provider.");
        status.available = this.environmentCredentials.has(status.provider);
        status.source = status.available ? "environment" : "none";
        return { cleared: true, provider: params.provider };
      }
      case "repository.status":
        return { repos: structuredClone(this.settingsValue.repos) };
      case "workspace.readiness":
        return {
          configuredRepos: this.settingsValue.repos.length,
          selectedAgent: this.settingsValue.aiProvider,
          tools: ["git", "claude", "codex", "opencode"].map((name) => ({
            name,
            available: (
              this.options.availableTools ?? ["git", "claude", "codex", "opencode"]
            ).includes(name),
          })),
          configRootAvailable: true,
          dataRootAvailable: true,
          credentialStore: this.options.credentialStore ?? "native",
        };
      case "review.targets":
        return { targets: this.options.targets ?? [] };
      case "review.history":
        return { runs: this.options.history ?? [] };
      case "review.threads":
        return { threads: this.options.threads ?? [] };
      case "review.output":
        return {
          target: params.target,
          runId: (params.target as { runId?: string }).runId ?? "fake-run",
          text: this.options.reviewOutput ?? "Synthetic stored review output.",
        };
      case "browser.open":
        return { url: `http://127.0.0.1:0/session/${"0".repeat(64)}/` };
      case "file.preview": {
        const file = this.options.files?.find((file) => file.path === params.path);
        if (
          (params.side === "old" && ["added", "untracked"].includes(String(file?.status ?? ""))) ||
          (params.side === "new" && file?.status === "deleted")
        )
          return {
            path: params.path,
            side: params.side,
            available: false,
            mimeType: "",
            size: 0,
            dataBase64: "",
            offset: 0,
            nextOffset: null,
            contentSha256: null,
            width: null,
            height: null,
          };
        const data =
          params.side === "old"
            ? "iVBORw0KGgoAAAANSUhEUgAAAAIAAAACCAYAAABytg0kAAAAEUlEQVR4nGOoCLD5D8IMMAYARXwIDRVgxqcAAAAASUVORK5CYII="
            : "iVBORw0KGgoAAAANSUhEUgAAAAIAAAACCAYAAABytg0kAAAAEUlEQVR4nGPQyOv5D8IMMAYAR3AIhdwoG5cAAAAASUVORK5CYII=";
        const bytes = Buffer.from(data, "base64");
        return {
          path: params.path,
          side: params.side ?? "new",
          available: true,
          mimeType: "image/png",
          size: bytes.length,
          dataBase64: data,
          offset: 0,
          nextOffset: null,
          width: 2,
          height: 2,
          contentSha256: new Bun.CryptoHasher("sha256").update(bytes).digest("hex"),
        };
      }
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
      case "review.commit.preview":
      case "review.sync.preview":
      case "review.conflicts.preview":
      case "review.push.preview":
      case "review.fix.preview": {
        const action =
          method === "review.commit.preview"
            ? "commit"
            : method === "review.push.preview"
              ? "push"
              : method === "review.sync.preview"
                ? "sync"
                : method === "review.conflicts.preview"
                  ? "conflicts"
                  : "fix";
        if (
          action === "commit" &&
          (this.fixPhase !== "readyToCommit" || !(params.message as string)?.trim())
        )
          throw new Error("Complete a fix and provide a commit message.");
        if (action === "push" && this.fixPhase !== "readyToPush")
          throw new Error("Create a commit before pushing.");
        const token = String(this.nextApproval++).padStart(64, "0");
        this.approvals.set(token, {
          target: JSON.stringify(params.target),
          action,
          message: params.message as string | undefined,
        });
        return {
          target: params.target,
          action,
          commitMessage: params.message ?? null,
          filesToCommit: action === "commit" ? ["src/app.ts"] : [],
          commitSha: action === "push" ? "c".repeat(40) : null,
          approvalToken: token,
          repoPath: "/tmp/example-clone",
          currentBranch: "main",
          sourceBranch: "feature",
          destinationBranch: "main",
          dirtyFiles: [" M src/app.ts"],
          snapshotSha256: "a".repeat(64),
          expiresInSeconds: 300,
          effects:
            action === "commit"
              ? [
                  `Commit only src/app.ts with message: ${params.message}`,
                  "Exclude unrelated staged files. No push or publication.",
                ]
              : action === "push"
                ? [
                    "Push the approved commit to origin/feature without force.",
                    "Inspect the remote on interruption. No automatic retry.",
                  ]
                : action === "sync" || action === "conflicts"
                  ? [
                      "Requires a clean source checkout; fetch and merge destination without committing.",
                      "Preserve conflicts and partial changes. No automatic commit or push.",
                      ...(action === "conflicts"
                        ? ["Run Claude to resolve conflicts and verify Git index."]
                        : []),
                    ]
                  : [
                      "Stash and restore local changes.",
                      "Check out feature; fetch/pull origin.",
                      "Run Claude with edit permissions.",
                      "Leave edits for inspection. No automatic commit or push.",
                    ],
        };
      }
      case "review.fix.state":
        return {
          target: params.target,
          state: {
            status: "succeeded",
            phase: this.fixPhase,
            commitSha: this.fixPhase === "readyToCommit" ? null : "c".repeat(40),
            summary: "Fixed null handling.",
            suggestedCommitMessage: "fix: guard null",
            filesTouched: ["src/app.ts"],
            tests: ["unit checks passed"],
          },
        };
      case "review.commit":
      case "review.sync":
      case "review.conflicts":
      case "review.push":
      case "review.fix": {
        const token = params.approvalToken as string;
        const approvedTarget = this.approvals.get(token);
        this.approvals.delete(token);
        const action =
          method === "review.commit"
            ? "commit"
            : method === "review.push"
              ? "push"
              : method === "review.sync"
                ? "sync"
                : method === "review.conflicts"
                  ? "conflicts"
                  : "fix";
        if (
          approvedTarget?.target !== JSON.stringify(params.target) ||
          approvedTarget.action !== action
        )
          throw new Error("Approval missing or target changed.");
        const operationId = `fake-${action}-${this.nextOperation++}`;
        if (action === "sync" || action === "conflicts") this.fixPhase = "readyToCommit";
        if (action === "commit") this.fixPhase = "readyToPush";
        if (action === "push") this.fixPhase = "completed";
        const target = params.target as TargetIdentity;
        this.operations.set(operationId, { target, state: "accepted", sequence: 0 });
        return { target, operationId, state: "accepted" };
      }
      case "review.reply": {
        const operationId = `fake-reply-${this.nextOperation++}`;
        const target = params.target as TargetIdentity;
        this.operations.set(operationId, { target, state: "accepted", sequence: 0 });
        return { target, operationId, state: "accepted" };
      }
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
