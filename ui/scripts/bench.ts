// Repeatable OpenTUI vertical-slice benchmark. Measures startup to the first
// usable frame, selection-to-diff-resolved latency, and bulk selection cost for
// the small / 100-file / 1000-file fixtures, and reports the runtime and
// platform. Run with `bun run bench`.
//
// The Ratatui baseline is measured separately (see
// docs/opentui-migration/vertical-slice.md); this script covers the OpenTUI side
// deterministically on the fake transport so numbers are comparable run to run.

import { createTestRenderer } from "@opentui/core/testing";
import { ShellStore } from "../src/app/store";
import { mountApp } from "../src/app/ui";
import { BackendClient } from "../src/transport/backendClient";
import { FakeBackendTransport } from "../src/transport/fakeTransport";

const repositories = [
  { provider: "github" as const, workspace: "acme", repo: "payments", localPath: null },
];

function files(count: number) {
  return Array.from({ length: count }, (_, index) => ({
    path: `src/generated/file-${index}.ts`,
    status: "modified",
    additions: 12,
    deletions: 3,
    oldPath: null,
  }));
}

async function settleDiff(store: ShellStore): Promise<void> {
  const selected = store.getSnapshot().files[store.getSnapshot().selectedFile]?.path;
  for (let attempt = 0; attempt < 2000; attempt += 1) {
    const snapshot = store.getSnapshot();
    if (!snapshot.diffLoading && snapshot.diff && snapshot.diff.path === selected) {
      return;
    }
    if (snapshot.error || snapshot.diffError) {
      throw new Error(`diff request failed: ${snapshot.error ?? snapshot.diffError}`);
    }
    await new Promise((resolve) => setTimeout(resolve, 1));
  }
  throw new Error(`diff for ${selected ?? "unknown file"} did not resolve`);
}

async function measure(count: number) {
  const client = new BackendClient(
    new FakeBackendTransport({ repositories, files: files(count), findings: [] }),
  );
  const store = new ShellStore();

  // Connect-to-frame latency ends at the first usable frame (client connect,
  // store construction, mount, and one render). It is not OS cold start.
  const startupStart = performance.now();
  await store.connect(client);
  const renderer = await createTestRenderer({ width: 120, height: 30 });
  const ui = mountApp(renderer.renderer, store, client, () => {});
  await renderer.renderOnce();
  const startupMs = round(performance.now() - startupStart);

  // Bulk selection cost: 200 moves applied, one render, no waiting on diffs.
  store.cycleFocus(); // repositories -> files
  const bulkStart = performance.now();
  for (let step = 0; step < 200; step += 1) {
    store.move(1);
  }
  await renderer.renderOnce();
  const bulkMovesMs = round(performance.now() - bulkStart);

  // Selection-to-diff-resolved latency: wait for each diff, then render.
  const sampled = Math.min(20, count);
  const latencyStart = performance.now();
  for (let step = 0; step < sampled; step += 1) {
    store.move(1);
    await settleDiff(store);
    await renderer.renderOnce();
  }
  const diffLatencyMs = sampled > 0 ? round((performance.now() - latencyStart) / sampled) : 0;

  // Process-wide RSS before teardown; fixtures share one process, so this is a
  // cumulative reading rather than an isolated per-fixture footprint.
  const processRssMb = Math.round(process.memoryUsage().rss / (1024 * 1024));
  ui.unmount();
  renderer.renderer.destroy();
  return {
    files: count,
    connectToFrameMs: startupMs,
    bulkMovesMs,
    diffLatencyMsPerFile: diffLatencyMs,
    processRssMb,
  };
}

function round(value: number): number {
  return Math.round(value * 100) / 100;
}

const results = [];
for (const count of [10, 100, 1000]) {
  results.push(await measure(count));
}

function largeDiff(lineCount: number): string {
  const body = Array.from({ length: lineCount }, (_, index) =>
    index % 2 === 0 ? `+line ${index}` : `-line ${index}`,
  ).join("\n");
  return `@@ -1,${lineCount} +1,${lineCount} @@\n${body}\n`;
}

async function measureLargeDiff(lineCount: number) {
  const client = new BackendClient(
    new FakeBackendTransport({
      repositories,
      files: files(1),
      findings: [],
      diffText: largeDiff(lineCount),
    }),
  );
  const store = new ShellStore();
  await store.connect(client);
  const renderer = await createTestRenderer({ width: 120, height: 30 });
  const mountStart = performance.now();
  const ui = mountApp(renderer.renderer, store, client, () => {});
  await renderer.renderOnce();
  const mountMs = round(performance.now() - mountStart);

  // Repaint a large diff in split mode to exercise the row mapping + culling.
  const repaintStart = performance.now();
  store.toggleDiffMode();
  await renderer.renderOnce();
  const splitRepaintMs = round(performance.now() - repaintStart);

  const processRssMb = Math.round(process.memoryUsage().rss / (1024 * 1024));
  ui.unmount();
  renderer.renderer.destroy();
  return { lines: lineCount, mountMs, splitRepaintMs, processRssMb };
}

const largeDiffResults = [];
for (const lineCount of [1000, 10000]) {
  largeDiffResults.push(await measureLargeDiff(lineCount));
}

console.log(
  JSON.stringify(
    {
      runtime: { bun: Bun.version, platform: process.platform, arch: process.arch },
      results,
      largeDiffResults,
    },
    null,
    2,
  ),
);
