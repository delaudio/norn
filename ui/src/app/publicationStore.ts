import type {
  PublicationItemResult,
  PublicationJournalEntry,
  PublicationPrepareResult,
  PublicationScope,
  RemoteComment,
  WorkspaceDraft,
} from "../../../src/protocol/generated/protocol";
import type { BackendClient } from "../transport/backendClient";
import { validDraft } from "../transport/workspaceValidation";

interface PublicationState {
  scope: PublicationScope | null;
  drafts: WorkspaceDraft[];
  selected: number;
  checked: string[];
  preview: PublicationPrepareResult | null;
  results: PublicationItemResult[];
  journal: PublicationJournalEntry[];
  comments: RemoteComment[];
  view: "drafts" | "status" | "threads";
  busy: boolean;
  sending: boolean;
  error: string | null;
  notice: string | null;
}
const initial: PublicationState = {
  scope: null,
  drafts: [],
  selected: 0,
  checked: [],
  preview: null,
  results: [],
  journal: [],
  comments: [],
  view: "drafts",
  busy: false,
  sending: false,
  error: null,
  notice: null,
};
export class PublicationStore {
  private state = { ...initial };
  private generation = 0;
  private canceled = false;
  private client: BackendClient | null = null;
  private listeners = new Set<() => void>();
  getSnapshot = () => this.state;
  subscribe = (listener: () => void) => {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  };
  private update(patch: Partial<PublicationState>) {
    this.state = { ...this.state, ...patch };
    for (const listener of this.listeners) listener();
  }
  dispose() {
    void this.cancel();
    this.generation++;
    this.update({ ...initial });
    this.client = null;
  }
  async load(client: BackendClient, scope: PublicationScope) {
    this.dispose();
    const generation = ++this.generation;
    this.client = client;
    this.canceled = false;
    this.update({ scope, busy: true });
    try {
      const key = `drafts.${scope.provider}:${scope.workspace}/${scope.repo}/${scope.prId}`;
      const record = await client.readWorkspace(key, () => generation === this.generation);
      if (generation !== this.generation) return;
      if (
        record.payload.kind !== "drafts" ||
        !record.payload.value.every(validDraft) ||
        record.payload.value.some((draft) => draft.prId !== scope.prId)
      )
        throw new Error("Invalid PR draft record.");
      this.update({ drafts: record.payload.value });
      await this.refreshStatus(generation);
    } catch (error) {
      this.fail(error, generation);
    } finally {
      if (generation === this.generation) this.update({ busy: false });
    }
  }
  private fail(error: unknown, generation: number) {
    if (generation === this.generation)
      this.update({
        error:
          error instanceof Error
            ? error.message
            : "Publication request failed. Inspect status before sending again.",
      });
  }
  move(delta: number) {
    if (this.state.busy || this.state.preview) return;
    const length =
      this.state.view === "threads"
        ? this.state.comments.length
        : this.state.view === "status"
          ? this.state.journal.length
          : this.state.drafts.length;
    if (length) this.update({ selected: (this.state.selected + delta + length) % length });
  }
  toggle() {
    if (this.state.busy || this.state.preview || this.state.view !== "drafts") return;
    const draft = this.state.drafts[this.state.selected];
    if (!draft) return;
    const checked = [...this.state.checked];
    const index = checked.indexOf(draft.localId);
    if (index >= 0) checked.splice(index, 1);
    else if (checked.length < 20) checked.push(draft.localId);
    else {
      this.update({ error: "Select at most twenty drafts per preview." });
      return;
    }
    this.update({ checked, error: null });
  }
  async prepare() {
    const { scope, checked } = this.state;
    const client = this.client;
    if (!scope || !client || this.state.busy || !checked.length) return;
    const generation = this.generation;
    this.update({ busy: true, error: null, results: [], notice: null });
    try {
      const preview = await client.preparePublication(scope, checked);
      if (generation !== this.generation) {
        await client.cancelPublication(scope, preview.approvalId);
        return;
      }
      this.canceled = false;
      this.update({ preview });
    } catch (error) {
      this.fail(error, generation);
    } finally {
      if (generation === this.generation) this.update({ busy: false });
    }
  }
  async approve() {
    const { scope, preview } = this.state;
    const client = this.client;
    if (!scope || !preview || !client || this.state.busy || this.canceled) return;
    if (preview.expiresAt <= Date.now()) {
      await this.cancel();
      this.update({ error: "Preview expired. Prepare a fresh preview." });
      return;
    }
    const generation = this.generation;
    this.update({ busy: true, sending: true, error: null });
    try {
      for (const item of preview.items) {
        if (this.canceled || generation !== this.generation) break;
        const result = await client.applyPublication(
          scope,
          preview.approvalId,
          item.comment.localId,
        );
        if (generation !== this.generation) return;
        this.update({
          results: [...this.state.results, result],
          drafts: result.draftRemoved
            ? this.state.drafts.filter((draft) => draft.localId !== item.comment.localId)
            : this.state.drafts,
        });
        if (result.entry.state !== "published" || !result.localSaved || !result.draftRemoved) {
          this.canceled = true;
          break;
        }
      }
    } catch (error) {
      this.canceled = true;
      this.fail(error, generation);
    } finally {
      // Invalidates unused approval items after partial failure or cancellation.
      await client.cancelPublication(scope, preview.approvalId).catch(() => {});
      if (generation === this.generation) {
        this.update({
          preview: null,
          busy: false,
          sending: false,
          checked: [],
          notice:
            "Inspect per-item results. Outstanding drafts remain local; uncertain sends require read-only reconciliation.",
        });
        await this.refreshStatus(generation).catch((error) => this.fail(error, generation));
      }
    }
  }
  async cancel() {
    this.canceled = true;
    const generation = this.generation;
    const { scope, preview } = this.state;
    if (scope && preview && this.client)
      await this.client.cancelPublication(scope, preview.approvalId).catch(() => {});
    if (generation === this.generation && !this.state.sending) this.update({ preview: null });
  }
  private async refreshStatus(generation = this.generation) {
    const scope = this.state.scope;
    const client = this.client;
    if (!scope || !client) return;
    const journal: PublicationJournalEntry[] = [];
    let after: string | null = null;
    const seen = new Set<string>();
    do {
      const page = await client.publicationStatus(scope, after);
      if (generation !== this.generation) return;
      journal.push(...page.entries);
      after = page.nextAfter;
      if (journal.length > 1000 || (after && seen.has(after)))
        throw new Error(
          "Publication history exceeds its bounded display or contains an invalid cursor.",
        );
      if (after) seen.add(after);
    } while (after);
    this.update({ journal });
  }
  async show(view: PublicationState["view"]) {
    if (this.state.busy || this.state.preview) return;
    const generation = this.generation;
    this.update({ view, selected: 0, busy: true, error: null });
    try {
      if (view === "status") await this.refreshStatus(generation);
      if (view === "threads") {
        const scope = this.state.scope;
        const client = this.client;
        if (!scope || !client) return;
        const comments: RemoteComment[] = [];
        let offset = 0;
        let version: string | null = null;
        for (;;) {
          const page = await client.remoteComments(scope, offset, version);
          if (generation !== this.generation) return;
          comments.push(...page.comments);
          version = page.version;
          if (comments.length > 1000 || JSON.stringify(comments).length > 16 * 1024 * 1024)
            throw new Error("Remote thread display exceeds its limit.");
          if (page.nextOffset === null) break;
          if (page.nextOffset <= offset) throw new Error("Invalid remote thread cursor.");
          offset = page.nextOffset;
        }
        this.update({ comments });
      }
    } catch (error) {
      this.fail(error, generation);
    } finally {
      if (generation === this.generation) this.update({ busy: false });
    }
  }
  async reconcile() {
    const scope = this.state.scope;
    const client = this.client;
    const entry = this.state.journal[this.state.selected];
    if (!scope || !client || !entry || this.state.busy) return;
    const generation = this.generation;
    this.update({ busy: true, error: null });
    try {
      const result = await client.reconcilePublication(scope, entry.comment.localId);
      if (generation !== this.generation) return;
      this.update({ results: [result] });
      await this.refreshStatus(generation);
    } catch (error) {
      this.fail(error, generation);
    } finally {
      if (generation === this.generation) this.update({ busy: false });
    }
  }
}
