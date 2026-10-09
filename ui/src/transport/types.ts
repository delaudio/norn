// Transport contract between the OpenTUI client and the Rust backend.

import type { TargetIdentity } from "./protocol";

export interface ReadyInfo {
  serverVersion: string;
  capabilities: string[];
}

export interface BackendOperationEvent {
  operationId: string;
  sequence: number;
  target: TargetIdentity;
  event: unknown;
}

export interface BackendExit {
  code: number | null;
}

export interface BackendTransport {
  /** Complete the handshake and resolve with the backend capabilities. */
  start(): Promise<ReadyInfo>;
  /** Send a request and resolve its result, rejecting on error/timeout/EOF. */
  request(method: string, params: Record<string, unknown>): Promise<Record<string, unknown>>;
  /** Subscribe to operation events; returns an unsubscribe function. */
  onOperationEvent(listener: (event: BackendOperationEvent) => void): () => void;
  /** Subscribe to backend exit; returns an unsubscribe function. */
  onExit(listener: (exit: BackendExit) => void): () => void;
  close(): void;
}
