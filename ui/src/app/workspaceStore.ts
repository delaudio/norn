import type { BackendClient } from "../transport/backendClient";
import type { PickerEntry } from "./store";

interface WorkspaceState {
  scope: string | null;
  promptKey: string | null;
  viewedKey: string | null;
  version: string | null;
  viewedVersion: string | null;
  saved: string;
  text: string;
  viewed: string[];
  busy: boolean;
  error: string | null;
  notice: string | null;
}
const initial: WorkspaceState = {
  scope: null,
  promptKey: null,
  viewedKey: null,
  version: null,
  viewedVersion: null,
  saved: "",
  text: "",
  viewed: [],
  busy: false,
  error: null,
  notice: null,
};
export class WorkspaceStore {
  private state: WorkspaceState = { ...initial };
  private generation = 0;
  private listeners = new Set<() => void>();
  getSnapshot = () => this.state;
  subscribe = (listener: () => void) => {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  };
  private update(patch: Partial<WorkspaceState>) {
    this.state = { ...this.state, ...patch };
    for (const listener of this.listeners) listener();
  }
  dispose() {
    this.generation++;
    this.update({ ...initial });
  }
  async load(client: BackendClient, entry: PickerEntry | undefined) {
    const generation = ++this.generation;
    this.update({ ...initial, busy: true });
    if (!entry) {
      this.update({ busy: false, error: "Select a repository first." });
      return;
    }
    const scope = `${entry.provider}:${entry.workspace}/${entry.repo}`;
    const promptKey = `prompt.${scope}`;
    const viewedKey = entry.prId === null ? null : `viewed.${scope}/${entry.prId}`;
    this.update({ scope, promptKey, viewedKey });
    const current = () => generation === this.generation;
    try {
      const [prompt, viewed] = await Promise.all([
        client.readWorkspace(promptKey, current),
        viewedKey ? client.readWorkspace(viewedKey, current) : Promise.resolve(null),
      ]);
      if (!current()) return;
      if (prompt.payload.kind !== "prompt" || (viewed && viewed.payload.kind !== "viewed"))
        throw new Error("Workspace response has the wrong record kind.");
      this.update({
        version: prompt.recordVersion,
        saved: prompt.payload.value,
        text: prompt.payload.value,
        viewedVersion: viewed?.recordVersion ?? null,
        viewed: viewed?.payload.kind === "viewed" ? viewed.payload.value : [],
        busy: false,
      });
    } catch (error) {
      if (current())
        this.update({ busy: false, error: String(error instanceof Error ? error.message : error) });
    }
  }
  append(value: string) {
    if (this.state.busy || !this.state.version) return;
    if (/[^\P{Cc}\n\t]/u.test(value) || Buffer.byteLength(this.state.text + value) > 65536) {
      this.update({
        error:
          "Prompt exceeds 64 KiB or contains unsupported control characters; paste was not applied.",
      });
      return;
    }
    this.update({ text: this.state.text + value, notice: null });
  }
  backspace() {
    if (!this.state.busy) this.update({ text: [...this.state.text].slice(0, -1).join("") });
  }
  clear() {
    if (!this.state.busy) this.update({ text: "", notice: null });
  }
  discard() {
    if (!this.state.busy)
      this.update({ text: this.state.saved, error: null, notice: "Unsaved edits discarded." });
  }
  async save(client: BackendClient) {
    const { promptKey, version, text } = this.state;
    if (this.state.busy || !promptKey || !version) return;
    const generation = this.generation;
    this.update({ busy: true, error: null });
    try {
      const result = await client.mutateWorkspace(promptKey, version, {
        kind: "setPrompt",
        value: text,
      });
      if (generation === this.generation)
        this.update({
          version: result.recordVersion,
          saved: text,
          notice: text.trim()
            ? "Saved for new reviews and fresh-session replies. Existing resumed sessions retain their conversation."
            : "Saved empty override; repository policy applies.",
        });
    } catch (error) {
      if (generation === this.generation)
        this.update({
          error: error instanceof Error ? error.message : "Cannot save prompt; edits retained.",
        });
    } finally {
      if (generation === this.generation) this.update({ busy: false });
    }
  }
  async toggleViewed(client: BackendClient, path: string | undefined) {
    const { viewedKey, viewedVersion, viewed } = this.state;
    if (this.state.busy || !viewedKey || !viewedVersion || !path) return;
    const generation = this.generation;
    const removing = viewed.includes(path);
    this.update({ busy: true, error: null });
    try {
      const result = await client.mutateWorkspace(viewedKey, viewedVersion, {
        kind: removing ? "removeViewed" : "addViewed",
        path,
      });
      if (generation === this.generation)
        this.update({
          viewedVersion: result.recordVersion,
          viewed: removing ? viewed.filter((item) => item !== path) : [...viewed, path],
          notice: "Viewed mark saved.",
        });
    } catch (error) {
      if (generation === this.generation)
        this.update({ error: error instanceof Error ? error.message : "Cannot save viewed mark." });
    } finally {
      if (generation === this.generation) this.update({ busy: false });
    }
  }
}
