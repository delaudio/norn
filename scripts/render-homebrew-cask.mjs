#!/usr/bin/env node

import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

import { verifiedArchitectureChecksums } from "./render-homebrew-formula.mjs";

const scriptDirectory = dirname(fileURLToPath(import.meta.url));
const defaultTemplatePath = resolve(scriptDirectory, "../packaging/homebrew/norn-cask.rb.template");

export function renderCask({
  version,
  artifactsDirectory,
  outputPath,
  templatePath = defaultTemplatePath,
}) {
  if (!/^\d+\.\d+\.\d+$/.test(version)) {
    throw new Error(`Release version must be stable semver, received ${version}.`);
  }
  const checksums = verifiedArchitectureChecksums({
    version,
    artifactsDirectory,
    artifactName: (releaseVersion, architecture) =>
      `Norn-${releaseVersion}-macos-${architecture}.dmg`,
  });
  const template = readFileSync(templatePath, "utf8");
  const cask = template
    .replaceAll("{{VERSION}}", version)
    .replaceAll("{{ARM64_SHA256}}", checksums.arm64)
    .replaceAll("{{X86_64_SHA256}}", checksums.x86_64);

  if (/\{\{[^}]+\}\}/.test(cask) || /sha256\s+:no_check/.test(cask)) {
    throw new Error("Rendered cask contains unresolved or unchecked values.");
  }
  mkdirSync(dirname(outputPath), { recursive: true });
  writeFileSync(outputPath, cask);
  return { checksums, outputPath };
}

function parseArguments(argv) {
  const values = new Map();
  for (let index = 0; index < argv.length; index += 2) {
    const flag = argv[index];
    const value = argv[index + 1];
    if (!flag?.startsWith("--") || value === undefined) {
      throw new Error(`Invalid argument near ${flag ?? "end of command"}.`);
    }
    values.set(flag.slice(2), value);
  }
  for (const required of ["version", "artifacts-dir", "output"]) {
    if (!values.has(required)) {
      throw new Error(`Missing required --${required} argument.`);
    }
  }
  return values;
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  try {
    const args = parseArguments(process.argv.slice(2));
    const result = renderCask({
      version: args.get("version"),
      artifactsDirectory: resolve(args.get("artifacts-dir")),
      outputPath: resolve(args.get("output")),
    });
    console.log(`Rendered verified Homebrew cask at ${result.outputPath}.`);
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exit(1);
  }
}
