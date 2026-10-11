// Shell state for the OpenTUI workspace. UI state owns selection, focus and
// scroll; the backend owns persistent data and operations. Async results and
// events are fenced by target generation and request id so a late response for a
// previously selected target or file can never be applied.

import type {
  BackendClient,
  ChangedFile,
  FindingSummary,
  FixPreview,
  FixState,
  RepositorySummary,
  ReviewRunSummary,
  ReviewTarget,
  ReviewThreadSummary,
} from "../transport/backendClient";
import type { TargetIdentity } from "../transport/protocol";
import type { BackendOperationEvent } from "../transport/types";

export type ShellStatus = "idle" | "connecting" | "ready" | "error" | "closed";
export type ShellFocus = "repositories" | "files" | "findings" | "diff" | "threads";
export type FindingGroup = "none" | "file" | "severity";
export type FindingFilter = "all" | FindingSummary["severity"];

const SEVERITIES: FindingFilter[] = ["all", "critical", "high", "medium", "low", "info"];

export function visibleFindings(state: ShellSnapshot): FindingSummary[] {
  const filtered = state.findings.filter(
    (finding) => state.findingFilter === "all" || finding.severity === state.findingFilter,
  );
  if (state.findingGroup === "file") {
    filtered.sort((a, b) =>
      (a.anchor?.path ?? "~unanchored").localeCompare(b.anchor?.path ?? "~unanchored"),
    );
  } else if (state.findingGroup === "severity") {
    filtered.sort((a, b) => SEVERITIES.indexOf(a.severity) - SEVERITIES.indexOf(b.severity));
  }
  return filtered;
}

export interface OperationLog {
  id: number;
  text: string;
}

export interface OperationView {
  id: string;
  state: string;
  sequence: number;
  logs: OperationLog[];
  targetLabel: string;
  reply: boolean;
}

const GraphemeSegmenter = (
  Intl as unknown as {
    Segmenter?: new () => { segment(text: string): Iterable<{ segment: string }> };
  }
).Segmenter;

function graphemes(text: string): string[] {
  if (GraphemeSegmenter) {
    return [...new GraphemeSegmenter().segment(text)].map((entry) => entry.segment);
  }
  return Array.from(text);
}

function targetLabel(target: TargetIdentity): string {
  if (target.kind === "local") {
    return "local";
  }
  const provider = target as { workspace?: string; repo?: string; prId?: number };
  const base = `${provider.workspace ?? ""}/${provider.repo ?? ""}`;
  return provider.prId === undefined ? base : `${base} #${provider.prId}`;
}

export interface FilePreviewView {
  path: string;
  mimeType: string;
  size: number;
  dataBase64: string;
}

const IMAGE_EXTENSIONS = [".png", ".jpg", ".jpeg", ".gif", ".webp", ".bmp", ".svg"];

