#!/usr/bin/env node

import { createHash } from "node:crypto";
import { mkdirSync, readdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { basename, dirname, join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const scriptDirectory = dirname(fileURLToPath(import.meta.url));
const defaultTemplatePath = resolve(scriptDirectory, "../packaging/homebrew/norn.rb.template");
const architectures = ["arm64", "x86_64"];

function filesBelow(directory) {
  const files = [];
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) {
      files.push(...filesBelow(path));
    } else if (entry.isFile()) {
      files.push(path);
    }
  }
  return files;
}

function singleArtifact(files, name) {
  const matches = files.filter((path) => basename(path) === name);
  if (matches.length !== 1) {
    throw new Error(`Expected exactly one ${name}; found ${matches.length}.`);
  }
  return matches[0];
}

function verifiedChecksum(archivePath, checksumPath) {
  const checksumLine = readFileSync(checksumPath, "utf8").trim();
  const match = /^([0-9a-f]{64}) {2}([^/\s]+)$/.exec(checksumLine);
  if (!match || match[2] !== basename(archivePath)) {
    throw new Error(
      `Invalid checksum sidecar for ${basename(archivePath)}; expected an exact digest and filename.`,
    );
  }

  const actual = createHash("sha256").update(readFileSync(archivePath)).digest("hex");
  if (actual !== match[1]) {
    throw new Error(`Checksum mismatch for ${basename(archivePath)}.`);
  }
  return actual;
}

export function verifiedArchitectureChecksums({ version, artifactsDirectory, artifactName }) {
  const files = filesBelow(artifactsDirectory);
  return Object.fromEntries(
    architectures.map((architecture) => {
      const name = artifactName(version, architecture);
      const artifactPath = singleArtifact(files, name);
      const checksumPath = singleArtifact(files, `${name}.sha256`);
      return [architecture, verifiedChecksum(artifactPath, checksumPath)];
    }),
  );
}

export function renderFormula({
  version,
  artifactsDirectory,
  outputPath,
  templatePath = defaultTemplatePath,
}) {
  if (!/^\d+\.\d+\.\d+$/.test(version)) {
    throw new Error(`Release version must be stable semver, received ${version}.`);
  }
  if (!statSync(artifactsDirectory).isDirectory()) {
    throw new Error(`Artifact path is not a directory: ${artifactsDirectory}`);
  }

  const checksums = verifiedArchitectureChecksums({
    version,
    artifactsDirectory,
    artifactName: (releaseVersion, architecture) =>
      `norn-${releaseVersion}-macos-${architecture}.tar.gz`,
  });

  const template = readFileSync(templatePath, "utf8");
  const formula = template
    .replaceAll("{{VERSION}}", version)
    .replaceAll("{{ARM64_SHA256}}", checksums.arm64)
    .replaceAll("{{X86_64_SHA256}}", checksums.x86_64);

  if (/\{\{[^}]+\}\}/.test(formula) || /sha256\s+:no_check/.test(formula)) {
    throw new Error("Rendered formula contains unresolved or unchecked values.");
  }

  mkdirSync(dirname(outputPath), { recursive: true });
  writeFileSync(outputPath, formula);
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
    const result = renderFormula({
      version: args.get("version"),
      artifactsDirectory: resolve(args.get("artifacts-dir")),
      outputPath: resolve(args.get("output")),
    });
    console.log(`Rendered verified Homebrew formula at ${result.outputPath}.`);
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exit(1);
  }
}
