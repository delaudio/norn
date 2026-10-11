import type { WorkspaceDraft } from "../../../src/protocol/generated/protocol";
import type { BackendClient } from "../transport/backendClient";
import { draftModes, validDraft } from "../transport/workspaceValidation";
import type { PickerEntry } from "./store";

export { draftModes, validDraft } from "../transport/workspaceValidation";
export const draftFields = ["publicationMode", "path", "to", "from", "parentId", "raw"] as const;
export type DraftField = (typeof draftFields)[number];
const textLimit = (field: DraftField) => (field === "raw" ? 65536 : field === "path" ? 4096 : 256);
interface DraftState {
  key: string | null;
  version: string | null;
  items: WorkspaceDraft[];
  selected: number;
  field: number;
  draft: WorkspaceDraft | null;
  editing: boolean;
  text: string;
  removing: boolean;
  busy: boolean;
  error: string | null;
  notice: string | null;
}
const initial: DraftState = {
  key: null,
  version: null,
  items: [],
  selected: 0,
  field: 0,
  draft: null,
  editing: false,
  text: "",
  removing: false,
  busy: false,
  error: null,
  notice: null,
};
export class DraftStore {
  private state: DraftState = { ...initial };
  private generation = 0;
  private listeners = new Set<() => void>();
  constructor(
    private readonly now = Date.now,
    private readonly id: () => string = () => crypto.randomUUID(),
  ) {}
  getSnapshot = () => this.state;
  subscribe = (listener: () => void) => {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  };
  private update(patch: Partial<DraftState>) {
    this.state = { ...this.state, ...patch };
    for (const listener of this.listeners) listener();
  }
  dispose() {
    this.generation++;
    this.update({ ...initial });
  }
  async load(
    client: BackendClient,
    entry: Pick<PickerEntry, "provider" | "workspace" | "repo" | "prId"> | undefined,
  ) {
    const generation = ++this.generation;
    this.update({ ...initial, busy: true });
    if (!entry || entry.prId === null) {
      this.update({
        busy: false,
        error: "Select a pull request. Drafts are scoped to a provider and PR.",
      });
      return;
    }
    const key = `drafts.${entry.provider}:${entry.workspace}/${entry.repo}/${entry.prId}`;
    this.update({ key });
    try {
      const record = await client.readWorkspace(key, () => generation === this.generation);
      if (generation !== this.generation) return;
      if (
        record.payload.kind !== "drafts" ||
        record.payload.value.length > 1000 ||
        !record.payload.value.every((item) => validDraft(item) && item.prId === entry.prId) ||
        new Set(record.payload.value.map((item) => item.localId)).size !==
          record.payload.value.length
      )
        throw new Error(
          "Invalid saved draft comment record. Preserve the source before repairing it.",
        );
      this.update({ items: structuredClone(record.payload.value), version: record.recordVersion });
    } catch (error) {
      if (generation === this.generation)
        this.update({
          error: error instanceof Error ? error.message : "Cannot load saved drafts.",
        });
    } finally {
      if (generation === this.generation) this.update({ busy: false });
    }
  }
  move(delta: number) {
    if (this.state.busy || this.state.editing || this.state.removing) return;
    if (this.state.draft)
      this.update({
        field: (this.state.field + delta + draftFields.length) % draftFields.length,
      });
    else if (this.state.items.length)
      this.update({
        selected: (this.state.selected + delta + this.state.items.length) % this.state.items.length,
      });
  }
  add() {
    if (this.state.busy || !this.state.version) return;
    if (this.state.items.length >= 1000) {
      this.update({
        error:
          "This PR already has 1,000 drafts. Remove an unused draft comment before adding another.",
      });
      return;
    }
    const time = this.now();
    this.update({
      draft: {
        localId: this.id(),
        prId: Number(this.state.key!.split("/").at(-1)),
        path: "",
        raw: "",
        publicationMode: "general",
        source: "manual",
        createdAt: time,
      },
      field: 0,
      error: null,
      notice: null,
    });
  }
  edit() {
    if (this.state.busy || !this.state.items[this.state.selected]) return;
    this.update({
      draft: structuredClone(this.state.items[this.state.selected]!),
      field: 0,
      error: null,
      notice: null,
    });
  }
  beginField() {
    if (this.state.busy || !this.state.draft) return;
    const field = draftFields[this.state.field]!;
    if (field === "publicationMode") {
      const index =
        this.state.draft.publicationMode == null
          ? -1
          : draftModes.indexOf(this.state.draft.publicationMode);
      this.update({
        draft: {
          ...this.state.draft,
          publicationMode: draftModes[(index + 1) % draftModes.length]!,
        },
      });
      return;
    }
    this.update({ editing: true, text: String(this.state.draft[field] ?? "") });
  }
  append(value: string) {
    if (!this.state.editing || this.state.busy) return;
    const field = draftFields[this.state.field]!;
    const text = this.state.text + value;
    if (
      Buffer.byteLength(text) > textLimit(field) ||
      (field === "raw" ? /[^\P{Cc}\n\r\t]/u : /\p{Cc}/u).test(text)
    ) {
      this.update({
        error: "Input exceeds its limit or contains unsupported controls. Paste was not applied.",
      });
      return;
    }
    this.update({ text, error: null });
  }
  backspace() {
    if (this.state.editing && !this.state.busy)
      this.update({ text: [...this.state.text].slice(0, -1).join("") });
  }
  clearField() {
    if (this.state.editing && !this.state.busy) this.update({ text: "" });
  }
  finishField() {
    if (!this.state.draft || !this.state.editing || this.state.busy) return;
    const field = draftFields[this.state.field]!;
    let value: string | number | null = ["path", "raw"].includes(field)
      ? this.state.text
      : this.state.text || null;
    if ((field === "to" || field === "from") && value !== null) {
      if (!/^[1-9][0-9]*$/.test(String(value)) || Number(value) > 4294967295) {
        this.update({ error: "Line must be a positive 32-bit integer, or empty." });
        return;
      }
      value = Number(value);
    }
    this.update({
      draft: { ...this.state.draft, [field]: value },
      editing: false,
      text: "",
      error: null,
    });
  }
  cancel() {
    if (this.state.busy) return;
    if (this.state.editing) this.update({ editing: false, text: "", error: null });
    else
      this.update({
        draft: null,
        removing: false,
        error: null,
        notice: "Unsaved draft comment edits discarded.",
      });
  }
  requestRemoval() {
    if (!this.state.busy && !this.state.draft && this.state.items[this.state.selected])
      this.update({ removing: true, error: null });
  }
  async save(client: BackendClient) {
    if (
      this.state.busy ||
      this.state.editing ||
      !this.state.draft ||
      !this.state.key ||
      !this.state.version
    )
      return;
    const draft = structuredClone(this.state.draft);
    if (!validDraft(draft)) {
      this.update({ error: "Draft fields are invalid. Correct them before saving." });
      return;
    }
    const generation = this.generation;
    this.update({ busy: true, error: null });
    try {
      const result = await client.mutateWorkspace(this.state.key, this.state.version, {
        kind: "putDraft",
        draft,
      });
      if (generation !== this.generation) return;
      const items = this.state.items.some((item) => item.localId === draft.localId)
        ? this.state.items.map((item) => (item.localId === draft.localId ? draft : item))
        : [...this.state.items, draft];
      this.update({
        items,
        version: result.recordVersion,
        draft: null,
        selected: items.findIndex((item) => item.localId === draft.localId),
        notice: "Draft saved locally. Publication requires a separate preview and approval.",
      });
    } catch (error) {
      if (generation === this.generation)
        this.update({
          error: error instanceof Error ? error.message : "Draft was not saved. Edits retained.",
        });
    } finally {
      if (generation === this.generation) this.update({ busy: false });
    }
  }
  async remove(client: BackendClient) {
    if (this.state.busy || !this.state.removing || !this.state.key || !this.state.version) return;
    const item = this.state.items[this.state.selected];
    if (!item) return;
    const generation = this.generation;
    this.update({ busy: true, error: null });
    try {
      const result = await client.mutateWorkspace(this.state.key, this.state.version, {
        kind: "removeDraft",
        localId: item.localId,
      });
      if (generation !== this.generation) return;
      const items = this.state.items.filter((entry) => entry.localId !== item.localId);
      this.update({
        items,
        version: result.recordVersion,
        selected: Math.min(this.state.selected, Math.max(0, items.length - 1)),
        removing: false,
        notice: "Draft removed from this PR. No publication or external access.",
      });
    } catch (error) {
      if (generation === this.generation)
        this.update({
          removing: false,
          error: error instanceof Error ? error.message : "Draft removal was not confirmed.",
        });
    } finally {
      if (generation === this.generation) this.update({ busy: false });
    }
  }
}