export function isImagePath(path: string): boolean {
  const normalized = path.toLowerCase();
  return IMAGE_EXTENSIONS.some((extension) => normalized.endsWith(extension));
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
  findingFilter: FindingFilter;
  findingGroup: FindingGroup;
  selectedFinding: number;
  history: ReviewRunSummary[];
  threads: ReviewThreadSummary[];
  selectedThread: number;
  conversationOpen: boolean;
  diff: DiffView | null;
  diffLoading: boolean;
  diffError: string | null;
  highlightLine: number | null;
  highlightSide: "old" | "new";
  scrollRequest: number;
  diffMode: "unified" | "split";
  fixPreview: FixPreview | null;
  fixPreviewLoading: boolean;
  fixState: FixState | null;
  composerKind: "reply" | "commit";
  composing: boolean;
  composerText: string;
  diffExpanded: boolean;
  diffLineCount: number;
  preview: FilePreviewView | null;
  previewError: string | null;
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
  findingFilter: "all",
  findingGroup: "none",
  selectedFinding: 0,
  history: [],
  threads: [],
  selectedThread: 0,
  conversationOpen: false,
  diff: null,
  diffLoading: false,
  diffError: null,
  highlightLine: null,
  highlightSide: "new",
  scrollRequest: 0,
  diffMode: "unified",
  fixPreview: null,
  fixPreviewLoading: false,
  fixState: null,
  composerKind: "reply",
  composing: false,
  composerText: "",
  diffExpanded: false,
  diffLineCount: 0,
  preview: null,
  previewError: null,
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
  private previewRequest = 0;
  private replying = false;
  private replyGeneration = -1;
  private diffInFlight = false;
  private diffPending = false;
  private snapshotInFlight = false;
  private snapshotPending = false;
  private client: BackendClient | null = null;
  private fixing = false;
  private fixOperation: { id: string; generation: number; target: TargetIdentity } | null = null;

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
        const previousId = visibleFindings(this.state)[this.state.selectedFinding]?.id;
        this.update({ findings: findings.value });
        const index = visibleFindings(this.state).findIndex((finding) => finding.id === previousId);
        this.update({ selectedFinding: Math.max(0, index) });
      } else {
        this.update({ error: message(findings.error) });
      }
    }
    try {
      const [history, threads] = await Promise.all([
        client.reviewHistory(target),
        client.reviewThreads(target),
      ]);
      if (generation === this.state.generation) {
        const thread = threads.findIndex(
          (item) =>
            item.selected ||
            item.latestRunId === (target.kind === "provider" ? target.runId : null),
        );
        this.update({ history, threads, selectedThread: Math.max(0, thread) });
      }
    } catch {
      if (generation === this.state.generation) {
        this.update({ history: [], threads: [] });
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
    void this.loadPreview(client);
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
      history: [],
      threads: [],
      selectedThread: 0,
      fixPreview: null,
      fixPreviewLoading: false,
      fixState: null,
      composerKind: "reply",
      composing: false,
      composerText: "",
      diff: null,
      diffLoading: false,
      diffError: null,
      highlightLine: null,
      highlightSide: "new",
      scrollRequest: 0,
      preview: null,
      previewError: null,
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
      this.update({
        diff: null,
        diffLoading: false,
        diffError: null,
        preview: null,
        previewError: null,
      });
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
          diffExpanded: false,
          diffLineCount: result.diff.split("\n").length,
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
    if (this.state.focus === "threads") {
      const length = this.state.threads.length;
      if (!length) return false;
      this.update({ selectedThread: (this.state.selectedThread + delta + length) % length });
      return true;
    }
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
        void this.loadPreview(this.client);
      }
      return true;
    }
    const length = visibleFindings(this.state).length;
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
    const finding = visibleFindings(this.state)[this.state.selectedFinding];
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
      void this.loadPreview(this.client);
      return true;
    }
    return false;
  }

  toggleDiffMode(): void {
    this.update({ diffMode: this.state.diffMode === "unified" ? "split" : "unified" });
  }

  toggleDiffExpanded(): void {
    this.update({ diffExpanded: !this.state.diffExpanded });
  }

  private preserveFindingSelection(patch: Partial<ShellSnapshot>): void {
    const id = visibleFindings(this.state)[this.state.selectedFinding]?.id;
    this.update(patch);
    const next = visibleFindings(this.state);
    this.update({
      selectedFinding: Math.max(
        0,
        next.findIndex((finding) => finding.id === id),
      ),
    });
    if (!next.length) {
      this.update({ highlightLine: null, notice: "No findings match the severity filter." });
    } else if (this.state.focus === "findings") {
      this.jumpToFinding();
    }
  }

  cycleFindingFilter(): void {
    this.preserveFindingSelection({
      findingFilter:
        SEVERITIES[(SEVERITIES.indexOf(this.state.findingFilter) + 1) % SEVERITIES.length] ?? "all",
    });
  }

  cycleFindingGroup(): void {
    const order: FindingGroup[] = ["none", "file", "severity"];
    this.preserveFindingSelection({
      findingGroup: order[(order.indexOf(this.state.findingGroup) + 1) % order.length] ?? "none",
    });
  }

  toggleThreads(): void {
    if (!this.state.threads.length) {
      this.update({ notice: "No saved conversation threads for this target." });
      return;
    }
    this.update({
      focus: this.state.focus === "threads" ? "diff" : "threads",
      conversationOpen: true,
    });
  }

  toggleConversation(): void {
    this.update({ conversationOpen: !this.state.conversationOpen });
  }

  async selectThread(): Promise<void> {
    const thread = this.state.threads[this.state.selectedThread];
    if (!thread?.latestRunId || !this.client) {
      this.update({ notice: "This thread has no stored review run to inspect or reply to." });
      return;
    }
    const entry = this.state.picker[this.state.selectedPicker];
    if (entry?.kind !== "review") return;
    const picker = this.state.picker.map((row, index) =>
      index === this.state.selectedPicker ? { ...row, runId: thread.latestRunId } : row,
    );
    this.update({ picker });
    await this.selectPicker(this.state.selectedPicker);
    this.update({ conversationOpen: true });
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
          targetLabel: targetLabel(target),
          reply: false,
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
    const state = eventState(event.event) ?? operation.state;
    this.update({
      operation: {
        id: operation.id,
        state,
        sequence: event.sequence,
        logs,
        targetLabel: operation.targetLabel,
        reply: operation.reply,
      },
    });
    const fix = this.fixOperation;
    if (
      fix?.id === operation.id &&
      fix.generation === this.state.generation &&
      (state === "succeeded" || state === "failed" || state === "cancelled")
    ) {
      void this.refreshFixState(fix);
    }
    if (operation.reply && this.replyGeneration === this.state.generation) {
      if (state === "succeeded") {
        this.update({ composerText: "" });
      } else if (state === "failed" || state === "cancelled") {
        // Restore the submitted draft so it is not lost on failure.
        this.update({ composing: true });
      }
    }
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

  /// Load the bounded new-side image preview for the selected file, or clear it.
  private async loadPreview(client: BackendClient): Promise<void> {
    const request = ++this.previewRequest;
    const target = this.activeTarget();
    const file = this.state.files[this.state.selectedFile];
    if (!target || !file || !isImagePath(file.path)) {
      this.update({ preview: null, previewError: null });
      return;
    }
    const generation = this.state.generation;
    this.update({ preview: null, previewError: null });
    const current = () =>
      request === this.previewRequest &&
      generation === this.state.generation &&
      this.state.files[this.state.selectedFile]?.path === file.path;
    try {
      const result = await client.filePreview(target, file.path);
      if (current()) {
        this.update({ preview: result, previewError: null });
      }
    } catch (error) {
      if (current()) {
        this.update({ preview: null, previewError: message(error) });
      }
    }
  }

  setPreviewError(error: string): void {
    this.update({ preview: null, previewError: error });
  }

  beginReply(): void {
    if (this.replying) {
      return;
    }
    const active = this.state.operation;
    if (active && (active.state === "accepted" || active.state === "running")) {
      this.update({ notice: "Another review operation is already in progress." });
      return;
    }
    const entry = this.state.picker[this.state.selectedPicker];
    if (entry?.kind !== "review") {
      this.update({ notice: "Replies are available for pull-request reviews." });
      return;
    }
    if (
      this.state.focus === "threads" &&
      this.state.threads[this.state.selectedThread]?.latestRunId !== entry.runId
    ) {
      this.update({ notice: "Press Enter to load the selected thread before replying." });
      return;
    }
    this.update({
      composing: true,
      composerKind: "reply",
      composerText: this.state.composerKind === "reply" ? this.state.composerText : "",
    });
  }

  cancelReply(): void {
    this.update({ composing: false, composerText: "" });
  }

  appendReplyText(text: string): void {
    const next = graphemes(this.state.composerText + text)
      .slice(0, 4000)
      .join("");
    this.update({ composerText: next });
  }

  backspaceReply(): void {
    const characters = graphemes(this.state.composerText);
    characters.pop();
    this.update({ composerText: characters.join("") });
  }

  async submitReply(client: BackendClient): Promise<void> {
    const replyMessage = this.state.composerText.trim();
    if (replyMessage.length === 0) {
      this.update({ notice: "Reply message is empty." });
      return;
    }
    const activeOperation = this.state.operation;
    if (
      this.replying ||
      (activeOperation !== null &&
        (activeOperation.state === "accepted" || activeOperation.state === "running"))
    ) {
      return;
    }
    const target = this.activeTarget();
    if (!target) {
      return;
    }
    const generation = this.state.generation;
    this.replying = true;
    this.replyGeneration = generation;
    // Close the composer while the request is in flight; restore the draft on failure.
    this.update({ composing: false });
    try {
      const result = await client.reviewReply(target, replyMessage);
      if (generation !== this.state.generation) {
        return;
      }
      this.update({
        operation: {
          id: result.operationId,
          state: result.state,
          sequence: result.sequence ?? 0,
          logs: [{ id: this.nextLogId++, text: `reply ${result.operationId}` }],
          targetLabel: targetLabel(target),
          reply: true,
        },
      });
      this.drainPendingEvents(result.operationId);
    } catch (error) {
      if (generation === this.state.generation) {
        this.update({ composing: true, error: message(error) });
      }
    } finally {
      this.replying = false;
    }
  }

  /// Start (or refresh) the authenticated browser diff session and surface its
  /// URL in the notice line.
  async previewFix(client: BackendClient): Promise<void> {
    await this.previewMutation(client, "fix");
  }

  private loadedConversationSelected(): boolean {
    return (
      this.state.focus !== "threads" ||
      this.state.threads[this.state.selectedThread]?.latestRunId ===
        this.state.picker[this.state.selectedPicker]?.runId
    );
  }

  async beginCommit(client: BackendClient): Promise<void> {
    if (!this.loadedConversationSelected()) {
      this.update({ notice: "Press Enter to load the selected thread before committing." });
      return;
    }
    const target = this.activeTarget();
    if (
      !target ||
      (this.state.operation && ["accepted", "running"].includes(this.state.operation.state))
    )
      return;
    const generation = this.state.generation;
    try {
      const result = await client.fixState(target);
      if (generation !== this.state.generation) return;
      if (result?.phase !== "readyToCommit" || result.status !== "succeeded") {
        this.update({ notice: "Complete a successful fix before committing." });
        return;
      }
      this.update({
        fixState: result,
        composerKind: "commit",
        composing: true,
        composerText:
          this.state.composerKind === "commit" && this.state.composerText
            ? this.state.composerText
            : (result.suggestedCommitMessage ?? ""),
      });
    } catch (error) {
      if (generation === this.state.generation) this.update({ error: message(error) });
    }
  }

  async previewCommit(client: BackendClient): Promise<void> {
    const text = this.state.composerText.trim();
    if (!text || Array.from(text).length > 4096) {
      this.update({ notice: "Enter a commit message between 1 and 4096 characters." });
      return;
    }
    const generation = this.state.generation;
    const approved = await this.previewMutation(client, "commit", text);
    if (approved && generation === this.state.generation) this.update({ composing: false });
  }

  async previewPush(client: BackendClient): Promise<void> {
    await this.previewMutation(client, "push");
  }

  private async previewMutation(
    client: BackendClient,
    action: "fix" | "commit" | "push",
    commitMessage?: string,
  ): Promise<boolean> {
    if (this.fixing || this.state.fixPreviewLoading) return false;
    const operation = this.state.operation;
    if (operation && ["accepted", "running"].includes(operation.state)) {
      this.update({ notice: "Wait for the active operation before requesting a fix." });
      return false;
    }
    if (!this.loadedConversationSelected()) {
      this.update({
        notice: "Press Enter to load the selected thread before changing the checkout.",
      });
      return false;
    }
    const target = this.activeTarget();
    if (target?.kind !== "provider" || !target.prId || !target.runId) {
      this.update({ notice: "Select a saved pull-request review before requesting a fix." });
      return false;
    }
    const generation = this.state.generation;
    this.update({ fixPreview: null, fixPreviewLoading: true, error: null });
    try {
      const preview =
        action === "commit"
          ? await client.commitPreview(target, commitMessage ?? "")
          : action === "push"
            ? await client.pushPreview(target)
            : await client.fixPreview(target);
      if (generation === this.state.generation) {
        this.update({ fixPreview: preview });
        return true;
      }
    } catch (error) {
      if (generation === this.state.generation) this.update({ error: message(error) });
    } finally {
      if (generation === this.state.generation) this.update({ fixPreviewLoading: false });
    }
    return false;
  }

  dismissFix(): void {
    this.update({ fixPreview: null });
  }

  async confirmFix(client: BackendClient): Promise<void> {
    const preview = this.state.fixPreview;
    if (!preview || this.fixing) return;
    const generation = this.state.generation;
    this.fixing = true;
    // Consume the UI approval immediately. A timeout never triggers a retry.
    this.update({ fixPreview: null, fixState: null });
    try {
      const result =
        preview.action === "commit"
          ? await client.reviewCommit(preview.target, preview.approvalToken)
          : preview.action === "push"
            ? await client.reviewPush(preview.target, preview.approvalToken)
            : await client.reviewFix(preview.target, preview.approvalToken);
      if (generation !== this.state.generation) return;
      this.fixOperation = { id: result.operationId, generation, target: preview.target };
      this.update({
        operation: {
          id: result.operationId,
          state: result.state,
          sequence: result.sequence ?? 0,
          logs: [{ id: this.nextLogId++, text: `Approved ${preview.action ?? "fix"} accepted.` }],
          targetLabel: targetLabel(preview.target),
          reply: false,
        },
      });
      this.drainPendingEvents(result.operationId);
    } catch (error) {
      if (generation === this.state.generation)
        this.update({
          error: message(error),
          notice:
            "Fix state may have changed. Inspect the operation before requesting another preview.",
        });
    } finally {
      this.fixing = false;
    }
  }

  private async refreshFixState(fix: {
    id: string;
    generation: number;
    target: TargetIdentity;
  }): Promise<void> {
    if (!this.client) return;
    try {
      const state = await this.client.fixState(fix.target);
      if (fix.generation === this.state.generation && this.state.operation?.id === fix.id) {
        this.update({
          fixState: state,
          notice:
            "Inspect the operation result. Further commits, pushes and publication require separate approval.",
        });
      }
    } catch (error) {
      if (fix.generation === this.state.generation) this.update({ error: message(error) });
    }
  }

  async openBrowser(client: BackendClient): Promise<void> {
    const target = this.activeTarget();
    if (!target) {
      return;
    }
    const generation = this.state.generation;
    try {
      const result = await client.browserOpen(target);
      if (generation === this.state.generation) {
        this.update({ notice: `Browser diff: ${result.url}` });
      }
    } catch (error) {
      if (generation === this.state.generation) {
        this.update({ error: message(error) });
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
