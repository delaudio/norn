import { readFile, writeFile } from "node:fs/promises";
import { createTestRenderer } from "@opentui/core/testing";
import { ShellStore } from "../src/app/store";
import { mountApp } from "../src/app/ui";
import { BackendClient } from "../src/transport/backendClient";
import { FakeBackendTransport } from "../src/transport/fakeTransport";
import type { RenderInputs } from "./render-bench-inputs";

const inputPath = Bun.argv[2];
const outputPath = Bun.argv[3];
if (!inputPath || !outputPath)
  throw new Error("Usage: bun run scripts/render-bench.ts INPUT.json OUTPUT.json");
const bytes = await readFile(inputPath);
if (bytes.length > 2 * 1024 * 1024) throw new Error("Rendering input exceeds 2 MiB");
const inputs = JSON.parse(bytes.toString()) as RenderInputs;
if (
  inputs.schemaVersion !== 1 ||
  inputs.width !== 120 ||
  inputs.height !== 30 ||
  !Array.isArray(inputs.cases)
)
  throw new Error("Unsupported rendering input");
const results = [];
for (const entry of inputs.cases) {
  if (
    !entry.files.length ||
    entry.files.length > 1000 ||
    entry.selectedFile < 0 ||
    entry.selectedFile >= entry.files.length
  )
    throw new Error("Invalid rendering case");
  const patches = new Map(entry.files.map((file) => [file.path, file.patch]));
  class SharedTransport extends FakeBackendTransport {
    async request(method: string, params: Record<string, unknown>) {
      if (method === "diff.file") {
        const diff = patches.get(String(params.path));
        if (!diff) throw new Error("Unknown shared fixture path");
        return { target: params.target, path: params.path, diff, truncated: false };
      }
      return super.request(method, params);
    }
  }
  const client = new BackendClient(
    new SharedTransport({
      repositories: [{ provider: "github", workspace: "acme", repo: "payments", localPath: null }],
      files: entry.files.map((file) => ({
        path: file.path,
        status: "modified",
        oldPath: null,
        additions: 10,
        deletions: 10,
      })),
      findings: [],
    }),
  );
  const store = new ShellStore();
  const connectStart = performance.now();
  await store.connect(client);
  store.cycleFocus();
  store.move(entry.selectedFile);
  const selected = entry.files[entry.selectedFile]!;
  for (let attempt = 0; attempt < 2000; attempt++) {
    if (!store.getSnapshot().diffLoading && store.getSnapshot().diff?.path === selected.path) break;
    if (store.getSnapshot().error || store.getSnapshot().diffError)
      throw new Error("Shared diff failed to load");
    await Bun.sleep(1);
  }
  if (store.getSnapshot().diff?.path !== selected.path)
    throw new Error("Shared diff load timed out");
  const fakeConnectAndSelectMs = performance.now() - connectStart;
  store.toggleDiffExpanded();
  const measurements = [];
  for (const mode of ["unified", "split"] as const) {
    if (store.getSnapshot().diffMode !== mode) store.toggleDiffMode();
    const renderer = await createTestRenderer({ width: inputs.width, height: inputs.height });
    const frame = async () => {
      await renderer.renderOnce();
      await Bun.sleep(0);
      await renderer.renderOnce();
    };
    const start = performance.now();
    const ui = mountApp(renderer.renderer, store, client);
    try {
      await frame();
      const firstFrameMs = performance.now() - start;
      const scroll = ui.scroll;
      if (!scroll) throw new Error("Native diff panel missing");
      if (
        entry.name !== "real-norn-backend" &&
        !renderer.captureCharFrame().includes("RENDER_BENCH")
      )
        throw new Error("Native diff body missing");
      if (
        entry.name === "real-norn-backend" &&
        !renderer.captureCharFrame().includes("MAX_FRAME_BYTES")
      )
        throw new Error("Real native diff body missing");
      const times: number[] = [];
      let maximumMountedRows = 0;
      for (let sample = 0; sample < 100; sample++) {
        const start = performance.now();
        scroll.scrollTop = Math.floor(
          (sample * Math.max(0, scroll.scrollHeight - scroll.viewport.height)) / 99,
        );
        await frame();
        if (!ui.scroll) throw new Error("Native diff panel disappeared");
        times.push(performance.now() - start);
        maximumMountedRows = Math.max(
          maximumMountedRows,
          scroll.getChildren().filter((row) => row.id.startsWith("diff-row-")).length,
        );
      }
      times.sort((a, b) => a - b);
      measurements.push({
        mode,
        firstFrameMs,
        scrollP95Ms: times[94],
        samples: 100,
        maximumMountedRows,
        contentRows: scroll.scrollHeight,
      });
    } finally {
      ui.unmount();
      renderer.renderer.destroy();
    }
  }
  client.close();
  results.push({
    case: entry.name,
    files: entry.files.length,
    selectedPath: selected.path,
    selectedPatchBytes: Buffer.byteLength(selected.patch),
    totalReviewBytes: entry.files.reduce((sum, file) => sum + Buffer.byteLength(file.patch), 0),
    fakeConnectAndSelectMs,
    measurements,
    processRssMiB: process.memoryUsage().rss / (1024 * 1024),
  });
}
await writeFile(
  outputPath,
  JSON.stringify(
    {
      renderer: "opentui",
      width: inputs.width,
      height: inputs.height,
      runtime: Bun.version,
      platform: process.platform,
      arch: process.arch,
      inputSha256: new Bun.CryptoHasher("sha256").update(bytes).digest("hex"),
      results,
    },
    null,
    2,
  ),
);
console.log(`Wrote OpenTUI native rendering results to ${outputPath}`);
