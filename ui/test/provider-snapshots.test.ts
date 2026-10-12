import { expect, test } from "bun:test";
import type { ProviderSnapshotInfo } from "../../src/protocol/generated/protocol";
import { BackendClient } from "../src/transport/backendClient";
import { FakeBackendTransport } from "../src/transport/fakeTransport";

const repository = { provider: "github", workspace: "Owner", repo: "Repo" } as const;
const snapshot = { repository, prId: 7, snapshotId: "e".repeat(64) };
const info: ProviderSnapshotInfo = {
  request: { repository, prId: 7 },
  snapshot,
  title: "Owned PR",
  sourceBranch: "feature",
  destinationBranch: "main",
  headSha: "a".repeat(40),
  destinationSha: "b".repeat(40),
  baseSha: "c".repeat(40),
  patchSha256: "d".repeat(64),
  fileCount: 2,
  patchBytes: 150,
  expiresAt: 1900000000000,
};
const file = (path: string) => ({
  path,
  oldPath: null,
  status: "modified",
  additions: 1,
  deletions: 1,
  binary: false,
});
class SnapshotTransport extends FakeBackendTransport {
  calls: Array<{ method: string; params: Record<string, unknown> }> = [];
  constructor(
    private readonly answer: (
      method: string,
      params: Record<string, unknown>,
    ) => Record<string, unknown>,
  ) {
    super();
  }
  async request(method: string, params: Record<string, unknown>) {
    this.calls.push({ method, params });
    return this.answer(method, params);
  }
}

test("immutable image chunks verify scope, hash, metadata, missing sides and stale generation", async () => {
  const bytes = Buffer.from("owned immutable raster bytes");
  const sha = new Bun.CryptoHasher("sha256").update(bytes).digest("hex");
  for (const fault of ["none", "scope", "hash", "mime", "length", "absent", "stale"]) {
    let current = true;
    const transport = new SnapshotTransport((_method, request) => {
      const offset = Number(request.offset);
      const end = Math.min(offset + 8, bytes.length);
      if (fault === "stale") current = false;
      return {
        request: fault === "scope" ? { ...request, snapshot: { ...snapshot, prId: 8 } } : request,
        available: fault !== "absent",
        mimeType: fault === "absent" ? "" : fault === "mime" ? "text/html" : "image/png",
        size: fault === "absent" ? 0 : bytes.length,
        dataBase64: fault === "absent" ? "" : bytes.subarray(offset, end).toString("base64"),
        nextOffset:
          fault === "absent"
            ? null
            : fault === "length"
              ? end + 1
              : end < bytes.length
                ? end
                : null,
        contentSha256: fault === "absent" ? null : fault === "hash" ? "f".repeat(64) : sha,
        width: fault === "absent" ? null : 2,
        height: fault === "absent" ? null : 3,
      };
    });
    const result = new BackendClient(transport).providerSnapshotImage(
      snapshot,
      "picture.png",
      "new",
      () => current,
    );
    if (fault === "none") {
      expect((await result).dataBase64).toBe(bytes.toString("base64"));
      expect(transport.calls).toHaveLength(4);
      expect(transport.calls.slice(1).every((call) => call.params.contentSha256 === sha)).toBe(
        true,
      );
    } else if (fault === "absent") expect((await result).available).toBe(false);
    else await expect(result).rejects.toThrow();
  }
});

