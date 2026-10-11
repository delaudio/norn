import type {
  DraftPreparation,
  DraftPrepareParams,
  DraftTarget,
} from "../../../src/protocol/generated/protocol";
import type { BackendClient } from "../transport/backendClient";

export const stagingModes = ["inline", "file", "general", "localOnly"] as const;
interface State {
  target: DraftTarget | null;
  findingId: string;
  mode: DraftPrepareParams["publicationMode"];
  prepared: DraftPreparation | null;
  busy: boolean;
  done: boolean;
  error: string | null;
}
const initial: State = {
  target: null,
  findingId: "",
  mode: "inline",
  prepared: null,
  busy: false,
  done: false,
  error: null,
};
export class DraftStagingStore {
  private state: State = { ...initial };
  private generation = 0;
  private listeners = new Set<() => void>();
  getSnapshot = () => this.state;
  subscribe = (listener: () => void) => {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  };
  private update(patch: Partial<State>) {
    this.state = { ...this.state, ...patch };
    for (const listener of this.listeners) listener();
  }
  open(target: DraftTarget, findingId: string) {
    this.generation++;
    this.update({ ...initial, target: structuredClone(target), findingId });
  }
  dispose() {
    this.generation++;
    this.update({ ...initial });
  }
  moveMode(delta: number) {
    if (this.state.busy || this.state.done) return;
    const index = stagingModes.indexOf(this.state.mode);
    this.update({
      mode: stagingModes[(index + delta + stagingModes.length) % stagingModes.length]!,
      prepared: null,
      error: null,
    });
  }
  dismissPreview() {
    if (!this.state.busy) this.update({ prepared: null, error: null });
  }
  async preview(client: BackendClient) {
    if (!this.state.target || this.state.busy || this.state.done) return;
    const generation = this.generation;
    this.update({ busy: true, prepared: null, error: null });
    try {
      const prepared = await client.prepareFindingDraft({
        target: this.state.target,
        findingId: this.state.findingId,
        publicationMode: this.state.mode,
      });
      if (generation === this.generation) this.update({ prepared });
    } catch (error) {
      if (generation === this.generation)
        this.update({ error: error instanceof Error ? error.message : "Cannot prepare a draft." });
    } finally {
      if (generation === this.generation) this.update({ busy: false });
    }
  }
  async stage(client: BackendClient) {
    const { target, findingId, mode, prepared } = this.state;
    if (!target || !prepared || this.state.busy || this.state.done) return;
    const generation = this.generation;
    this.update({ busy: true, error: null });
    try {
      const result = await client.stageFindingDraft({
        target,
        findingId,
        publicationMode: mode,
        expectedVersion: prepared.recordVersion,
        previewHash: prepared.previewHash,
      });
      if (generation !== this.generation) return;
      if (result.localId !== prepared.draft.localId)
        throw new Error(
          "Staging acknowledged another draft. Inspect saved drafts before retrying.",
        );
      this.update({ done: true });
    } catch (error) {
      if (generation === this.generation)
        this.update({
          error:
            error instanceof Error
              ? error.message
              : "Staging outcome is unknown. Inspect saved drafts before retrying.",
        });
    } finally {
      if (generation === this.generation) this.update({ busy: false });
    }
  }
}
