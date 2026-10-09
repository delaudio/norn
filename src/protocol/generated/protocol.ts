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
    };
export type RequestId = string;
/**
 * Identity of the review target a request or event applies to.
 */
export type TargetIdentity = ProviderTarget | LocalTarget;
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
export interface ReviewStartParams {
  target: TargetIdentity;
  profile?: string | null;
}
export interface OperationControlParams {
  operationId?: string;
}
export interface TargetParams {
  target: TargetIdentity;
}
export interface EmptyResult {}
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
    | ReviewTargetsResult;
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
}
export interface ChangedFile {
  path: string;
  status: "added" | "modified" | "deleted" | "renamed" | "untracked";
  additions: number;
  deletions: number;
  oldPath?: string | null;
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
