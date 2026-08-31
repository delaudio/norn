#!/usr/bin/env node

import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";

const STATUS_BY_EXIT_CODE = new Map([
  [0, "ok"],
  [1, "warn"],
  [2, "fail"],
]);

export function validateReadinessReport(report, exitCode) {
  if (
    !report ||
    typeof report !== "object" ||
    report.schemaVersion !== "norn.readiness.v1" ||
    !["ok", "warn", "fail"].includes(report.status) ||
    !Array.isArray(report.issues)
  ) {
    throw new Error("Readiness probe output does not match the expected schema.");
  }

  const expectedStatus = STATUS_BY_EXIT_CODE.get(exitCode);
  if (!expectedStatus) {
    throw new Error(`Readiness probe returned unexpected exit ${exitCode}.`);
  }
  if (report.status !== expectedStatus) {
    throw new Error("Readiness probe exit code and report status do not match.");
  }
  return report.status;
}

function main() {
  const [reportPath, rawExitCode] = process.argv.slice(2);
  const exitCode = Number(rawExitCode);
  if (!reportPath || !Number.isInteger(exitCode)) {
    throw new Error("Usage: validate-readiness-report.mjs <report.json> <exit-code>");
  }

  let report;
  try {
    report = JSON.parse(readFileSync(reportPath, "utf8"));
  } catch {
    throw new Error("Readiness probe output is not valid JSON.");
  }
  console.log(validateReadinessReport(report, exitCode));
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  try {
    main();
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exit(1);
  }
}
