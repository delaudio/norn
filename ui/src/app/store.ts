// Shell state for the OpenTUI workspace. UI state owns selection, focus and
// scroll; the backend owns persistent data and operations. Async results and
// events are fenced by target generation and request id so a late response for a
// previously selected target or file can never be applied.

import type {
  BackendClient,
  ChangedFile,
  FindingSummary,
  RepositorySummary,
  ReviewTarget,
} from "../transport/backendClient";
import type { TargetIdentity } from "../transport/protocol";
import type { BackendOperationEvent } from "../transport/types";

export type ShellStatus = "idle" | "connecting" | "ready" | "error" | "closed";
export type ShellFocus = "repositories" | "files" | "findings" | "diff";

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

export interface DiffView {
  path: string;
  text: string;
  truncated: boolean;
}

/// A row in the target picker: a working-tree target for each configured
/// repository, followed by stored PR review targets. Working-tree entries carry
/// no `prId`; review entries do.
export interface PickerEntry {
  kind: "working-tree" | "review";
  provider: "github" | "bitbucket";
  workspace: string;
  repo: string;
  prId: number | null;
  runId: string | null;
  title: string;
  status: string;
}

export interface ShellSnapshot {
  status: ShellStatus;
  error: string | null;
  notice: string | null;
  serverVersion: string | null;
  repositories: RepositorySummary[];
  selected: number;
  picker: PickerEntry[];
  selectedPicker: number;
  files: ChangedFile[];
  selectedFile: number;
  findings: FindingSummary[];
  selectedFinding: number;
  diff: DiffView | null;
  diffLoading: boolean;
  diffError: string | null;
  highlightLine: number | null;
  highlightSide: "old" | "new";
  scrollRequest: number;
  focus: ShellFocus;
  operation: OperationView | null;
  generation: number;
}

