#!/usr/bin/env node

import { existsSync, readFileSync } from "node:fs";

const sources = [
  ["package.json", "package version", () => {
    const pkg = JSON.parse(readFileSync("package.json", "utf8"));
    return pkg.version;
  }],
  ["src-tauri/tauri.conf.json", "tauri version", () => {
    const config = JSON.parse(readFileSync("src-tauri/tauri.conf.json", "utf8"));
    return config.version;
  }],
  ["src-tauri/Cargo.toml", "Cargo version", () => {
    const cargo = readFileSync("src-tauri/Cargo.toml", "utf8");
    const match = cargo.match(/^version\s*=\s*"([^"]+)"/m);
    return match?.[1];
  }],
];

const optionalFormulaPaths = [
  "Formula/norn.rb",
  "homebrew-tap/Formula/norn.rb",
];

for (const path of optionalFormulaPaths) {
  if (!existsSync(path)) {
    continue;
  }
  sources.push([
    path,
    "formula version",
    () => {
      const formula = readFileSync(path, "utf8");
      const versionMatch = formula.match(/^\s*version\s+"([^"]+)"/m);
      return versionMatch?.[1];
    },
  ]);
}

const versions = new Map();

for (const [path, label, readVersion] of sources) {
  const value = readVersion();
  if (!value) {
    console.error(`Unable to read ${label} from ${path}.`);
    process.exit(1);
  }
  versions.set(path, value);
}

const uniqueValues = new Set(versions.values());
if (uniqueValues.size === 1) {
  const [value] = uniqueValues;
  console.log(`Version sources are aligned: ${value}`);
  process.exit(0);
}

console.error("Version mismatch detected:");
for (const [path, value] of versions) {
  console.error(`- ${path}: ${value}`);
}
console.error("Align these sources before publishing.");
process.exit(1);
