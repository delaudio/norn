// Production transport: a persistent `norn-backend` subprocess speaking the
// versioned stdio protocol. The executable is resolved by path and launched with
// an argument array (never a shell); protocol input is written to the child's
// stdin, separate from terminal input.

import {
  encodeFrame,
  type NornStdioProtocol,
  PROTOCOL_VERSION,
  parseFrame,
  type TargetIdentity,
} from "./protocol";
import type { BackendExit, BackendOperationEvent, BackendTransport, ReadyInfo } from "./types";

const MAX_PENDING = 64;
const HANDSHAKE_TIMEOUT_MS = 5_000;
const REQUEST_TIMEOUT_MS = 30_000;

export function resolveBackendBinary(): string {
  const explicit = process.env.NORN_BACKEND_BIN;
  if (explicit && explicit.length > 0) {
    return explicit;
  }
  const found = Bun.which("norn-backend");
  if (!found) {
    throw new Error("norn-backend was not found on PATH; set NORN_BACKEND_BIN.");
  }
  return found;
}

interface Pending {
  resolve: (value: Record<string, unknown>) => void;
  reject: (error: Error) => void;
  timer: ReturnType<typeof setTimeout>;
}

async function* readLines(stream: ReadableStream<Uint8Array>): AsyncGenerator<string, void, void> {
  const reader = stream.getReader();
  const decoder = new TextDecoder();
  let buffer = "";
  try {
    while (true) {
      const { done, value } = await reader.read();
      if (done) {
        break;
      }
      buffer += decoder.decode(value, { stream: true });
      let index = buffer.indexOf("\n");
      while (index >= 0) {
        yield buffer.slice(0, index);
        buffer = buffer.slice(index + 1);
        index = buffer.indexOf("\n");
      }
    }
    if (buffer.length > 0) {
      yield buffer;
    }
  } finally {
    reader.releaseLock();
  }
}

export class ProcessBackendTransport implements BackendTransport {
  private readonly proc: Bun.Subprocess<"pipe", "pipe", "pipe">;
  private readonly pending = new Map<string, Pending>();
  private readonly eventListeners = new Set<(event: BackendOperationEvent) => void>();
  private readonly exitListeners = new Set<(exit: BackendExit) => void>();
  private nextId = 1;
  private closed = false;
  private exitNotified = false;
  private readyResolve: ((info: ReadyInfo) => void) | null = null;
  private readyReject: ((error: Error) => void) | null = null;

  constructor(binary = resolveBackendBinary()) {
    this.proc = Bun.spawn([binary], {
      stdin: "pipe",
      stdout: "pipe",
      stderr: "pipe",
    });
    void this.readLoop();
    void this.drainStderr();
    void this.exitLoop();
  }

  start(): Promise<ReadyInfo> {
    if (this.closed) {
      return Promise.reject(new Error("backend transport is closed"));
    }
    this.write({
      type: "hello",
      protocolVersion: PROTOCOL_VERSION,
      client: { name: "norn-ui", version: "0.1.0" },
    });
    return new Promise<ReadyInfo>((resolve, reject) => {
      const timer = setTimeout(() => {
        this.readyResolve = null;
        this.readyReject = null;
        reject(new Error("backend handshake timed out"));
      }, HANDSHAKE_TIMEOUT_MS);
      this.readyResolve = (info) => {
        clearTimeout(timer);
        this.readyResolve = null;
        this.readyReject = null;
        resolve(info);
      };
      this.readyReject = (error) => {
        clearTimeout(timer);
        this.readyResolve = null;
        this.readyReject = null;
        reject(error);
      };
    });
  }

  request(method: string, params: Record<string, unknown>): Promise<Record<string, unknown>> {
    if (this.closed) {
      return Promise.reject(new Error("backend transport is closed"));
    }
    if (this.pending.size >= MAX_PENDING) {
      return Promise.reject(new Error("too many pending backend requests"));
    }
    const id = `ui-${this.nextId++}`;
    return new Promise<Record<string, unknown>>((resolve, reject) => {
      const timer = setTimeout(() => {
        this.pending.delete(id);
        reject(new Error(`backend request ${method} timed out`));
      }, REQUEST_TIMEOUT_MS);
      this.pending.set(id, { resolve, reject, timer });
      this.write({ type: "request", id, method, params } as unknown as NornStdioProtocol);
    });
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
    this.failPending(new Error("backend transport closed"));
    try {
      this.proc.kill();
    } catch {
      // Ignore: the process may have already exited.
    }
  }

