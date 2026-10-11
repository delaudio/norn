import type { WorkspaceSettings } from "../../../src/protocol/generated/protocol";
import type { BackendClient, CredentialStatus, SettingsSnapshot } from "../transport/backendClient";

export interface SettingsField {
  id: string;
  label: string;
  display: string;
  choices?: readonly string[];
  sensitive?: boolean;
}
export interface SettingsState {
  loading: boolean;
  saving: boolean;
  credentialsLoading: boolean;
  error: string | null;
  notice: string | null;
  draft: WorkspaceSettings | null;
  selected: number;
  editing: boolean;
  editText: string;
  credentials: CredentialStatus[];
  credentialProvider: CredentialStatus["provider"];
  username: string;
  token: string;
  clearConfirmation: boolean;
}
const errorText = (error: unknown) => (error instanceof Error ? error.message : String(error));
const preferenceFields: Array<{
  id: keyof Omit<WorkspaceSettings, "repos">;
  label: string;
  choices?: readonly string[];
}> = [
  { id: "reviewProvider", label: "Review provider", choices: ["github", "bitbucket"] },
  { id: "aiProvider", label: "Agent", choices: ["claude", "codex", "opencode"] },
  { id: "claudeModel", label: "Claude model (blank = default)" },
  { id: "claudeEffort", label: "Claude effort (blank = default)" },
  { id: "codexModel", label: "Codex model (blank = default)" },
  { id: "codexEffort", label: "Codex effort (blank = default)" },
  { id: "opencodeModel", label: "OpenCode model (blank = default)" },
  { id: "opencodeEffort", label: "OpenCode effort (blank = default)" },
  { id: "defaultDiffView", label: "Default diff", choices: ["unified", "split"] },
  { id: "theme", label: "Theme", choices: ["dark", "light"] },
  { id: "jiraBaseUrl", label: "Jira URL" },
  { id: "automaticSyncIntervalSeconds", label: "Automatic PR sync seconds (blank = off)" },
];

