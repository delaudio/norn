import { expect, test } from "bun:test";
import fixture from "../../protocol/fixtures/valid/response-local-image.json";
import type { LocalSnapshotRef } from "../../src/protocol/generated/protocol";
import { BackendClient } from "../src/transport/backendClient";
import { FakeBackendTransport } from "../src/transport/fakeTransport";

const snapshot = fixture.result.request.snapshot as LocalSnapshotRef;
const bytes = Buffer.from(new Uint8Array(600_001).map((_, index) => index % 251));
const hash = new Bun.CryptoHasher("sha256").update(bytes).digest("hex");
class Images extends FakeBackendTransport {
  requests: Record<string, unknown>[] = [];
  change: (page: Record<string, unknown>, offset: number) => void = () => {};
  override async request(method: string, params: Record<string, unknown>) {
    if (method !== "local.snapshot.image")
      throw new Error(`Unexpected mutable fallback: ${method}`);
    this.requests.push(structuredClone(params));
    const offset = Number(params.offset);
    const end = Math.min(offset + Number(params.maxBytes), bytes.length);
    const page: Record<string, unknown> = {
      request: structuredClone(params),
      available: true,
      mimeType: "image/png",
      size: bytes.length,
      dataBase64: bytes.subarray(offset, end).toString("base64"),
      nextOffset: end < bytes.length ? end : null,
      contentSha256: hash,
      width: 128,
      height: 128,
      reason: null,
    };
    this.change(page, offset);
    return page;
  }
}
test("saved local image chunks retain the complete clone, layer, side and content identity", async () => {
  const transport = new Images();
  const result = await new BackendClient(transport).localSnapshotImage(
    snapshot,
    "staged",
    "picture.png",
    "old",
  );
  expect(Buffer.from(result.dataBase64, "base64")).toEqual(bytes);
  expect(transport.requests).toHaveLength(4);
  for (const [index, request] of transport.requests.entries()) {
    expect(request.snapshot).toEqual(snapshot);
    expect(request.layer).toBe("staged");
    expect(request.side).toBe("old");
    expect(request.contentSha256).toBe(index === 0 ? null : hash);
  }
});
test("saved local image reads reject foreign clones, layer changes and invalid availability", async () => {
  const changes: Images["change"][] = [
    (page) => {
      const request = page.request as typeof fixture.result.request;
      request.snapshot.rootId = "d".repeat(64);
    },
    (page) => {
      (page.request as typeof fixture.result.request).layer = "unstaged";
    },
    (page) => {
      page.reason = "legacySnapshot";
    },
    (page) => {
      page.dataBase64 = `${page.dataBase64}\n`;
    },
    (page, offset) => {
      if (offset > 0) page.contentSha256 = "d".repeat(64);
    },
    (page, offset) => {
      if (offset > 0) Object.assign(page, fixture.result, { request: page.request });
    },
    (page) => {
      page.nextOffset = 0;
    },
  ];
  for (const change of changes) {
    const transport = new Images();
    transport.change = change;
    await expect(
      new BackendClient(transport).localSnapshotImage(snapshot, "staged", "picture.png", "new"),
    ).rejects.toThrow();
  }
});
test("legacy captures report unavailable bytes without falling back to a live file", async () => {
  const transport = new Images();
  transport.change = (page) => Object.assign(page, fixture.result, { request: page.request });
  const result = await new BackendClient(transport).localSnapshotImage(
    snapshot,
    "staged",
    "picture.png",
    "new",
  );
  expect(result.available).toBe(false);
  expect(result.unavailableReason).toBe("legacySnapshot");
  expect(transport.requests).toHaveLength(1);
});
test("changing local image selection fences the response and stops subsequent reads", async () => {
  const transport = new Images();
  await expect(
    new BackendClient(transport).localSnapshotImage(
      snapshot,
      "unstaged",
      "picture.png",
      "new",
      () => transport.requests.length === 0,
    ),
  ).rejects.toThrow("selection changed");
  expect(transport.requests).toHaveLength(1);
});
