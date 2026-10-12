import type {
  PrRepositoryIdentity,
  RepositoryOperation,
  RepositoryOperationAction,
  RepositoryOperationBranch,
  RepositoryOperationCheckout,
  RepositoryOperationPrepared,
} from "../../../src/protocol/generated/protocol";
import type { BackendClient } from "../transport/backendClient";
import { sameLocalValue } from "../transport/localValidation";

export interface RepositoryOperationsState {
  repository: PrRepositoryIdentity | null;
  checkout: RepositoryOperationCheckout | null;
  branches: RepositoryOperationBranch[];
  offset: number;
  next: number | null;
  total: number;
  cursor: number;
  loading: boolean;
  busy: boolean;
  prepared: RepositoryOperationPrepared | null;
  expiresAt: number | null;
  operation: RepositoryOperation | null;
  cancelRequested: boolean;
  error: string | null;
}
const initial = (): RepositoryOperationsState => ({
  repository: null,
  checkout: null,
  branches: [],
  offset: 0,
  next: null,
  total: 0,
  cursor: 0,
  loading: false,
  busy: false,
  prepared: null,
  expiresAt: null,
  operation: null,
  cancelRequested: false,
  error: null,
});
const active = (operation: RepositoryOperation) =>
  ["starting", "checking", "running", "inspecting"].includes(operation.state);
const errorText = (error: unknown) =>
  error instanceof Error ? error.message : "Repository operation failed";

