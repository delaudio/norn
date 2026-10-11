import type { BackendClient, DataImportPreview } from "../transport/backendClient";

interface DataState {
  path: string;
  exportPath: string;
  provider: "github" | "bitbucket" | null;
  selected: number;
  editing: boolean;
  editText: string;
  busy: boolean;
  preview: DataImportPreview | null;
  error: string | null;
  notice: string | null;
}
const initial: DataState = {
  path: "",
  exportPath: "",
  provider: null,
  selected: 0,
  editing: false,
  editText: "",
  busy: false,
  preview: null,
  error: null,
  notice: null,
};
export class DataStore {
  private state: DataState = { ...initial };
  private generation = 0;
  private listeners = new Set<() => void>();
  getSnapshot = () => this.state;
  subscribe = (listener: () => void) => {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  };
  private update(patch: Partial<DataState>) {
    this.state = { ...this.state, ...patch };
    for (const listener of this.listeners) listener();
  }
  dispose() {
    this.generation++;
    this.update({ ...initial });
  }
  move(delta: number) {
    if (!this.state.busy && !this.state.editing)
      this.update({ selected: (this.state.selected + delta + 3) % 3 });
  }
  beginEdit() {
    if (this.state.busy) return;
    if (this.state.selected === 1) {
      const values = [null, "github", "bitbucket"] as const;
      this.update({
        provider: values[(values.indexOf(this.state.provider) + 1) % 3] ?? null,
        preview: null,
      });
      return;
    }
    this.update({
      editing: true,
      editText: this.state.selected === 0 ? this.state.path : this.state.exportPath,
      error: null,
    });
  }
  append(text: string) {
    if (!this.state.editing) return;
    if (/\p{Cc}/u.test(text) || [...(this.state.editText + text)].length > 4096) {
      this.update({
        error:
          "Path input exceeds its limit or contains control characters. Paste was not applied.",
      });
      return;
    }
    this.update({ editText: this.state.editText + text });
  }
  backspace() {
    this.update({ editText: [...this.state.editText].slice(0, -1).join("") });
  }
  clearEdit() {
    this.update({ editText: "" });
  }
  finishEdit() {
    if (!this.state.editing) return;
    this.update({
      ...(this.state.selected === 0
        ? { path: this.state.editText, preview: null }
        : { exportPath: this.state.editText }),
      editing: false,
      editText: "",
    });
  }
  cancelEdit() {
    this.update({ editing: false, editText: "" });
  }
  async preview(client: BackendClient) {
    if (this.state.busy || this.state.editing) return;
    if (!this.state.path) {
      this.update({ error: "Enter an export file path first." });
      return;
    }
    const generation = this.generation;
    this.update({ busy: true, error: null, notice: null, preview: null });
    try {
      const preview = await client.previewData(this.state.path, this.state.provider);
      if (generation === this.generation)
        this.update({
          preview,
          notice: "Preview only. Ctrl+Y confirms additions; existing conflicts are kept.",
        });
    } catch (error) {
      if (generation === this.generation)
        this.update({
          error: error instanceof Error ? error.message : "Cannot preview user data.",
        });
    } finally {
      if (generation === this.generation) this.update({ busy: false });
    }
  }
  async confirm(client: BackendClient) {
    if (this.state.busy || this.state.editing || !this.state.preview) return;
    const generation = this.generation;
    const preview = this.state.preview;
    this.update({ busy: true, error: null });
    try {
      const result = await client.importData(this.state.path, this.state.provider, preview);
      if (generation === this.generation)
        this.update({
          preview: null,
          notice: `Imported ${result.imported} scopes; ${result.unchanged} unchanged, ${result.conflicts} conflicts kept, ${result.rejectedCount} rejected. ${result.backupPath ? `Backup: ${result.backupPath}` : "No destination changes."}`,
        });
    } catch (error) {
      if (generation === this.generation)
        this.update({
          preview: null,
          error:
            error instanceof Error
              ? error.message
              : "Import was not confirmed. Preview again before retrying.",
        });
    } finally {
      if (generation === this.generation) this.update({ busy: false });
    }
  }
  async export(client: BackendClient) {
    if (this.state.busy || this.state.editing) return;
    if (!this.state.exportPath) {
      this.update({ error: "Enter a new export file path first." });
      return;
    }
    const generation = this.generation;
    this.update({ busy: true, error: null, notice: null });
    try {
      const count = await client.exportData(this.state.exportPath);
      if (generation === this.generation)
        this.update({
          notice: `Exported ${count} canonical scopes. Existing files are never replaced.`,
        });
    } catch (error) {
      if (generation === this.generation)
        this.update({ error: error instanceof Error ? error.message : "Cannot export user data." });
    } finally {
      if (generation === this.generation) this.update({ busy: false });
    }
  }
}