const INITIAL: ShellSnapshot = {
  status: "idle",
  error: null,
  notice: null,
  serverVersion: null,
  repositories: [],
  selected: 0,
  picker: [],
  selectedPicker: 0,
  files: [],
  selectedFile: 0,
  findings: [],
  selectedFinding: 0,
  diff: null,
  diffLoading: false,
  diffError: null,
  highlightLine: null,
  highlightSide: "new",
  scrollRequest: 0,
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

export function pickerTargetIdentity(entry: PickerEntry): TargetIdentity {
  if (entry.kind === "review" && entry.prId !== null) {
    return {
      kind: "provider",
      provider: entry.provider,
      workspace: entry.workspace,
      repo: entry.repo,
      prId: entry.prId,
      ...(entry.runId === null ? {} : { runId: entry.runId }),
    } as TargetIdentity;
  }
  return {
    kind: "provider",
    provider: entry.provider,
    workspace: entry.workspace,
    repo: entry.repo,
  } as TargetIdentity;
}

export function buildPicker(
  repositories: RepositorySummary[],
  targets: ReviewTarget[],
): PickerEntry[] {
  const workingTree: PickerEntry[] = repositories.map((repo) => ({
    kind: "working-tree",
    provider: repo.provider,
    workspace: repo.workspace,
    repo: repo.repo,
    prId: null,
    runId: null,
    title: "Working tree",
    status: "local",
  }));
  const reviews: PickerEntry[] = targets.map((target) => ({
    kind: "review",
    provider: target.provider,
    workspace: target.workspace,
    repo: target.repo,
    prId: target.prId,
    runId: target.runId,
    title: target.title,
    status: target.status,
  }));
  return [...workingTree, ...reviews];
}

function message(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function findFile<T>(
  items: T[],
  primary: (item: T) => boolean,
  fallback: (item: T) => boolean,
): number {
  const index = items.findIndex(primary);
  return index >= 0 ? index : items.findIndex(fallback);
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

/// Whether a line number on the given side actually appears in a unified diff.
/// Used to reject stale finding anchors instead of silently dropping them.
export function anchorPresent(text: string, line: number, side: "old" | "new"): boolean {
  let oldLine = 0;
  let newLine = 0;
  let inHunk = false;
  for (const raw of text.split("\n")) {
    const hunk = raw.match(/^@@ -(\d+)(?:,\d+)? \+(\d+)(?:,\d+)? @@/);
    if (hunk) {
      oldLine = Number.parseInt(hunk[1] ?? "0", 10);
      newLine = Number.parseInt(hunk[2] ?? "0", 10);
      inHunk = true;
      continue;
    }
    if (!inHunk || raw === "" || raw.startsWith("\\")) {
      continue;
    }
    if (raw.startsWith("-")) {
      if (side === "old" && oldLine === line) {
        return true;
      }
      oldLine += 1;
      continue;
    }
    if (raw.startsWith("+")) {
      if (side === "new" && newLine === line) {
        return true;
      }
      newLine += 1;
      continue;
    }
    if (raw.startsWith(" ")) {
      if ((side === "old" && oldLine === line) || (side === "new" && newLine === line)) {
        return true;
      }
      oldLine += 1;
      newLine += 1;
    }
  }
  return false;
}

export class ShellStore {
  private state: ShellSnapshot = INITIAL;
  private readonly listeners = new Set<() => void>();
  private nextLogId = 1;
  private starting = false;
  private pendingEvents: BackendOperationEvent[] = [];
  private diffRequest = 0;
  private diffInFlight = false;
  private diffPending = false;
  private snapshotInFlight = false;
  private snapshotPending = false;
  private client: BackendClient | null = null;

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

  /// The active backend target for the selected picker entry.
  private activeTarget(): TargetIdentity | null {
    const entry = this.state.picker[this.state.selectedPicker];
    return entry ? pickerTargetIdentity(entry) : null;
  }

  async connect(client: BackendClient): Promise<void> {
    this.client = client;
    this.update({ status: "connecting", error: null });
    try {
      const ready = await client.start();
      const [repositories, targets] = await Promise.all([
        client.repositories(),
        client.reviewTargets(),
      ]);
      this.update({
        status: "ready",
        serverVersion: ready.serverVersion,
        repositories,
        selected: 0,
        picker: buildPicker(repositories, targets),
        selectedPicker: 0,
        generation: this.state.generation + 1,
        files: [],
        findings: [],
        selectedFile: 0,
        selectedFinding: 0,
        diff: null,
        diffLoading: false,
        diffError: null,
        highlightLine: null,
        highlightSide: "new",
        scrollRequest: 0,
      });
      await this.loadSnapshot(client, this.state.generation);
    } catch (error) {
      this.update({ status: "error", error: message(error), diffLoading: false });
    }
  }

  /// Load the changed-file and finding snapshot for the selected target.
  async loadSnapshot(client: BackendClient, generation: number): Promise<void> {
    const target = this.activeTarget();
    if (!target) {
      return;
    }
    // The two requests are independent and both get a rejection handler
    // immediately, so a late failure never becomes an unhandled rejection. A
    // file-listing failure (for example a historical review whose revisions are
    // unavailable locally) must not hide findings the backend can still return.
    const filesOutcome = client.reviewFiles(target).then(
      (value) => ({ ok: true as const, value }),
      (error: unknown) => ({ ok: false as const, error }),
    );
    const findingsOutcome = client.reviewFindings(target).then(
      (value) => ({ ok: true as const, value }),
      (error: unknown) => ({ ok: false as const, error }),
    );
    const files = await filesOutcome;
    if (generation === this.state.generation) {
      if (files.ok) {
        this.update({ files: files.value, selectedFile: 0, notice: null });
      } else {
        this.update({ files: [], error: message(files.error) });
      }
    }
    const findings = await findingsOutcome;
    if (generation === this.state.generation) {
      if (findings.ok) {
        this.update({ findings: findings.value, selectedFinding: 0 });
      } else {
        this.update({ error: message(findings.error) });
      }
    }
    if (generation !== this.state.generation) {
      return;
    }
    // Keep the focused pane consistent: resolve the selected finding when the
    // findings pane is active, otherwise show the selected file's diff.
    if (this.state.focus === "findings") {
      const started = this.jumpToFinding();
      if (!started) {
        await this.loadDiff(client);
      }
    } else {
      await this.loadDiff(client);
    }
  }

  async selectRepository(selected: number): Promise<void> {
    await this.selectPicker(selected, selected);
  }

  async selectPicker(selectedPicker: number, selected?: number): Promise<void> {
    const generation = this.state.generation + 1;
    this.update({
      selectedPicker,
      selected: selected ?? this.state.selected,
      generation,
      files: [],
      findings: [],
      selectedFile: 0,
      selectedFinding: 0,
      diff: null,
      diffLoading: false,
      diffError: null,
      highlightLine: null,
      highlightSide: "new",
      scrollRequest: 0,
      operation: null,
      notice: null,
      error: null,
    });
    if (!this.client) {
      return;
    }
    // Coalesce target navigation: at most one snapshot load runs at a time, and
    // a newer selection replaces any queued follow-up.
    if (this.snapshotInFlight) {
      this.snapshotPending = true;
      return;
    }
    this.snapshotInFlight = true;
    try {
      let target = generation;
      // Drain queued selections inside the guard so the follow-up reload cannot
      // race a newly started one.
      while (true) {
        await this.loadSnapshot(this.client, target);
        if (!this.snapshotPending) {
          break;
        }
        this.snapshotPending = false;
        target = this.state.generation;
      }
    } finally {
      this.snapshotInFlight = false;
    }
  }

  /// Fetch the selected file's diff with at most one request in flight. Rapid
  /// navigation coalesces into a single pending follow-up for the latest
  /// selection instead of starting (and then discarding) one request per move.
  private async loadDiff(client: BackendClient): Promise<void> {
    if (this.diffInFlight) {
      // Invalidate the in-flight result so it cannot overwrite the newer
      // selection, and coalesce the follow-up into one pending fetch.
      this.diffRequest += 1;
      this.diffPending = true;
      return;
    }
    const target = this.activeTarget();
    const file = this.state.files[this.state.selectedFile];
    if (!target || !file) {
      this.update({ diff: null, diffLoading: false, diffError: null });
      return;
    }
    const request = ++this.diffRequest;
    const generation = this.state.generation;
    this.diffInFlight = true;
    this.update({ diffLoading: true, diffError: null });
    try {
      const result = await client.diffFile(target, file.path, { oldPath: file.oldPath });
      if (request === this.diffRequest && generation === this.state.generation) {
        this.update({
          diff: { path: file.path, text: result.diff, truncated: result.truncated },
          diffLoading: false,
          diffError: null,
        });
        const highlight = this.state.highlightLine;
        if (highlight != null) {
          if (anchorPresent(result.diff, highlight, this.state.highlightSide)) {
            this.update({ scrollRequest: this.state.scrollRequest + 1 });
          } else {
            this.update({
              highlightLine: null,
              notice: `Anchor line ${highlight} is not in the current diff.`,
            });
          }
        }
      }
    } catch (error) {
      if (request === this.diffRequest && generation === this.state.generation) {
        this.update({ diffLoading: false, diff: null, diffError: message(error) });
      }
    } finally {
      this.diffInFlight = false;
      if (this.diffPending) {
        this.diffPending = false;
        await this.loadDiff(client);
      }
    }
  }

  move(delta: number): boolean {
    if (this.state.focus === "repositories") {
      const length = this.state.picker.length;
      if (length === 0) {
        return false;
      }
      const selectedPicker = (this.state.selectedPicker + delta + length) % length;
      if (selectedPicker === this.state.selectedPicker) {
        return false;
      }
      // Keep `selected` aligned while the entry is a working-tree repository so
      // repository-scoped mistakes never point at the wrong row.
      const selected = selectedPicker < this.state.repositories.length ? selectedPicker : undefined;
      this.pendingEvents = [];
      void this.selectPicker(selectedPicker, selected);
      return true;
    }
    if (this.state.focus === "files") {
      const length = this.state.files.length;
      if (length === 0) {
        return false;
      }
      const selectedFile = (this.state.selectedFile + delta + length) % length;
      if (selectedFile === this.state.selectedFile) {
        return false;
      }
      this.update({ selectedFile, highlightLine: null, highlightSide: "new", notice: null });
      if (this.client) {
        void this.loadDiff(this.client);
      }
      return true;
    }
    const length = this.state.findings.length;
    if (length === 0) {
      return false;
    }
    const selectedFinding = (this.state.selectedFinding + delta + length) % length;
    this.update({ selectedFinding });
    this.jumpToFinding();
    return true;
  }

  /// Move selection (and the diff) to the selected finding's anchor without
  /// guessing: a missing target file or a null/outdated anchor surfaces a notice.
  /// Returns true when it started a diff load for the anchor file.
  private jumpToFinding(): boolean {
    const finding = this.state.findings[this.state.selectedFinding];
    if (!finding) {
      return false;
    }
    if (!finding.anchor) {
      this.update({
        highlightLine: null,
        highlightSide: "new",
        notice: `Finding ${finding.id} has no anchor.`,
      });
      return false;
    }
    const anchor = finding.anchor;
    const side = anchor.side ?? "new";
    // A side-specific match avoids selecting a recreated file: an old-side
    // anchor belongs to the rename source, a new-side anchor to the destination.
    const index =
      side === "old"
        ? findFile(
            this.state.files,
            (file) => file.oldPath === anchor.path,
            (file) => file.path === anchor.path,
          )
        : findFile(
            this.state.files,
            (file) => file.path === anchor.path,
            (file) => file.oldPath === anchor.path,
          );
    if (index < 0) {
      this.update({
        highlightLine: null,
        highlightSide: "new",
        notice: `Anchor file is not in the current diff: ${anchor.path}.`,
      });
      return false;
    }
    this.update({
      selectedFile: index,
      highlightLine: anchor.startLine,
      highlightSide: side,
      notice: null,
    });
    if (this.client) {
      void this.loadDiff(this.client);
      return true;
    }
    return false;
  }

  cycleFocus(): void {
    const order: ShellFocus[] = ["repositories", "files", "findings", "diff"];
    const index = order.indexOf(this.state.focus);
    const next = order[(index + 1) % order.length] ?? "repositories";
    this.update({ focus: next });
    // Resolve the selected finding when the finding pane gains focus so the
    // selection and the diff agree before the first navigation key.
    if (next === "findings") {
      this.jumpToFinding();
    }
  }

  async startReview(client: BackendClient): Promise<void> {
    const target = this.activeTarget();
    if (!target || this.starting) {
      return;
    }
    const active = this.state.operation;
    if (active && (active.state === "accepted" || active.state === "running")) {
      return;
    }
    const generation = this.state.generation;
    this.starting = true;
    try {
      const result = await client.startReview(target);
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
