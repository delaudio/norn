// Shell state for the OpenTUI workspace. UI state owns selection, focus and
// scroll; the backend owns persistent data and operations. Async results and
// events are fenced by target generation and operation id so a late response for
// a previously selected target can never be applied.

import type { BackendClient, RepositorySummary } from "../transport/backendClient";
import type { TargetIdentity } from "../transport/protocol";
import type { BackendOperationEvent } from "../transport/types";

export type ShellStatus = "idle" | "connecting" | "ready" | "error" | "closed";
export type ShellFocus = "repositories" | "detail";

export interface OperationLog {
  id: number;
  text: string;
}

export interface OperationView {
  id: string;
  state: string;
  sequence: number;
  logs: OperationLog[];
}

export interface ShellSnapshot {
  status: ShellStatus;
  error: string | null;
  serverVersion: string | null;
  repositories: RepositorySummary[];
  selected: number;
  focus: ShellFocus;
  operation: OperationView | null;
  generation: number;
}

const INITIAL: ShellSnapshot = {
  status: "idle",
  error: null,
  serverVersion: null,
  repositories: [],
  selected: 0,
  focus: "repositories",
  operation: null,
  generation: 0,
};

export function repositoryTarget(repo: RepositorySummary): TargetIdentity {
  return {
    kind: "provider",
    provider: repo.provider,
    workspace: repo.workspace,
    repo: repo.repo,
  } as TargetIdentity;
}

function message(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function describeEvent(event: unknown): string {
  if (event && typeof event === "object") {
    const record = event as Record<string, unknown>;
    if (record.kind === "progress" && typeof record.message === "string") {
      return record.message;
    }
    if (record.kind === "log" && typeof record.message === "string") {
      return record.message;
    }
    if (record.kind === "state" && typeof record.state === "string") {
      return `state: ${record.state}`;
    }
  }
  return "event";
}

function eventState(event: unknown): string | null {
  if (event && typeof event === "object") {
    const record = event as Record<string, unknown>;
    if (record.kind === "state" && typeof record.state === "string") {
      return record.state;
    }
  }
  return null;
}

export class ShellStore {
  private state: ShellSnapshot = INITIAL;
  private readonly listeners = new Set<() => void>();
  private nextLogId = 1;
  private starting = false;
  private pendingEvents: BackendOperationEvent[] = [];

  subscribe = (listener: () => void): (() => void) => {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  };

  getSnapshot = (): ShellSnapshot => this.state;

  private update(patch: Partial<ShellSnapshot>): void {
    this.state = { ...this.state, ...patch };
    for (const listener of this.listeners) {
      listener();
    }
  }

  get current(): RepositorySummary | undefined {
    return this.state.repositories[this.state.selected];
  }

  async connect(client: BackendClient): Promise<void> {
    this.update({ status: "connecting", error: null });
    try {
      const ready = await client.start();
      const repositories = await client.repositories();
      this.update({
        status: "ready",
        serverVersion: ready.serverVersion,
        repositories,
        selected: 0,
        generation: this.state.generation + 1,
      });
    } catch (error) {
      this.update({ status: "error", error: message(error) });
    }
  }

  move(delta: number): boolean {
    const length = this.state.repositories.length;
    if (length === 0) {
      return false;
    }
    const selected = (this.state.selected + delta + length) % length;
    if (selected === this.state.selected) {
      return false;
    }
    // Changing the selected target drops any operation bound to the previous
    // target so its late events and logs can never render under the new one.
    this.pendingEvents = [];
    this.update({
      selected,
      generation: this.state.generation + 1,
      operation: null,
    });
    return true;
  }

  setFocus(focus: ShellFocus): void {
    this.update({ focus });
  }

  toggleFocus(): void {
    this.setFocus(this.state.focus === "repositories" ? "detail" : "repositories");
  }

  async startReview(client: BackendClient): Promise<void> {
    const repo = this.current;
    if (!repo || this.starting) {
      return;
    }
    const active = this.state.operation;
    if (active && (active.state === "accepted" || active.state === "running")) {
      return;
    }
    const generation = this.state.generation;
    this.starting = true;
    try {
      const result = await client.startReview(repositoryTarget(repo));
      if (generation !== this.state.generation) {
        return;
      }
      this.update({
        operation: {
          id: result.operationId,
          state: result.state,
          sequence: result.sequence ?? 0,
          logs: [{ id: this.nextLogId++, text: `started ${result.operationId}` }],
        },
      });
      this.drainPendingEvents(result.operationId);
    } catch (error) {
      if (generation === this.state.generation) {
        this.update({ error: message(error) });
      }
    } finally {
      this.starting = false;
    }
  }

  applyEvent(event: BackendOperationEvent): void {
    const operation = this.state.operation;
    if (!operation || event.operationId !== operation.id) {
      // Events can arrive in the same transport read as the start response,
      // before the store registers the operation; buffer them briefly.
      if (this.pendingEvents.length < 256) {
        this.pendingEvents.push(event);
      }
      return;
    }
    this.applyEventToOperation(operation, event);
  }

  private applyEventToOperation(operation: OperationView, event: BackendOperationEvent): void {
    const logs = [
      ...operation.logs,
      { id: this.nextLogId++, text: describeEvent(event.event) },
    ].slice(-100);
    this.update({
      operation: {
        id: operation.id,
        state: eventState(event.event) ?? operation.state,
        sequence: event.sequence,
        logs,
      },
    });
  }

  private drainPendingEvents(operationId: string): void {
    const pending = this.pendingEvents;
    this.pendingEvents = [];
    for (const event of pending) {
      if (event.operationId === operationId) {
        this.applyEvent(event);
      }
    }
  }

  async cancel(client: BackendClient): Promise<void> {
    const operation = this.state.operation;
    if (!operation) {
      return;
    }
    const generation = this.state.generation;
    try {
      // The cancelled state is delivered as an operation event; the response is
      // only an acknowledgement, so it must not overwrite newer state.
      await client.operationCancel(operation.id);
    } catch (error) {
      if (generation === this.state.generation && this.state.operation?.id === operation.id) {
        this.setError(message(error));
      }
    }
  }

  clearError(): void {
    if (this.state.error !== null) {
      this.update({ error: null });
    }
  }

  markClosed(): void {
    this.update({ status: "closed" });
  }

  setError(error: string): void {
    this.update({ error });
  }
}
