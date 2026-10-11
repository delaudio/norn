import { expect, test } from "bun:test";
import { ShellStore } from "../src/app/store";
import { BackendClient } from "../src/transport/backendClient";
import { FakeBackendTransport } from "../src/transport/fakeTransport";

const target = { kind: "local" as const, localSnapshotSha256: "a".repeat(64) };
const bytes = new Uint8Array(600_001).map((_, index) => index % 251);
const hash = new Bun.CryptoHasher("sha256").update(bytes).digest("hex");
class ChunkTransport extends FakeBackendTransport {
  reads = 0;
  badHash = false;
  badOffset = false;
  async request(method: string, params: Record<string, unknown>) {
    if (method !== "file.preview") return super.request(method, params);
    this.reads++;
    const offset = Number(params.offset ?? 0);
    const end = Math.min(offset + 192 * 1024, bytes.length);
    return {
      path: params.path,
      side: params.side ?? "new",
      available: true,
      mimeType: "image/png",
      size: bytes.length,
      dataBase64: Buffer.from(bytes.slice(offset, end)).toString("base64"),
      offset,
      nextOffset: end < bytes.length ? (this.badOffset ? offset : end) : null,
      width: 128,
      height: 128,
      contentSha256: this.badHash && offset > 0 ? "b".repeat(64) : hash,
    };
  }
}
test("image chunks assemble within bounds and preserve the content version", async () => {
  const transport = new ChunkTransport();
  const client = new BackendClient(transport);
  const preview = await client.filePreview(target, "pic.png", "old");
  expect(preview.side).toBe("old");
  expect(Buffer.from(preview.dataBase64, "base64")).toEqual(Buffer.from(bytes));
  expect(transport.reads).toBe(4);
});
test("image content drift and non-progressing chunks fail clearly", async () => {
  const drift = new ChunkTransport();
  drift.badHash = true;
  await expect(new BackendClient(drift).filePreview(target, "pic.png")).rejects.toThrow("changed");
  const loop = new ChunkTransport();
  loop.badOffset = true;
  await expect(new BackendClient(loop).filePreview(target, "pic.png")).rejects.toThrow(
    "expected chunk",
  );
  expect(loop.reads).toBe(1);
});
test("changing file selection stops remaining image chunk requests", async () => {
  const transport = new ChunkTransport();
  await expect(
    new BackendClient(transport).filePreview(target, "pic.png", "new", () => transport.reads === 0),
  ).rejects.toThrow("selection changed");
  expect(transport.reads).toBe(1);
});
test("added/deleted image sides and renderer errors preserve the other side", async () => {
  const repositories = [
    { provider: "github" as const, workspace: "owner", repo: "repo", localPath: null },
  ];
  const client = new BackendClient(
    new FakeBackendTransport({
      repositories,
      files: [{ path: "pic.png", status: "added", additions: 0, deletions: 0, oldPath: null }],
    }),
  );
  const store = new ShellStore();
  await store.connect(client);
  await Bun.sleep(5);
  const preview = store.getSnapshot().preview;
  expect(preview?.before?.available).toBe(false);
  expect(preview?.after?.available).toBe(true);
  const requestId = preview!.requestId;
  store.setPreviewError("new", "native decoder unavailable", requestId);
  expect(store.getSnapshot().preview?.before?.available).toBe(false);
  expect(store.getSnapshot().preview?.afterError).toContain("unavailable");
  store.setPreviewError("old", "late stale error", requestId - 1);
  expect(store.getSnapshot().preview?.beforeError).toBeNull();
});

test("native image decoder accepts both deterministic sides and terminal protocol selection has a block fallback", async () => {
  const { NativeImage, resolveImageRenderProtocol } = await import("@opentui/core");
  const { createTerminalCapabilities } = await import("@opentui/core/testing");
  const transport = new FakeBackendTransport();
  for (const side of ["old", "new"]) {
    const response = await transport.request("file.preview", { path: "pic.png", side });
    const image = NativeImage.decode(Buffer.from(String(response.dataBase64), "base64"));
    try {
      expect(image.width).toBe(2);
      expect(image.height).toBe(2);
    } finally {
      image.dispose();
    }
  }
  expect(
    resolveImageRenderProtocol("auto", createTerminalCapabilities({ kitty_graphics: true }), false),
  ).toBe("kitty");
  expect(
    resolveImageRenderProtocol(
      "auto",
      createTerminalCapabilities({ kitty_graphics: false, sixel: true }),
      true,
    ),
  ).toBe("sixel");
  expect(
    resolveImageRenderProtocol(
      "auto",
      createTerminalCapabilities({ kitty_graphics: false, sixel: true }),
      false,
    ),
  ).toBe("blocks");
  expect(
    resolveImageRenderProtocol(
      "auto",
      createTerminalCapabilities({ kitty_graphics: true, multiplexer: "tmux" }),
      true,
    ),
  ).toBe("blocks");
  expect(resolveImageRenderProtocol("auto", null, false)).toBe("blocks");
});
