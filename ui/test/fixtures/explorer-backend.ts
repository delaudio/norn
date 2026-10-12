import type { RepositoryExploreFile } from "../../../src/protocol/generated/protocol";
import { FakeBackendTransport } from "../../src/transport/fakeTransport";

const sha = (text: string | Buffer) => new Bun.CryptoHasher("sha256").update(text).digest("hex");
const root = (provider: string) => (provider === "github" ? "a" : "b").repeat(64);
export class ExplorerFixture extends FakeBackendTransport {
  calls: Array<{ method: string; params: Record<string, unknown> }> = [];
  nextGate: { method: string; wait: Promise<void> } | null = null;
  alter: (result: Record<string, unknown>) => void = () => {};
  files: RepositoryExploreFile[] = [
    { path: "README.md", oldPath: null, status: "unchanged" },
    { path: "deleted.txt", oldPath: null, status: "deleted" },
    { path: "images/binary.png", oldPath: null, status: "untracked" },
    { path: "src/--modified ü.ts", oldPath: null, status: "modified" },
    { path: "src/conflict.ts", oldPath: null, status: "conflicted" },
    { path: "src/new.ts", oldPath: "src/old.ts", status: "renamed" },
    { path: "untracked.txt", oldPath: null, status: "untracked" },
  ];
  documents: Record<string, string> = {
    "README.md":
      "# Owned explorer\n\n**Readable** Markdown.\n\n- first item\n- second item\n\n```ts\nconst owned = true;\n```\n",
    "src/--modified ü.ts":
      "🙂 owned first line\n" +
      "bounded ordinary line\n".repeat(3000) +
      "TARGET_AFTER_FIRST_WINDOW\n",
    "src/conflict.ts": "<<<<<<< left\nleft\n=======\nright\n>>>>>>> right\n",
    "src/new.ts": "renamed owned source\n",
    "untracked.txt": "owned untracked source\n",
  };
  constructor() {
    super({
      repositories: ["github", "bitbucket"].map((provider) => ({
        provider,
        workspace: "owned",
        repo: "same",
        localPath: `/owned/${provider}`,
      })),
      targets: [],
    });
  }
  override async start() {
    const ready = await super.start();
    return {
      ...ready,
      capabilities: [
        ...ready.capabilities,
        "repository-explorer-reads",
        "repository-explorer-windowed-find",
      ],
    };
  }
  override async request(method: string, params: Record<string, unknown>) {
    if (!method.startsWith("repository.explore.")) return super.request(method, params);
    const request = structuredClone(params);
    this.calls.push({ method, params: request });
    if (this.nextGate?.method === method) {
      const wait = this.nextGate.wait;
      this.nextGate = null;
      await wait;
    }
    const context = request.context as { repository: { provider: string }; rootId: string };
    const path = String(request.path ?? "");
    const text = this.documents[path] ?? "";
    const offset = Number(request.offset ?? 0);
    let value: Record<string, unknown>;
    if (method.endsWith(".info")) {
      const repository = request.repository as { provider: string };
      value = {
        request,
        context: { repository, rootId: root(repository.provider) },
        branch: "owned-main",
        headSha: "d".repeat(40),
      };
    } else {
      if (context.rootId !== root(context.repository.provider))
        throw new Error("Selected clone changed.");
      if (method.endsWith(".files")) {
        const files = this.files
          .filter(
            (file) =>
              (!request.changedOnly || file.status !== "unchanged") &&
              file.path.toLowerCase().includes(String(request.query).trim().toLowerCase()),
          )
          .sort((a, b) => Buffer.compare(Buffer.from(a.path), Buffer.from(b.path)));
        const version = sha(JSON.stringify(files));
        if (request.version !== null && request.version !== version)
          throw new Error("Inventory changed.");
        const end = Math.min(files.length, offset + Number(request.limit));
        value = {
          request,
          version,
          total: files.length,
          files: files.slice(offset, end),
          nextOffset: end < files.length ? end : null,
        };
      } else if (method.endsWith(".content") || method.endsWith(".diff")) {
        const diff = method.endsWith(".diff");
        const binary = path.endsWith(".png");
        const document = diff
          ? `diff --git a/${path} b/${path}\n--- a/${path}\n+++ b/${path}\n@@ -1 +1 @@\n-owned base\n+owned change\n`
          : text;
        const bytes = binary && !diff ? Buffer.from([0, 255, 1]) : Buffer.from(document);
        const hash = path === "deleted.txt" && !diff ? null : sha(bytes);
        if (request.contentSha256 !== null && request.contentSha256 !== hash)
          throw new Error("Preview changed.");
        let end = Math.min(bytes.length, offset + Number(request.maxBytes));
        while (end < bytes.length && (bytes[end]! & 192) === 128) end--;
        const displayed =
          (binary && !diff) || hash === null ? "" : bytes.subarray(offset, end).toString("utf8");
        value = diff
          ? {
              request,
              baseSha: "d".repeat(40),
              totalBytes: bytes.length,
              contentSha256: hash,
              text: displayed,
              nextOffset: end < bytes.length ? end : null,
            }
          : {
              request,
              state: binary ? "binary" : hash === null ? "deleted" : "text",
              size: hash === null ? null : bytes.length,
              contentSha256: hash,
              text: displayed,
              nextOffset: binary || hash === null ? null : end < bytes.length ? end : null,
            };
      } else if (method.endsWith(".blame")) {
        if (request.contentSha256 !== sha(text)) throw new Error("Preview changed.");
        const count = !text ? 0 : text.split("\n").length - (text.endsWith("\n") ? 1 : 0);
        const end = Math.min(count, offset + Number(request.limit));
        value = {
          request,
          baseSha: "d".repeat(40),
          available: true,
          totalLines: count,
          lines: Array.from({ length: end - offset }, (_, index) => ({
            line: offset + index + 1,
            sha: "d".repeat(40),
            shortSha: "d".repeat(8),
            author: "Owned Author",
            authorEmail: "owned@example.invalid",
            authorTime: 1_700_000_000,
            summary: "Owned complete title",
            message: "Owned complete title\n\nFull commit body.\nRetained final paragraph.",
          })),
          nextOffset: end < count ? end : null,
        };
      } else if (method.endsWith(".find")) {
        const document =
          request.source === "diff"
            ? `diff --git a/${path} b/${path}\n--- a/${path}\n+++ b/${path}\n@@ -1 +1 @@\n-owned base\n+owned change\n`
            : text;
        if (request.contentSha256 !== sha(document)) throw new Error("Preview changed.");
        const query = String(request.query).toLowerCase(),
          lower = document.toLowerCase();
        const matches = [];
        let start = lower.indexOf(query);
        while (start !== -1) {
          const prefix = document.slice(0, start),
            end = start + query.length;
          matches.push({
            startUtf16: start,
            endUtf16: end,
            startByte: Buffer.byteLength(prefix),
            endByte: Buffer.byteLength(document.slice(0, end)),
            line: prefix.split("\n").length,
            columnUtf16: start - prefix.lastIndexOf("\n") - 1,
          });
          start = lower.indexOf(query, end);
        }
        const end = Math.min(matches.length, offset + Number(request.limit));
        value = {
          request,
          total: matches.length,
          matches: matches.slice(offset, end),
          nextOffset: end < matches.length ? end : null,
        };
      } else throw new Error(`Unsupported owned explorer fixture: ${method}`);
    }
    this.alter(value);
    return value;
  }
}
