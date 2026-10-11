import { SyntaxProcess, type SyntaxTask } from "./syntaxProcess";
import {
  type DiffSyntax,
  MAX_SYNTAX_BYTES,
  MAX_SYNTAX_ROWS,
  plainSyntax,
  type SyntaxReader,
  syntaxLanguage,
} from "./syntaxTypes";

interface Pending {
  path: string;
  diff: string;
  generation: number;
  resolve: (result: DiffSyntax) => void;
}
export function validSyntax(value: unknown, diff: string): value is DiffSyntax {
  if (!value || typeof value !== "object" || Array.isArray(value)) return false;
  const data = value as DiffSyntax;
  if (
    !["ready", "plain", "failed"].includes(data.status) ||
    !(
      data.language === null ||
      (typeof data.language === "string" && data.language.length <= 40)
    ) ||
    !(data.reason === null || (typeof data.reason === "string" && data.reason.length <= 256)) ||
    !data.rows ||
    typeof data.rows !== "object" ||
    Array.isArray(data.rows) ||
    ![Object.prototype, null].includes(Object.getPrototypeOf(data.rows))
  )
    return false;
  const lines = diff.split("\n");
  const entries = Object.entries(data.rows);
  if (entries.length > MAX_SYNTAX_ROWS || (data.status !== "ready" && entries.length !== 0))
    return false;
  let fragments = 0;
  for (const [key, parts] of entries) {
    if (!/^(0|[1-9]\d*)$/.test(key)) return false;
    const line = lines[Number(key)];
    if (line === undefined || ![" ", "+", "-"].includes(line[0] ?? "") || !Array.isArray(parts))
      return false;
    let text = "";
    for (const part of parts) {
      if (
        ++fragments > 16000 ||
        !part ||
        typeof part !== "object" ||
        typeof part.text !== "string" ||
        part.text.length > 4096 ||
        !(part.token === null || (typeof part.token === "string" && part.token.length <= 32))
      )
        return false;
      text += part.text;
      if (text.length > line.length) return false;
    }
    if (text !== line.slice(1)) return false;
  }
  return true;
}
export class SyntaxHighlighter implements SyntaxReader {
  private generation = 0;
  private pending: Pending | null = null;
  private busy = false;
  private poisoned = false;
  private cancelActive: (() => void) | null = null;
  private cached: { path: string; hash: string; result: DiffSyntax } | null = null;
  constructor(
    private readonly createWorker: () => SyntaxTask = () => new SyntaxProcess(),
    private readonly timeoutMs = 1500,
  ) {}
  highlight(path: string, diff: string): Promise<DiffSyntax> {
    const generation = ++this.generation;
    const language = syntaxLanguage(path);
    const hash = new Bun.CryptoHasher("sha256").update(diff).digest("hex");
    this.pending?.resolve(plainSyntax("Syntax selection changed."));
    this.pending = null;
    this.cancelActive?.();
    if (!language) return Promise.resolve(plainSyntax("No syntax grammar for this file."));
    if (Buffer.byteLength(diff) > MAX_SYNTAX_BYTES || diff.split("\n").length > MAX_SYNTAX_ROWS)
      return Promise.resolve(
        plainSyntax("Syntax limit reached; complete diff text remains available.", language),
      );
    if (this.cached?.path === path && this.cached.hash === hash)
      return Promise.resolve(this.cached.result);
    if (this.poisoned)
      return Promise.resolve(
        plainSyntax("Syntax worker unavailable; complete diff text remains available.", language),
      );
    return new Promise((resolve) => {
      this.pending = { path, diff, generation, resolve };
      this.pump();
    });
  }
  cancel() {
    this.generation++;
    this.pending?.resolve(plainSyntax("Syntax selection changed."));
    this.pending = null;
    this.cancelActive?.();
  }
  private pump() {
    if (this.busy || !this.pending) return;
    const request = this.pending;
    this.pending = null;
    this.busy = true;
    let worker: SyntaxTask;
    try {
      worker = this.createWorker();
    } catch {
      this.busy = false;
      request.resolve(
        plainSyntax("Syntax worker unavailable; complete diff text remains available."),
      );
      return;
    }
    let finished = false;
    const timer = setTimeout(
      () => finish(plainSyntax("Syntax timed out; complete diff text remains available.")),
      this.timeoutMs,
    );
    const finish = (result: DiffSyntax) => {
      if (finished) return;
      finished = true;
      clearTimeout(timer);
      worker.onmessage = null;
      worker.onerror = null;
      void (async () => {
        let terminationTimer: ReturnType<typeof setTimeout> | undefined;
        try {
          await Promise.race([
            Promise.resolve(worker.terminate()),
            new Promise<never>((_, reject) => {
              terminationTimer = setTimeout(
                () => reject(new Error("Syntax termination timed out")),
                1000,
              );
            }),
          ]);
        } catch {
          this.poisoned = true;
        } finally {
          if (terminationTimer) clearTimeout(terminationTimer);
        }
        const current = request.generation === this.generation;
        if (current && result.status === "ready")
          this.cached = {
            path: request.path,
            hash: new Bun.CryptoHasher("sha256").update(request.diff).digest("hex"),
            result,
          };
        request.resolve(current ? result : plainSyntax("Syntax selection changed."));
        this.cancelActive = null;
        this.busy = false;
        if (this.poisoned) {
          this.pending?.resolve(
            plainSyntax("Syntax worker unavailable; complete diff text remains available."),
          );
          this.pending = null;
        } else this.pump();
      })();
    };
    this.cancelActive = () => finish(plainSyntax("Syntax selection changed."));
    worker.onmessage = (event) => {
      const value: unknown = event.data;
      // The worker is packaged with the UI; still reject malformed output and
      // never accept a source-transforming result into the renderer.
      if (!validSyntax(value, request.diff))
        finish(plainSyntax("Syntax returned invalid data; complete diff text remains available."));
      else finish(value);
    };
    worker.onerror = (event) => {
      event.preventDefault();
      finish(plainSyntax("Syntax failed; complete diff text remains available."));
    };
    try {
      worker.postMessage({ path: request.path, diff: request.diff });
    } catch {
      finish(plainSyntax("Syntax could not start; complete diff text remains available."));
    }
  }
}
