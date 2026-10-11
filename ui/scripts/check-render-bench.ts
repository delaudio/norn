import { readFile } from "node:fs/promises";

interface Measurement {
  mode: string;
  firstFrameMs: number;
  scrollP95Ms: number;
  maximumMountedRows?: number;
}
interface ResultCase {
  case: string;
  selectedPath: string;
  selectedPatchBytes: number;
  totalReviewBytes: number;
  processRssMiB?: number;
  measurements: Measurement[];
}
interface Results {
  renderer: string;
  inputSha256: string;
  width: number;
  height: number;
  results: ResultCase[];
}

export function checkRenderBench(before: Results, after: Results): void {
  if (
    before.renderer !== "ratatui" ||
    after.renderer !== "opentui" ||
    before.inputSha256 !== after.inputSha256 ||
    before.width !== after.width ||
    before.height !== after.height ||
    before.results.length !== after.results.length ||
    after.results.length !== 6 ||
    new Set(after.results.map((entry) => entry.case)).size !== 6 ||
    new Set(before.results.map((entry) => entry.case)).size !== 6
  )
    throw new Error("Rendering workloads do not match");
  for (const entry of after.results) {
    const baseline = before.results.find((result) => result.case === entry.case);
    if (
      !baseline ||
      entry.selectedPath !== baseline.selectedPath ||
      entry.selectedPatchBytes !== baseline.selectedPatchBytes ||
      entry.totalReviewBytes !== baseline.totalReviewBytes
    )
      throw new Error(`Unmatched rendering input: ${entry.case}`);
    if (
      entry.processRssMiB === undefined ||
      !Number.isFinite(entry.processRssMiB) ||
      entry.processRssMiB <= 0 ||
      entry.processRssMiB > 512
    )
      throw new Error(`RSS budget exceeded: ${entry.case}`);
    for (const mode of ["unified", "split"]) {
      const old = baseline.measurements.find((row) => row.mode === mode);
      const next = entry.measurements.find((row) => row.mode === mode);
      if (
        !old ||
        !next ||
        ![old.firstFrameMs, old.scrollP95Ms, next.firstFrameMs, next.scrollP95Ms].every(
          (value) => Number.isFinite(value) && value >= 0,
        )
      )
        throw new Error(`Missing or invalid rendering measurement: ${entry.case}/${mode}`);
      // A bounded React reconciliation allowance plus the relative baseline;
      // the absolute limit keeps the same workload within a usable frame budget.
      const scrollBudget = Math.min(20, 2 * old.scrollP95Ms + 8);
      if (
        next.firstFrameMs > 100 ||
        next.scrollP95Ms > scrollBudget ||
        next.maximumMountedRows === undefined ||
        !Number.isInteger(next.maximumMountedRows) ||
        next.maximumMountedRows <= 0 ||
        next.maximumMountedRows > 64
      )
        throw new Error(`Rendering budget exceeded: ${entry.case}/${mode}`);
    }
  }
}

if (import.meta.main) {
  const [beforePath, afterPath] = Bun.argv.slice(2);
  if (!beforePath || !afterPath)
    throw new Error("Usage: bun run scripts/check-render-bench.ts RATATUI.json OPENTUI.json");
  const before = JSON.parse(await readFile(beforePath, "utf8")) as Results;
  const after = JSON.parse(await readFile(afterPath, "utf8")) as Results;
  checkRenderBench(before, after);
  console.log("Matched native rendering budgets pass.");
}
