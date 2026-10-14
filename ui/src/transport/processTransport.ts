import { readProtocolLines } from "./framing";
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
import {
  type BackendExit,
  type BackendOperationEvent,
  BackendRequestError,
  type BackendTransport,
  type ReadyInfo,
} from "./types";

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

export class ProcessBackendTransport implements BackendTransport {
  private readonly proc: Bun.Subprocess<"pipe", "pipe", "pipe">;
  private readonly pending = new Map<string, Pending>();
  private readonly eventListeners = new Set<(event: BackendOperationEvent) => void>();
  private readonly exitListeners = new Set<(exit: BackendExit) => void>();
  private nextId = 1;
  private closed = false;
  private exitNotified = false;
  private closing: Promise<void> | null = null;
  private readyResolve: ((info: ReadyInfo) => void) | null = null;
  private readyReject: ((error: Error) => void) | null = null;

  constructor(binary = resolveBackendBinary(), environment?: Record<string, string | undefined>) {
    this.proc = Bun.spawn([binary], {
      stdin: "pipe",
      stdout: "pipe",
      stderr: "pipe",
      env: environment,
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
        void this.close();
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
      const timer = setTimeout(
        () => {
          this.pending.delete(id);
          reject(new Error(`backend request ${method} timed out`));
        },
        method === "publication.prepare"
          ? 120_000
          : method === "publication.apply" ||
              method === "publication.reconcile" ||
              method === "comments.list"
            ? 90_000
            : [
                  "local.options",
                  "local.snapshot",
                  "local.snapshot.recheck",
                  "local.review.profiles",
                ].includes(method)
              ? // Native Git has a 30s budget; allow bounded SQLite/policy/codec
                // work too, rather than timing out as the Git budget expires.
                45_000
              : REQUEST_TIMEOUT_MS,
      );
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

  close(): Promise<void> {
    if (this.closing) return this.closing;
    if (this.closed) return Promise.resolve();
    this.closed = true;
    this.readyReject?.(new Error("backend transport closed"));
    this.failPending(new Error("backend transport closed"));
    this.closing = (async () => {
      const timer = setTimeout(() => {
        try {
          this.proc.kill("SIGKILL");
        } catch {
          /* Already exited. */
        }
      }, 5000);
      try {
        // Windows pipe writes can reject asynchronously. Await each step so
        // shutdown precedes EOF and a broken pipe cannot become an unhandled
        // rejection. Keep draining output and allow native guards to finish.
        try {
          await this.proc.stdin.write(
            encodeFrame({
              type: "request",
              id: `ui-close-${this.nextId++}`,
              method: "shutdown",
              params: {},
            }),
          );
          await this.proc.stdin.flush();
        } catch {
          /* The backend may already have closed its input. */
        }
        try {
          await this.proc.stdin.end();
        } catch {
          /* Already closed; the bounded reap below still owns the process. */
        }
        await this.proc.exited;
      } finally {
        clearTimeout(timer);
      }
    })();
    return this.closing;
  }

  private write(message: NornStdioProtocol): void {
    if (this.closed || !this.proc.stdin) {
      return;
    }
    void (async () => {
      try {
        await this.proc.stdin.write(encodeFrame(message));
        await this.proc.stdin.flush();
      } catch (error: unknown) {
        this.failPending(error instanceof Error ? error : new Error(String(error)));
      }
    })();
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
    try {
      for await (const line of readProtocolLines(this.proc.stdout)) {
        const frame = parseFrame(line);
        if (!frame || !["ready", "response", "event"].includes(frame.type)) {
          throw new Error("backend sent an invalid protocol frame");
        }
        this.dispatch(frame);
      }
    } catch (error) {
      // Never expose incoming payloads in diagnostics or dispatch an invalid
      // event. Reject work immediately and reap only our own subprocess.
      const failure = error instanceof Error ? error : new Error("backend protocol failed");
      this.readyReject?.(failure);
      this.failPending(failure);
      await this.close();
      this.notifyExit(await this.proc.exited);
      return;
    }
    // EOF rejects pending work immediately; report the actual reaped exit code.
    // An alive process which closed its protocol gets bounded native shutdown.
    this.readyReject?.(new Error("backend protocol ended before the handshake completed"));
    this.failPending(new Error("backend protocol ended"));
    await this.close();
    this.notifyExit(await this.proc.exited);
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
    frame: { ok: boolean; result?: unknown; error?: { message?: string; code?: string } },
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
      entry.reject(
        new BackendRequestError(
          frame.error?.code ?? "internal",
          frame.error?.message ?? "backend request failed",
        ),
      );
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
