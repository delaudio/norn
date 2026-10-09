// Golden-frame visual lane: regenerate native OpenTUI frames to a temporary
// file and compare them with the committed baseline. Fails on any layout, color
// or attribute regression and writes inspectable artifacts. Never accepts new
// images automatically; run `bun run frames:generate` and commit to update.

import { mkdir, readFile, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const baselineUrl = new URL("../storybook/generated/frames.json", import.meta.url);
const currentPath = join(tmpdir(), `norn-frames-${process.pid}.json`);
const renderer = fileURLToPath(new URL("./render-frames.tsx", import.meta.url));

const result = Bun.spawnSync([process.execPath, "run", renderer, currentPath], {
  stdout: "inherit",
  stderr: "inherit",
});
if (result.exitCode !== 0) {
  console.error("Native frame generation failed.");
  process.exit(result.exitCode ?? 1);
}

let baseline: string;
try {
  baseline = await readFile(baselineUrl, "utf8");
} catch {
  console.error("Missing baseline frames. Run `bun run frames:generate` and commit the result.");
  process.exit(1);
}
const current = await readFile(currentPath, "utf8");

if (baseline === current) {
  console.log("Native frames match the committed baseline.");
  process.exit(0);
}

const baselineFrames = JSON.parse(baseline) as Record<string, unknown>;
const currentFrames = JSON.parse(current) as Record<string, unknown>;
const ids = new Set([...Object.keys(baselineFrames), ...Object.keys(currentFrames)]);
const changed = [...ids].filter(
  (id) => JSON.stringify(baselineFrames[id]) !== JSON.stringify(currentFrames[id]),
);

const artifacts = new URL("../storybook/visual-artifacts/", import.meta.url);
await mkdir(artifacts, { recursive: true });
await writeFile(new URL("expected.json", artifacts), baseline);
await writeFile(new URL("current.json", artifacts), current);
await writeFile(
  new URL("diff.txt", artifacts),
  `Changed frames:\n${changed.map((id) => `- ${id}`).join("\n")}\n`,
);

console.error(`Frame regression detected in ${changed.length} scene(s):`);
for (const id of changed) {
  console.error(`- ${id}`);
}
console.error("Artifacts: ui/storybook/visual-artifacts/{expected,current,diff}.");
process.exit(1);
