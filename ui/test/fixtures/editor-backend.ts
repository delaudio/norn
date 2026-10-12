import type { RepositoryEditorPrepareParams } from "../../../src/protocol/generated/protocol";
import { ExplorerFixture } from "./explorer-backend";
export class EditorExplorerFixture extends ExplorerFixture {
  editorCalls: Array<{ method: string; params: Record<string, unknown> }> = [];
  state: "launched" | "failed" = "launched";
  prepared: RepositoryEditorPrepareParams | null = null;
  override async start() {
    const ready = await super.start();
    return { ...ready, capabilities: [...ready.capabilities, "repository-editor"] };
  }
  override async request(method: string, request: Record<string, unknown>) {
    if (!method.startsWith("repository.editor.")) return super.request(method, request);
    this.editorCalls.push({ method, params: structuredClone(request) });
    if (method === "repository.editor.prepare") {
      this.prepared = request as unknown as RepositoryEditorPrepareParams;
      return {
        request,
        token: "c".repeat(64),
        integration: "zed",
        expiresInMs: 30000,
        message: null,
      };
    }
    if (method === "repository.editor.open" && this.prepared)
      return {
        request,
        editor: {
          ...this.prepared,
          launchId: "d".repeat(64),
          integration: "zed",
          state: this.state,
          exitCode: this.state === "failed" ? 7 : null,
          message: this.state === "failed" ? "Owned launcher failed." : null,
        },
      };
    throw new Error("Unexpected editor fixture method");
  }
}