export class RepositoryOperationsStore {
  private state = initial();
  private generation = 0;
  private listeners = new Set<() => void>();
  private timer: ReturnType<typeof setTimeout> | null = null;
  private expiry: ReturnType<typeof setTimeout> | null = null;
  private unsubscribe: (() => void) | null = null;
  constructor(
    private client: BackendClient,
    private observed: (checkout: RepositoryOperationCheckout) => void | Promise<void> = () => {},
  ) {}
  getSnapshot = () => this.state;
  subscribe = (listener: () => void) => {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  };
  private update(patch: Partial<RepositoryOperationsState>) {
    this.state = { ...this.state, ...patch };
    for (const listener of this.listeners) listener();
  }
  private clearTimers() {
    if (this.timer) clearTimeout(this.timer);
    if (this.expiry) clearTimeout(this.expiry);
    this.timer = this.expiry = null;
  }
  dispose() {
    this.generation++;
    this.clearTimers();
    this.unsubscribe?.();
    this.unsubscribe = null;
  }
  async load(repository: PrRepositoryIdentity | null, available = true) {
    if (this.state.busy) return;
    this.dispose();
    this.state = initial();
    const generation = this.generation;
    this.update({ repository });
    this.unsubscribe = this.client.onExit(() => {
      if (generation !== this.generation) return;
      this.generation++;
      this.clearTimers();
      this.update({
        busy: false,
        loading: false,
        prepared: null,
        expiresAt: null,
        error:
          this.state.operation && active(this.state.operation)
            ? "Backend disconnected during Git execution. Its outcome is unknown. Restart and inspect actual Git state; no action will be replayed."
            : "Backend disconnected. Restart and refresh actual Git state; approvals are discarded.",
      });
    });
    if (!repository || !available) {
      this.update({
        error: repository
          ? "Repository operations require a matching backend."
          : "Select a configured repository first.",
      });
      return;
    }
    await this.refresh();
  }
  async refresh(offset = 0) {
    if (this.state.busy || this.state.loading || !this.state.repository) return;
    const generation = this.generation;
    const version = offset > 0 ? (this.state.checkout?.version ?? null) : null;
    this.clearTimers();
    this.update({ loading: true, prepared: null, expiresAt: null, error: null });
    try {
      if (offset > 0 && !version) throw new Error("Refresh the first branch page before paging.");
      const page =
        offset === 0
          ? await this.client.repositoryOperationsList({
              repository: this.state.repository,
              offset: 0,
              limit: 100,
              version: null,
            })
          : await this.client.repositoryOperationsList({
              repository: this.state.repository,
              offset,
              limit: 100,
              version: version!,
            });
      if (generation !== this.generation) return;
      this.update({
        checkout: page.checkout,
        branches: page.branches,
        offset,
        next: page.nextOffset,
        total: page.total,
        cursor: 0,
        loading: false,
      });
    } catch (error) {
      if (generation === this.generation) this.update({ loading: false, error: errorText(error) });
    }
  }
  move(delta: number) {
    if (this.state.loading || this.state.busy || this.state.prepared) return;
    this.update({
      cursor: Math.max(0, Math.min(this.state.branches.length - 1, this.state.cursor + delta)),
    });
  }
  async page(direction: number) {
    if (direction > 0 && this.state.next !== null) await this.refresh(this.state.next);
    else if (direction < 0 && this.state.offset > 0)
      await this.refresh(Math.max(0, this.state.offset - 100));
  }
  dismiss() {
    if (!this.state.busy && !this.state.loading) {
      this.clearTimers();
      this.update({ prepared: null, expiresAt: null });
    }
  }
  async preview(action: RepositoryOperationAction) {
    if (
      this.state.busy ||
      this.state.loading ||
      this.state.prepared ||
      !this.state.checkout?.context
    )
      return;
    const generation = this.generation,
      context = this.state.checkout.context;
    this.update({ loading: true, operation: null, error: null, cancelRequested: false });
    try {
      const prepared = await this.client.repositoryOperationsPreview({ context, action });
      if (generation !== this.generation || !sameLocalValue(this.state.checkout?.context, context))
        return;
      const expiresAt = Date.now() + prepared.expiresInMs;
      this.update({ loading: false, checkout: prepared.before, prepared, expiresAt });
      this.expiry = setTimeout(() => {
        if (generation === this.generation && this.state.prepared?.token === prepared.token)
          this.update({
            prepared: null,
            expiresAt: null,
            error: "Approval expired. Preview the action again.",
          });
      }, prepared.expiresInMs);
    } catch (error) {
      if (generation === this.generation) this.update({ loading: false, error: errorText(error) });
    }
  }
  async checkout() {
    const branch = this.state.branches[this.state.cursor];
    if (branch)
      await this.preview({
        kind: "checkout",
        branchKind: branch.kind,
        reference: branch.reference,
      });
  }
  async confirm() {
    const prepared = this.state.prepared;
    if (!prepared || this.state.busy || this.state.loading) return;
    if (!this.state.expiresAt || Date.now() >= this.state.expiresAt) {
      this.dismiss();
      this.update({ error: "Approval expired. Preview again." });
      return;
    }
    const generation = this.generation;
    this.clearTimers();
    this.update({ prepared: null, expiresAt: null, busy: true, error: null });
    try {
      const result = await this.client.repositoryOperationsApply(
        { ...prepared.request, token: prepared.token, confirm: true },
        prepared,
      );
      if (generation !== this.generation) return;
      this.update({ operation: result.operation });
      await this.poll(generation);
    } catch (error) {
      if (generation === this.generation)
        this.update({
          busy: false,
          error: `${errorText(error)}. Approval was consumed. Refresh actual state before another action.`,
        });
    }
  }
  private async poll(generation: number) {
    const previous = this.state.operation;
    if (generation !== this.generation || !previous || !this.state.busy) return;
    try {
      const result = await this.client.repositoryOperationsStatus(
        { context: previous.context, operationId: previous.operationId },
        previous,
      );
      if (generation !== this.generation) return;
      const operation = result.operation;
      this.update({ operation, cancelRequested: result.cancelRequested });
      if (active(operation)) {
        this.timer = setTimeout(() => {
          void this.poll(generation);
        }, 200);
        return;
      }
      this.update({ busy: false, checkout: operation.after ?? this.state.checkout });
      // Refresh actual options only in the full matching scope. Never recapture,
      // regenerate prompts, replay Git actions or clear staged review drafts.
      if (operation.after) await this.observed(operation.after);
      if (generation === this.generation) await this.refresh();
    } catch (error) {
      if (generation === this.generation) {
        this.clearTimers();
        this.update({
          busy: false,
          error: `${errorText(error)}. Outcome unknown; inspect actual Git state. No action is replayed.`,
        });
      }
    }
  }
  async cancel() {
    const operation = this.state.operation;
    if (!operation || !this.state.busy || this.state.cancelRequested) return;
    const generation = this.generation;
    this.update({ cancelRequested: true });
    try {
      await this.client.repositoryOperationsCancel(
        { context: operation.context, operationId: operation.operationId },
        operation,
      );
    } catch (error) {
      if (generation === this.generation) this.update({ error: errorText(error) });
    }
    // A concurrently in-flight status is allowed to precede cancellation. The
    // next poll remains authoritative; never overwrite newer final state.
  }
}