test("immutable browser links reject wrong scope, nonloopback URLs and dishonest launch responses", async () => {
  for (const fault of ["none", "scope", "host", "port", "token", "launch"]) {
    const transport = new SnapshotTransport((_method, request) => ({
      request: fault === "scope" ? { snapshot: { ...snapshot, prId: 8 } } : request,
      url: `http://${fault === "host" ? "example.test" : "127.0.0.1"}:${fault === "port" ? 65536 : 1234}/session/${fault === "token" ? "bad" : "a".repeat(64)}/`,
      launchRequested: false,
      launchError: fault === "launch" ? "failure" : null,
    }));
    const result = new BackendClient(transport).providerSnapshotBrowser(snapshot);
    if (fault === "none") expect((await result).launchRequested).toBe(false);
    else await expect(result).rejects.toThrow();
  }
});
test("snapshot metadata keeps destination and merge base distinct and checks exact handle scope", async () => {
  for (const fault of ["none", "provider", "pr", "sha", "unknown"]) {
    const transport = new SnapshotTransport(() => {
      const result = structuredClone(info) as unknown as Record<string, unknown>;
      if (fault === "provider")
        result.snapshot = { ...snapshot, repository: { ...repository, provider: "bitbucket" } };
      if (fault === "pr") result.snapshot = { ...snapshot, prId: 8 };
      if (fault === "sha") result.baseSha = "f".repeat(64);
      if (fault === "unknown") result.privateBody = "untrusted";
      return result;
    });
    const result = new BackendClient(transport).providerSnapshot(info.request);
    if (fault === "none") {
      expect((await result).baseSha).toBe(info.baseSha);
      expect(info.baseSha).not.toBe(info.destinationSha);
    } else await expect(result).rejects.toThrow();
  }
});
test("complete inventory accepts size-limited pages and rejects duplicates, totals, cursors and paths", async () => {
  for (const fault of ["none", "duplicate", "total", "cursor", "path", "empty", "echo"]) {
    const transport = new SnapshotTransport((_method, request) => {
      const offset = request.offset as number;
      return {
        request:
          fault === "echo"
            ? { ...request, snapshot: { ...snapshot, snapshotId: "f".repeat(64) } }
            : request,
        files:
          fault === "empty"
            ? []
            : [
                file(
                  fault === "path"
                    ? "../x"
                    : offset === 0 || fault === "duplicate"
                      ? "first"
                      : "second",
                ),
              ],
        total: fault === "total" ? 3 : 2,
        nextOffset: fault === "cursor" ? 2 : offset === 0 ? 1 : null,
      };
    });
    const result = new BackendClient(transport).providerSnapshotInventory(info);
    if (fault === "none") {
      expect((await result).map((f) => f.path)).toEqual(["first", "second"]);
      expect(transport.calls).toHaveLength(2);
    } else await expect(result).rejects.toThrow();
  }
});
test("UTF-8 diff pages reassemble exact bytes with a stable verified hash", async () => {
  const patch = "diff --git a/世界 b/世界\n" + "世".repeat(23000);
  const bytes = Buffer.from(patch);
  const hash = Buffer.from(await crypto.subtle.digest("SHA-256", bytes)).toString("hex");
  for (const fault of ["none", "hash", "total", "cursor", "surrogate"]) {
    const transport = new SnapshotTransport((_method, request) => {
      const offset = request.offset as number;
      let end = Math.min(offset + 65536, bytes.length);
      while (end < bytes.length && ((bytes[end] ?? 0) & 0xc0) === 0x80) end--;
      return {
        request,
        text: fault === "surrogate" ? "\ud800" : bytes.subarray(offset, end).toString("utf8"),
        totalBytes: bytes.length + (fault === "total" && offset > 0 ? 1 : 0),
        contentSha256: fault === "hash" ? "0".repeat(64) : hash,
        nextOffset: fault === "cursor" ? offset + 1 : end < bytes.length ? end : null,
      };
    });
    const result = new BackendClient(transport).providerSnapshotPatch(snapshot, "世界");
    if (fault === "none") {
      expect(await result).toBe(patch);
      expect(transport.calls).toHaveLength(2);
    } else await expect(result).rejects.toThrow();
  }
});
test("stale generations and invalid snapshot inputs stop before further transport reads", async () => {
  let current = true;
  const transport = new SnapshotTransport((_method, request) => {
    current = false;
    return { request, files: [file("x")], total: 2, nextOffset: 1 };
  });
  await expect(
    new BackendClient(transport).providerSnapshotInventory(info, () => current),
  ).rejects.toThrow("selection changed");
  expect(transport.calls).toHaveLength(1);
  await expect(
    new BackendClient(transport).providerSnapshotPatch(snapshot, "../x"),
  ).rejects.toThrow("path");
  expect(transport.calls).toHaveLength(1);
});
