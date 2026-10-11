// Native selected-file navigation while operation events arrive. Fixture only:
// no process backend, credentials, filesystem mutation or provider calls.
import { writeFile } from "node:fs/promises";
import { createTestRenderer } from "@opentui/core/testing";
import { ShellStore } from "../src/app/store";
import { mountApp } from "../src/app/ui";
import { BackendClient } from "../src/transport/backendClient";
import { FakeBackendTransport } from "../src/transport/fakeTransport";

const output = Bun.argv[2];
if (!output) throw new Error("Usage: bun run scripts/streaming-diff-bench.ts OUTPUT.json");
const target = {
  kind: "provider" as const,
  provider: "github" as const,
  workspace: "fixture",
  repo: "burst",
};
const patch = [
  "diff --git a/large.ts b/large.ts",
  "--- a/large.ts",
  "+++ b/large.ts",
  "@@ -1,10000 +1,10000 @@",
  ...Array.from({ length: 10000 }, (_, i) => ` ROW-${i} STREAM_RENDER_BODY const value = ${i};`),
].join("\n");
class BurstTransport extends FakeBackendTransport {
  async request(method: string, params: Record<string, unknown>) {
    if (method === "review.start" || method === "operation.status")
      return { operationId: "burst-operation", state: "running", sequence: 0, error: null };
    return super.request(method, params);
  }
}
const results = [];
for (const mode of ["unified", "split"] as const) {
  const client = new BackendClient(
    new BurstTransport({
      repositories: [{ provider: "github", workspace: "fixture", repo: "burst", localPath: null }],
      diffText: patch,
      files: [{ path: "large.ts", status: "modified", additions: 0, deletions: 0, oldPath: null }],
      findings: [],
    }),
  );
  const store = new ShellStore();
  await store.connect(client);
  store.toggleDiffExpanded();
  if (mode === "split") store.toggleDiffMode();
  const renderer = await createTestRenderer({ width: 120, height: 30 });
  const ui = mountApp(renderer.renderer, store, client);
  const frame = async () => {
    await renderer.renderOnce();
    await Bun.sleep(0);
    await renderer.renderOnce();
  };
  let maximumMountedRows = 0;
  const times: number[] = [];
  let sequence = 0;
  try {
    await frame();
    await store.startReview(client);
    await Bun.sleep(1);
    await frame();
    store.applyEvent({
      operationId: "burst-operation",
      sequence: ++sequence,
      target,
      event: { kind: "state", state: "running" },
    });
    const start = performance.now();
    for (let sample = 0; sample < 100; sample++) {
      const batchStart = performance.now();
      // Identical operation ID with the wrong target must not contaminate logs
      // or advance the accepted sequence.
      store.applyEvent({
        operationId: "burst-operation",
        sequence: sequence + 1,
        target: { ...target, repo: "other" },
        event: { kind: "log", level: "info", message: "WRONG_TARGET_MARKER" },
      });
      for (let token = 0; token < 100; token++)
        store.applyEvent({
          operationId: "burst-operation",
          sequence: ++sequence,
          target,
          event: {
            kind: "log",
            level: "info",
            message: `BURST ${sequence}: ${"stream-token ".repeat(32)}`,
          },
        });
      const view = ui.scroll;
      if (!view) throw new Error("Diff panel missing under stream load");
      view.scrollTop = Math.floor(
        ((sample % 20) / 19) * Math.max(0, view.scrollHeight - view.viewport.height),
      );
      await frame();
      if (!renderer.captureCharFrame().includes("STREAM_RENDER_BODY"))
        throw new Error(
          `Native diff body missing under stream load: ${mode} sample ${sample}\n${renderer.captureCharFrame()}`,
        );
      if (
        store.getSnapshot().operation?.logs.some((log) => log.text.includes("WRONG_TARGET_MARKER"))
      )
        throw new Error("Wrong-target stream attached");
      const rows = view.getChildren().filter((child) => child.id.startsWith("diff-row-")).length;
      maximumMountedRows = Math.max(maximumMountedRows, rows);
      if (rows <= 0 || rows > 64)
        throw new Error("Unbounded or missing diff rows under stream load");
      if ((store.getSnapshot().operation?.logs.length ?? 0) > 100)
        throw new Error("Operation log ring is unbounded");
      times.push(performance.now() - batchStart);
    }
    const sorted = [...times].sort((a, b) => a - b);
    const p95Ms = sorted[Math.floor(sorted.length * 0.95)]!;
    const elapsedMs = performance.now() - start;
    const currentRssBytes = process.memoryUsage.rss();
    if (p95Ms > 20)
      throw new Error(`Streaming batch+navigation p95 exceeds 20 ms: ${p95Ms.toFixed(2)}`);
    if (store.getSnapshot().operation?.sequence !== sequence)
      throw new Error("Event sequence did not converge");
    if (currentRssBytes > 512 * 1024 * 1024)
      throw new Error("Streaming fixture exceeds 512 MiB RSS budget");
    results.push({
      mode,
      lines: 10000,
      events: 10000,
      samples: 100,
      eventsPerBatch: 100,
      maximumMountedRows,
      elapsedMs,
      batchAndFrameP95Ms: p95Ms,
      eventsPerSecond: (10000 * 1000) / elapsedMs,
      currentRssBytes,
      retainedLogs: store.getSnapshot().operation!.logs.length,
    });
  } finally {
    ui.unmount();
    renderer.renderer.destroy();
    client.close();
  }
}
const artifact = {
  schemaVersion: 1,
  host: process.platform,
  arch: process.arch,
  bunVersion: Bun.version,
  dimensions: { width: 120, height: 30 },
  selectedPatchSha256: new Bun.CryptoHasher("sha256").update(patch).digest("hex"),
  selectedPatchBytes: Buffer.byteLength(patch),
  budgets: {
    batchAndFrameP95Ms: 20,
    maximumMountedRows: 64,
    retainedLogs: 100,
    currentRssBytes: 512 * 1024 * 1024,
  },
  results,
  limitations: [
    "Synthetic normalized events; not a live agent/provider or installed pair.",
    "Two native frames per 100-event batch; not physical terminal latency.",
    "RSS is cumulative current process memory across modes.",
  ],
};
await writeFile(output, `${JSON.stringify(artifact, null, 2)}\n`);
console.log(JSON.stringify(artifact.results));
