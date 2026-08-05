#!/usr/bin/env node

import { readFileSync } from "node:fs";

const releaseTag = process.env.GITHUB_REF_NAME;
if (!releaseTag) {
  console.error("GITHUB_REF_NAME is required for release validation.");
  process.exit(1);
}

if (!releaseTag.startsWith("v")) {
  console.error(`Release tags must use a 'v' prefix: received "${releaseTag}".`);
  process.exit(1);
}

const releaseVersion = releaseTag.substring(1);
if (!releaseVersion || !/^\d+\.\d+\.\d+(?:[-+].*)?$/.test(releaseVersion)) {
  console.error(`Invalid semver tag "${releaseTag}".`);
  process.exit(1);
}

const readJson = (path) => JSON.parse(readFileSync(path, "utf8"));
const readCargoVersion = () => {
  const cargo = readFileSync("src-tauri/Cargo.toml", "utf8");
  const match = cargo.match(/^version\s*=\s*"([^"]+)"/m);
  if (!match?.[1]) {
    throw new Error("Unable to read Cargo package version.");
  }
  return match[1];
};

const sources = {
  package: readJson("package.json").version,
  tauri: readJson("src-tauri/tauri.conf.json").version,
  cargo: readCargoVersion(),
};

const values = Object.entries(sources).map(([source, value]) => [source, value]);
const mismatches = values.filter(([, value]) => value !== releaseVersion);

if (mismatches.length > 0) {
  console.error("Version mismatch for release tag.");
  for (const [source, value] of values) {
    console.error(`- ${source}: ${value}`);
  }
  console.error(`- tag: ${releaseVersion}`);
  process.exit(1);
}

const metadata = {
  tag: releaseTag,
  version: releaseVersion,
  sources,
  commit: process.env.GITHUB_SHA,
  workflow: process.env.GITHUB_WORKFLOW,
  runId: process.env.GITHUB_RUN_ID,
};

console.log(JSON.stringify(metadata, null, 2));
