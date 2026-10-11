// Only published, tracked source is read. No working-tree/config/credential data.
import { writeFile } from "node:fs/promises";
import { resolve } from "node:path";

export interface RenderCase {
  name: string;
  files: { path: string; patch: string }[];
  selectedFile: number;
}
export interface RenderInputs {
  schemaVersion: 1;
  width: number;
  height: number;
  source: { base: string; head: string; path: string };
  cases: RenderCase[];
}

function patch(path: string, count: number) {
  const body = Array.from(
    { length: count },
    (_, index) => `${index % 2 ? "+" : "-"}RENDER_BENCH row ${index}`,
  ).join("\n");
  return `diff --git a/${path} b/${path}\n--- a/${path}\n+++ b/${path}\n@@ -1,${Math.ceil(count / 2)} +1,${Math.floor(count / 2)} @@\n${body}\n`;
}

export async function generateInputs(): Promise<RenderInputs> {
  const source = {
    base: "cdf6bdf0e8f2532578db47b4a05de9521e2e24ee",
    head: "e9c1ec78a225a9a9c7b55127c65a09b294afc192",
    path: "src-tauri/src/backend.rs",
  };
  const process = Bun.spawn(
    ["git", "diff", "--no-ext-diff", "--no-textconv", source.base, source.head, "--", source.path],
    { cwd: resolve(import.meta.dir, "../.."), stdout: "pipe", stderr: "pipe" },
  );
  const real = await new Response(process.stdout).text();
  const error = await new Response(process.stderr).text();
  if ((await process.exited) !== 0) throw new Error(`Published source diff unavailable: ${error}`);
  if (!real.startsWith("diff --git ") || real.length > 1_000_000)
    throw new Error("Published source diff is empty or exceeds the fixture bound");
  return {
    schemaVersion: 1,
    width: 120,
    height: 30,
    source,
    cases: [
      ...[10, 100, 1000].map((count) => ({
        name: `files-${count}`,
        selectedFile: count - 1,
        files: Array.from({ length: count }, (_, index) => {
          const path = `src/generated/file-${index}.ts`;
          return { path, patch: patch(path, 20) };
        }),
      })),
      ...[1000, 10000].map((count) => ({
        name: `lines-${count}`,
        selectedFile: 0,
        files: [{ path: "src/generated/large.ts", patch: patch("src/generated/large.ts", count) }],
      })),
      { name: "real-norn-backend", selectedFile: 0, files: [{ path: source.path, patch: real }] },
    ],
  };
}

if (import.meta.main) {
  const destination = Bun.argv[2];
  if (!destination) throw new Error("Usage: bun run scripts/render-bench-inputs.ts OUTPUT.json");
  await writeFile(destination, JSON.stringify(await generateInputs()));
  console.log(`Wrote shared rendering inputs to ${destination}`);
}