export class SettingsStore {
  private listeners = new Set<() => void>();
  private original: SettingsSnapshot | null = null;
  private generation = 0;
  private state: SettingsState = {
    loading: false,
    saving: false,
    credentialsLoading: false,
    error: null,
    notice: null,
    draft: null,
    selected: 0,
    editing: false,
    editText: "",
    credentials: [],
    credentialProvider: "github",
    username: "",
    token: "",
    clearConfirmation: false,
  };
  subscribe = (listener: () => void) => {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  };
  getSnapshot = () => this.state;
  private update(patch: Partial<SettingsState>) {
    this.state = { ...this.state, ...patch };
    for (const listener of this.listeners) listener();
  }
  async load(client: BackendClient) {
    const generation = ++this.generation;
    this.update({ loading: true, error: null, notice: null });
    try {
      const snapshot = await client.settings();
      if (generation !== this.generation) return;
      this.original = snapshot;
      this.update({ draft: structuredClone(snapshot.settings), loading: false });
      await this.loadCredentials(client, generation);
    } catch (error) {
      if (generation === this.generation) this.update({ loading: false, error: errorText(error) });
    }
  }
  private async loadCredentials(client: BackendClient, generation = this.generation) {
    this.update({ credentialsLoading: true });
    try {
      const credentials = await client.credentialStatus();
      if (generation === this.generation) this.update({ credentials });
    } catch (error) {
      if (generation === this.generation) this.update({ error: errorText(error) });
    } finally {
      if (generation === this.generation) this.update({ credentialsLoading: false });
    }
  }
  dispose() {
    this.generation++;
    this.original = null;
    this.update({
      draft: null,
      selected: 0,
      loading: false,
      saving: false,
      credentialsLoading: false,
      credentials: [],
      error: null,
      notice: null,
      username: "",
      token: "",
      editText: "",
      editing: false,
      clearConfirmation: false,
    });
  }
  fields(): SettingsField[] {
    const draft = this.state.draft;
    if (!draft) return [];
    const fields: SettingsField[] = preferenceFields.map((field) => ({
      ...field,
      display: String(draft[field.id] ?? ""),
    }));
    draft.repos.forEach((repo, index) => {
      fields.push({
        id: `repo.${index}.provider`,
        label: `Repo ${index + 1} provider`,
        display: repo.provider,
        choices: ["github", "bitbucket"],
      });
      for (const key of ["workspace", "repo", "localPath"] as const)
        fields.push({
          id: `repo.${index}.${key}`,
          label: `Repo ${index + 1} ${key === "workspace" ? "owner / workspace" : key === "repo" ? "name" : "local clone path"}`,
          display: repo[key] ?? "",
        });
    });
    fields.push(
      {
        id: "credentialProvider",
        label: "Credential provider",
        display: this.state.credentialProvider,
        choices: ["github", "bitbucket", "jira", "notion"],
      },
      { id: "username", label: "Bitbucket username", display: this.state.username },
      {
        id: "token",
        label: "New credential (never read back)",
        display: this.state.token ? "••••••••" : "",
        sensitive: true,
      },
    );
    return fields;
  }
  move(delta: number) {
    const length = this.fields().length;
    if (length && !this.state.editing && !this.state.saving)
      this.update({ selected: (this.state.selected + delta + length) % length });
  }
  select(id: string) {
    const selected = this.fields().findIndex((field) => field.id === id);
    if (selected >= 0) this.update({ selected });
  }
  beginEdit() {
    if (this.state.saving || this.state.loading) return;
    const field = this.fields()[this.state.selected];
    if (!field) return;
    if (field.choices) {
      this.apply(
        field.id,
        field.choices[(field.choices.indexOf(field.display) + 1) % field.choices.length] ??
          field.choices[0] ??
          "",
      );
      return;
    }
    this.update({
      editing: true,
      editText: field.sensitive ? this.state.token : field.display,
      error: null,
    });
  }
  append(text: string) {
    if (!this.state.editing) return;
    const field = this.fields()[this.state.selected];
    const max = field?.sensitive ? 8192 : 4096;
    const next = this.state.editText + text;
    if (Array.from(next).length > max) {
      this.update({ error: `Input exceeds ${max} characters; the pasted value was not applied.` });
      return;
    }
    if (/\p{Cc}/u.test(text)) {
      this.update({
        error: "Input contains control characters; the pasted value was not applied.",
      });
      return;
    }
    this.update({ editText: next });
  }
  backspace() {
    if (this.state.editing) {
      const text = Array.from(this.state.editText);
      text.pop();
      this.update({ editText: text.join("") });
    }
  }
  clearEdit() {
    this.update({ editText: "" });
  }
  finishEdit() {
    const field = this.fields()[this.state.selected];
    if (field && this.apply(field.id, this.state.editText))
      this.update({ editing: false, editText: "" });
  }
  cancelEdit() {
    this.update({ editing: false, editText: "", error: null });
  }
  editDisplay() {
    return this.fields()[this.state.selected]?.sensitive
      ? "•".repeat(Math.min(32, Array.from(this.state.editText).length))
      : this.state.editText;
  }
  private apply(id: string, text: string): boolean {
    if (id === "token" || id === "username") {
      this.update({ [id]: text });
      return true;
    }
    if (id === "credentialProvider") {
      this.update({
        credentialProvider: text as CredentialStatus["provider"],
        username: "",
        token: "",
        clearConfirmation: false,
      });
      return true;
    }
    if (!this.state.draft) return false;
    const draft = structuredClone(this.state.draft);
    if (id.startsWith("repo.")) {
      const [, index, property] = id.split(".");
      const repo = draft.repos[Number(index)];
      if (!repo) return false;
      if (property === "provider") repo.provider = text as "github" | "bitbucket";
      else if (property === "workspace") repo.workspace = text;
      else if (property === "repo") repo.repo = text;
      else repo.localPath = text || null;
    } else if (id === "automaticSyncIntervalSeconds") {
      if (text && (!/^\d+$/.test(text) || Number(text) < 10 || Number(text) > 86400)) {
        this.update({ error: "Use 10–86400 seconds, or blank to disable sync." });
        return false;
      }
      draft.automaticSyncIntervalSeconds = text ? Number(text) : null;
    } else (draft as unknown as Record<string, unknown>)[id] = text || null;
    this.update({ draft, error: null });
    return true;
  }
  addRepository() {
    if (!this.state.draft || this.state.saving || this.state.draft.repos.length >= 100) return;
    const draft = structuredClone(this.state.draft);
    draft.repos.push({ provider: draft.reviewProvider, workspace: "", repo: "", localPath: null });
    this.update({ draft });
    this.select(`repo.${draft.repos.length - 1}.workspace`);
  }
  removeRepository() {
    const id = this.fields()[this.state.selected]?.id;
    if (!id?.startsWith("repo.") || !this.state.draft || this.state.saving) return;
    const index = Number(id.split(".")[1]);
    const draft = structuredClone(this.state.draft);
    draft.repos.splice(index, 1);
    this.update({
      draft,
      selected: Math.min(this.state.selected, this.fields().length - 5),
      notice: "Repository removed from this draft. Local clone and review data are preserved.",
    });
  }
  private readyToSave(): boolean {
    if (
      this.state.saving ||
      this.state.loading ||
      this.state.credentialsLoading ||
      this.state.editing
    ) {
      this.update({
        notice: "Finish editing and wait for pending settings/credential requests before saving.",
      });
      return false;
    }
    return true;
  }
  async save(client: BackendClient): Promise<WorkspaceSettings | null> {
    if (!this.readyToSave() || !this.original || !this.state.draft) return null;
    const generation = this.generation;
    this.update({ saving: true, error: null });
    try {
      const snapshot = await client.saveSettings(
        this.original.version,
        structuredClone(this.state.draft),
      );
      if (generation !== this.generation) return null;
      this.original = snapshot;
      this.update({
        draft: structuredClone(snapshot.settings),
        notice: "Preferences saved. Credentials are saved separately.",
      });
      return snapshot.settings;
    } catch (error) {
      if (generation === this.generation)
        this.update({
          error: errorText(error),
          notice:
            "Preferences were not confirmed saved. Keep this draft and reload or inspect the file before retrying.",
        });
      return null;
    } finally {
      if (generation === this.generation) this.update({ saving: false });
    }
  }
  async saveCredential(client: BackendClient) {
    if (!this.readyToSave()) return;
    const token = this.state.token;
    if (!token.trim()) {
      this.update({ error: "Enter a new credential before saving it." });
      return;
    }
    const generation = this.generation;
    this.update({ saving: true, error: null });
    try {
      await client.setCredential(this.state.credentialProvider, this.state.username || null, token);
      if (generation !== this.generation) return;
      this.update({
        token: "",
        editText: "",
        notice: "Credential saved in the OS store. Preferences were not changed.",
      });
      await this.loadCredentials(client, generation);
    } catch (error) {
      if (generation === this.generation)
        this.update({
          error: errorText(error).split(token).join("[redacted]"),
          notice:
            "Credential save was not confirmed. Inspect OS credential status before retrying.",
        });
    } finally {
      if (generation === this.generation) this.update({ saving: false });
    }
  }
  requestClear() {
    if (this.readyToSave()) this.update({ clearConfirmation: true });
  }
  dismissClear() {
    this.update({ clearConfirmation: false });
  }
  async clearCredential(client: BackendClient) {
    if (!this.state.clearConfirmation || !this.readyToSave()) return;
    const generation = this.generation;
    this.update({ clearConfirmation: false, saving: true, error: null });
    try {
      await client.clearCredential(this.state.credentialProvider);
      if (generation !== this.generation) return;
      this.update({
        token: "",
        username: "",
        notice:
          "Keychain credential removed. Environment credentials, local config and review data were preserved.",
      });
      await this.loadCredentials(client, generation);
    } catch (error) {
      if (generation === this.generation) this.update({ error: errorText(error) });
    } finally {
      if (generation === this.generation) this.update({ saving: false });
    }
  }
}
