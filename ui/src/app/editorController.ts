import type { CliRenderer } from "@opentui/core";
import type {
  RepositoryEditor,
  RepositoryEditorPrepareParams,
} from "../../../src/protocol/generated/protocol";
import type { BackendClient } from "../transport/backendClient";
import { BackendRequestError } from "../transport/types";
import { protectConsoleInterrupts } from "./consoleControl";

export interface EditorSnapshot {
  busy: boolean;
  editor: RepositoryEditor | null;
  message: string | null;
  error: string | null;
}
const idle = (): EditorSnapshot => ({ busy: false, editor: null, message: null, error: null });
const pause = (ms: number) => new Promise<void>((resolve) => setTimeout(resolve, ms));
export class EditorController {
  private snapshot = idle();
  private listeners = new Set<() => void>();
  private active = false;
  private epoch = 0;
  private disposed = false;
  private scope = "";
  constructor(
    private client: BackendClient,
    private renderer: Pick<CliRenderer, "suspend" | "resume" | "isDestroyed">,
  ) {}
  subscribe = (listener: () => void) => {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  };
  getSnapshot = () => this.snapshot;
  private update(value: Partial<EditorSnapshot>) {
    this.snapshot = { ...this.snapshot, ...value };
    for (const listener of this.listeners) listener();
  }
  resetScope(value: string) {
    if (value === this.scope) return;
    this.scope = value;
    this.epoch++;
    this.update({
      ...idle(),
      busy: this.active,
      message: this.active ? "Stopping the previous editor action…" : null,
    });
  }
  unavailable() {
    this.update({ error: "Editor integration requires a matching Norn UI and backend." });
  }
  async start(params: RepositoryEditorPrepareParams): Promise<void> {
    if (this.active || this.disposed) return;
    this.active = true;
    const epoch = ++this.epoch;
    let suspended = false,
      dead = false;
    let editor: RepositoryEditor | null = null;
    let cancelSent = false;
    const current = () => !this.disposed && epoch === this.epoch;
    const emit = (value: Partial<EditorSnapshot>) => {
      if (current()) this.update(value);
    };
    const removeExit = this.client.onExit(() => {
      dead = true;
    });
    emit({ busy: true, editor: null, error: null, message: "Preparing the selected file…" });
    try {
      const prepared = await this.client.repositoryEditorPrepare(params);
      if (!current()) return;
      if (!prepared.token) {
        emit({ message: prepared.message ?? "Editor integration is unavailable." });
        return;
      }
      if (params.mode === "terminal") {
        await protectConsoleInterrupts();
        if (!current()) return;
        this.renderer.suspend();
        suspended = true;
        await protectConsoleInterrupts();
        if (!current()) return;
      }
      const opened = await this.client.repositoryEditorOpen(
        { context: params.context, token: prepared.token },
        params,
      );
      editor = opened.editor;
      const deadline = Date.now() + 25000;
      while (editor.state === "starting" || editor.state === "running") {
        emit({ editor, message: `${editor.integration} · ${editor.state}` });
        if (!current() && !cancelSent) {
          cancelSent = true;
          try {
            await this.client.repositoryEditorCancel(
              { context: editor.context, launchId: editor.launchId },
              editor,
            );
          } catch {
            /* A GUI launch may already be committed. */
          }
        }
        if (params.mode === "graphical" && Date.now() > deadline)
          throw new Error(
            "The editor launcher has not confirmed startup. Refresh its state before retrying.",
          );
        await pause(40);
        const status = await this.client.repositoryEditorStatus(
          { context: editor.context, launchId: editor.launchId },
          editor,
        );
        editor = status.editor;
      }
      emit({
        editor,
        message: [
          editor.message ?? `${editor.integration} · ${editor.state} · r refresh files`,
          prepared.message,
        ]
          .filter(Boolean)
          .join(" · "),
        error: editor.state === "failed" ? (editor.message ?? "Editor launch failed.") : null,
      });
    } catch (error) {
      // A malformed/lost terminal status cannot permit two terminal readers.
      // Close the owned backend so native shutdown stops and restores its editor.
      if (suspended && !dead && (editor !== null || !(error instanceof BackendRequestError))) {
        await this.client.close();
        dead = true;
      }
      emit({
        error: error instanceof Error ? error.message : "Editor integration failed.",
        message: null,
      });
    } finally {
      removeExit();
      if (suspended) {
        // The private native wrapper observes abrupt parent death and restores
        // the original foreground group; normal shutdown already waited for it.
        if (dead) await pause(100);
        if (!this.renderer.isDestroyed) this.renderer.resume();
      }
      this.active = false;
      if (current()) this.update({ busy: false });
      else if (!this.disposed) this.update(idle());
    }
  }
  dispose() {
    this.disposed = true;
    this.epoch++;
    this.listeners.clear();
  }
}
