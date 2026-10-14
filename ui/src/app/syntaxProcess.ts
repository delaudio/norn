import { fileURLToPath } from "node:url";

export interface SyntaxTask {
  onmessage: ((event: { data: unknown }) => void) | null;
  onerror: ((event: { preventDefault(): void }) => void) | null;
  postMessage(message: { path: string; diff: string }): void;
  terminate(): Promise<void> | void;
}

// Bun 1.3.8 WebWorker start/terminate churn can crash its host process. Keep
// optional coloring in an owned subprocess instead: even a tokenizer/runtime
// crash is isolated from the terminal and always reaped before a replacement.
export class SyntaxProcess implements SyntaxTask {
  onmessage: SyntaxTask["onmessage"] = null;
  onerror: SyntaxTask["onerror"] = null;
  private readonly proc: Bun.Subprocess<"ignore", "ignore", "ignore">;
  private stopping = false;
  constructor() {
    this.proc = Bun.spawn(
      process.env.NORN_COMPILED_UI === "1"
        ? [process.execPath, "--syntax-worker"]
        : [process.execPath, fileURLToPath(new URL("../workers/syntax.ts", import.meta.url))],
      {
        stdin: "ignore",
        stdout: "ignore",
        stderr: "ignore",
        env: { LANG: "C.UTF-8" },
        ipc: (data) => this.onmessage?.({ data }),
      },
    );
    void this.proc.exited.then(() => {
      if (!this.stopping) this.onerror?.({ preventDefault() {} });
    });
  }
  postMessage(message: { path: string; diff: string }) {
    this.proc.send(message);
  }
  async terminate(): Promise<void> {
    this.stopping = true;
    if (this.proc.exitCode === null) this.proc.kill("SIGKILL");
    await this.proc.exited;
  }
}