  private write(message: NornStdioProtocol): void {
    if (this.closed || !this.proc.stdin) {
      return;
    }
    try {
      this.proc.stdin.write(encodeFrame(message));
      void Promise.resolve(this.proc.stdin.flush()).catch((error: unknown) => {
        this.failPending(error instanceof Error ? error : new Error(String(error)));
      });
    } catch (error) {
      this.failPending(error instanceof Error ? error : new Error(String(error)));
    }
  }

  /// Drain backend stderr so a full pipe can never block the backend. Backend
  /// diagnostics are diagnostic only and never carry protocol frames.
  private async drainStderr(): Promise<void> {
    if (!this.proc.stderr) {
      return;
    }
    const reader = this.proc.stderr.getReader();
    const decoder = new TextDecoder();
    try {
      while (true) {
        const { done, value } = await reader.read();
        if (done) {
          break;
        }
        process.stderr.write(decoder.decode(value, { stream: true }));
      }
    } catch {
      // The pipe closed; nothing to drain.
    } finally {
      reader.releaseLock();
    }
  }

  private async readLoop(): Promise<void> {
    if (!this.proc.stdout) {
      return;
    }
    for await (const line of readLines(this.proc.stdout)) {
      const frame = parseFrame(line);
      if (!frame) {
        continue;
      }
      this.dispatch(frame);
    }
    // The protocol stream ended: the backend is gone. Reap it and reject
    // anything still pending instead of waiting for a timeout.
    this.notifyExit(null);
    try {
      this.proc.kill();
    } catch {
      // Already exited.
    }
  }

  private dispatch(frame: NornStdioProtocol): void {
    switch (frame.type) {
      case "ready":
        if (frame.protocolVersion !== PROTOCOL_VERSION) {
          this.readyReject?.(
            new Error(
              `backend speaks protocol ${frame.protocolVersion}, expected ${PROTOCOL_VERSION}`,
            ),
          );
          this.close();
          return;
        }
        this.readyResolve?.({
          serverVersion: frame.serverVersion,
          capabilities: frame.capabilities,
        });
        return;
      case "response":
        this.settle(frame.id, frame);
        return;
      case "event":
        for (const listener of this.eventListeners) {
          listener({
            operationId: frame.operationId,
            sequence: frame.sequence,
            target: frame.target as TargetIdentity,
            event: frame.event,
          });
        }
        return;
      default:
        return;
    }
  }

  private settle(
    id: string,
    frame: { ok: boolean; result?: unknown; error?: { message?: string } },
  ): void {
    const entry = this.pending.get(id);
    if (!entry) {
      return;
    }
    this.pending.delete(id);
    clearTimeout(entry.timer);
    if (frame.ok === true) {
      entry.resolve((frame.result as Record<string, unknown>) ?? {});
    } else {
      entry.reject(new Error(frame.error?.message ?? "backend request failed"));
    }
  }

  private async exitLoop(): Promise<void> {
    const code = await this.proc.exited;
    this.notifyExit(code);
  }

  /// Idempotent termination: mark closed, reject pending work and notify exit
  /// listeners exactly once.
  private notifyExit(code: number | null): void {
    if (this.exitNotified) {
      return;
    }
    this.exitNotified = true;
    this.closed = true;
    this.readyReject?.(new Error("backend terminated before the handshake completed"));
    this.failPending(new Error("backend terminated"));
    for (const listener of this.exitListeners) {
      listener({ code });
    }
  }

  private failPending(error: Error): void {
    for (const [, entry] of this.pending) {
      clearTimeout(entry.timer);
      entry.reject(error);
    }
    this.pending.clear();
  }
}
