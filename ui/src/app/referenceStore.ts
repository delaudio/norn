import type { WorkspaceReference } from "../../../src/protocol/generated/protocol";
import type { BackendClient } from "../transport/backendClient";
import type { PickerEntry } from "./store";

export const referenceTypes = ["note", "jira", "notion", "repository", "pullRequest"] as const;
export const referenceFields = [
  "type",
  "title",
  "url",
  "key",
  "workspace",
  "repo",
  "prId",
  "localPath",
  "body",
] as const;
export type ReferenceField = (typeof referenceFields)[number];
const textLimit = (field: ReferenceField) =>
  field === "body" ? 65536 : field === "url" || field === "localPath" ? 4096 : 1024;
export function validReference(value: unknown): value is WorkspaceReference {
  if (!value || typeof value !== "object" || Array.isArray(value)) return false;
  const item = value as WorkspaceReference;
  const keys = new Set([
    "id",
    "type",
    "source",
    "title",
    "url",
    "key",
    "workspace",
    "repo",
    "prId",
    "localPath",
    "body",
    "createdAt",
    "updatedAt",
  ]);
  if (
    Object.keys(value).some((key) => !keys.has(key)) ||
    typeof item.id !== "string" ||
    !item.id ||
    Buffer.byteLength(item.id) > 256 ||
    /\p{Cc}/u.test(item.id) ||
    !referenceTypes.includes(item.type) ||
    !["manual", "detected"].includes(item.source) ||
    ![item.createdAt, item.updatedAt].every((time) => Number.isSafeInteger(time) && time >= 0)
  )
    return false;
  if (
    item.prId != null &&
    (!Number.isInteger(item.prId) || item.prId < 1 || item.prId > 4294967295)
  )
    return false;
  return referenceFields
    .filter((field) => field !== "type" && field !== "prId")
    .every((field) => {
      const text = item[field];
      return (
        text == null ||
        (typeof text === "string" &&
          Buffer.byteLength(text) <= textLimit(field) &&
          !(field === "body" ? /[^\P{Cc}\n\t]/u : /\p{Cc}/u).test(text))
      );
    });
}
interface ReferenceState {
  key: string | null;
  version: string | null;
  items: WorkspaceReference[];
  selected: number;
  field: number;
  draft: WorkspaceReference | null;
  editing: boolean;
  text: string;
  removing: boolean;
  busy: boolean;
  error: string | null;
  notice: string | null;
}
const initial: ReferenceState = {
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
export class ReferenceStore {
  private state: ReferenceState = { ...initial };
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
  private update(patch: Partial<ReferenceState>) {
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
        error: "Select a pull request. References are scoped to a provider and PR.",
      });
      return;
    }
    const key = `references.${entry.provider}:${entry.workspace}/${entry.repo}/${entry.prId}`;
    this.update({ key });
    try {
      const record = await client.readWorkspace(key, () => generation === this.generation);
      if (generation !== this.generation) return;
      if (
        record.payload.kind !== "references" ||
        record.payload.value.length > 1000 ||
        !record.payload.value.every(validReference) ||
        new Set(record.payload.value.map((item) => item.id)).size !== record.payload.value.length
      )
        throw new Error("Invalid saved reference record. Preserve the source before repairing it.");
      this.update({ items: structuredClone(record.payload.value), version: record.recordVersion });
    } catch (error) {
      if (generation === this.generation)
        this.update({
          error: error instanceof Error ? error.message : "Cannot load saved references.",
        });
    } finally {
      if (generation === this.generation) this.update({ busy: false });
    }
  }
  move(delta: number) {
    if (this.state.busy || this.state.editing || this.state.removing) return;
    if (this.state.draft)
      this.update({
        field: (this.state.field + delta + referenceFields.length) % referenceFields.length,
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
          "This PR already has 1,000 references. Remove an unused reference before adding another.",
      });
      return;
    }
    const time = this.now();
    this.update({
      draft: { id: this.id(), type: "note", source: "manual", createdAt: time, updatedAt: time },
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
    const field = referenceFields[this.state.field]!;
    if (field === "type") {
      const index = referenceTypes.indexOf(this.state.draft.type);
      this.update({
        draft: { ...this.state.draft, type: referenceTypes[(index + 1) % referenceTypes.length]! },
      });
      return;
    }
    this.update({ editing: true, text: String(this.state.draft[field] ?? "") });
  }
  append(value: string) {
    if (!this.state.editing || this.state.busy) return;
    const field = referenceFields[this.state.field]!;
    const text = this.state.text + value;
    if (
      Buffer.byteLength(text) > textLimit(field) ||
      (field === "body" ? /[^\P{Cc}\n\t]/u : /\p{Cc}/u).test(text)
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
    const field = referenceFields[this.state.field]!;
    let value: string | number | null = this.state.text || null;
    if (field === "prId" && value !== null) {
      if (!/^[1-9][0-9]*$/.test(String(value)) || Number(value) > 4294967295) {
        this.update({ error: "Related PR number must be a positive 32-bit integer." });
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
        notice: "Unsaved reference edits discarded.",
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
    const draft = { ...this.state.draft, updatedAt: this.now() };
    if (!validReference(draft)) {
      this.update({ error: "Reference fields are invalid. Correct them before saving." });
      return;
    }
    const generation = this.generation;
    this.update({ busy: true, error: null });
    try {
      const result = await client.mutateWorkspace(this.state.key, this.state.version, {
        kind: "putReference",
        reference: draft,
      });
      if (generation !== this.generation) return;
      const items = this.state.items.some((item) => item.id === draft.id)
        ? this.state.items.map((item) => (item.id === draft.id ? draft : item))
        : [...this.state.items, draft];
      this.update({
        items,
        version: result.recordVersion,
        draft: null,
        selected: items.findIndex((item) => item.id === draft.id),
        notice:
          "Reference saved for new reviews and fresh-session replies. Resumed conversations retain their earlier context.",
      });
    } catch (error) {
      if (generation === this.generation)
        this.update({
          error:
            error instanceof Error ? error.message : "Reference was not saved. Edits retained.",
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
        kind: "removeReference",
        id: item.id,
      });
      if (generation !== this.generation) return;
      const items = this.state.items.filter((entry) => entry.id !== item.id);
      this.update({
        items,
        version: result.recordVersion,
        selected: Math.min(this.state.selected, Math.max(0, items.length - 1)),
        removing: false,
        notice: "Reference removed from this PR. No publication or external access.",
      });
    } catch (error) {
      if (generation === this.generation)
        this.update({
          removing: false,
          error: error instanceof Error ? error.message : "Reference removal was not confirmed.",
        });
    } finally {
      if (generation === this.generation) this.update({ busy: false });
    }
  }
}
