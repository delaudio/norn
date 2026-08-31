#!/usr/bin/env node

import { appendFileSync } from "node:fs";
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";

function parseVersion(tag) {
  const match = /^v(\d+)\.(\d+)\.(\d+)$/.exec(tag);
  return match ? match.slice(1).map(Number) : undefined;
}

function compareVersions(left, right) {
  for (let index = 0; index < left.length; index += 1) {
    if (left[index] !== right[index]) {
      return left[index] - right[index];
    }
  }
  return 0;
}

export function nextPageUrl(linkHeader) {
  if (!linkHeader) {
    return undefined;
  }
  for (const link of linkHeader.split(",")) {
    const match = /<([^>]+)>;\s*rel="([^"]+)"/.exec(link.trim());
    if (match?.[2] === "next") {
      const url = new URL(match[1]);
      if (url.origin !== "https://api.github.com") {
        throw new Error("GitHub pagination returned an unexpected origin.");
      }
      return url.href;
    }
  }
  return undefined;
}

export function candidateTagFromEnvironment(environment = process.env) {
  return environment.NORN_CANDIDATE_TAG;
}

async function fetchReleases(repository, headers) {
  let url = `https://api.github.com/repos/${repository}/releases?per_page=100`;
  const releases = [];
  for (let page = 1; page <= 100 && url; page += 1) {
    const response = await fetch(url, { headers });
    if (!response.ok) {
      throw new Error(`GitHub release lookup failed with HTTP ${response.status}.`);
    }
    const pageReleases = await response.json();
    if (!Array.isArray(pageReleases)) {
      throw new Error("GitHub release lookup returned an invalid response.");
    }
    releases.push(...pageReleases);
    url = nextPageUrl(response.headers.get("link"));
    if (page === 100 && url) {
      throw new Error("GitHub release lookup exceeded the 100-page safety limit.");
    }
  }
  return releases;
}

export function selectPreviousRelease(
  releases,
  candidateTag,
  channel = "formula",
  bootstrapTag = undefined,
) {
  const candidateVersion = parseVersion(candidateTag);
  if (!candidateVersion) {
    throw new Error(`Candidate tag must be stable semver, received ${candidateTag}.`);
  }

  if (!new Set(["formula", "desktop"]).has(channel)) {
    throw new Error(`Unsupported Homebrew release channel: ${channel}.`);
  }

  const eligible = releases.flatMap((release) => {
    const version = parseVersion(release.tag_name);
    if (
      !version ||
      release.draft ||
      release.prerelease ||
      compareVersions(version, candidateVersion) >= 0
    ) {
      return [];
    }
    const versionText = version.join(".");
    const manifestName = channel === "desktop" ? "norn-cask.rb" : "norn.rb";
    const requiredAssets =
      channel === "desktop"
        ? [
            manifestName,
            `Norn-${versionText}-macos-arm64.dmg`,
            `Norn-${versionText}-macos-x86_64.dmg`,
          ]
        : [
            manifestName,
            `norn-${versionText}-macos-arm64.tar.gz`,
            `norn-${versionText}-macos-x86_64.tar.gz`,
          ];
    const assets = new Map(
      (release.assets ?? []).map((asset) => [asset.name, asset.browser_download_url]),
    );
    if (!requiredAssets.every((asset) => assets.has(asset))) {
      return [];
    }
    return [{ release, version, manifestUrl: assets.get(manifestName) }];
  });

  eligible.sort((left, right) => compareVersions(right.version, left.version));
  const selected = eligible[0];
  if (!selected) {
    if (bootstrapTag === candidateTag) {
      return {
        bootstrap: true,
        tag: "",
        version: "",
        manifestUrl: "",
      };
    }
    throw new Error(
      `No earlier stable release with a verified ${channel} manifest and both architecture artifacts exists before ${candidateTag}.`,
    );
  }
  return {
    bootstrap: false,
    tag: selected.release.tag_name,
    version: selected.version.join("."),
    manifestUrl: selected.manifestUrl,
  };
}

async function main() {
  const repository = process.env.GITHUB_REPOSITORY;
  const candidateTag = candidateTagFromEnvironment();
  const outputPath = process.env.GITHUB_OUTPUT;
  const channel = process.env.NORN_RELEASE_CHANNEL ?? "formula";
  const bootstrapTag = process.env.NORN_HOMEBREW_BOOTSTRAP_TAG;
  const missingInputs = [
    ["GITHUB_REPOSITORY", repository],
    ["GITHUB_OUTPUT", outputPath],
    ["NORN_CANDIDATE_TAG", candidateTag],
  ]
    .filter(([, value]) => !value)
    .map(([name]) => name);
  if (missingInputs.length > 0) {
    throw new Error(`Missing required release resolver inputs: ${missingInputs.join(", ")}.`);
  }

  const headers = {
    Accept: "application/vnd.github+json",
    "X-GitHub-Api-Version": "2022-11-28",
  };
  if (process.env.GITHUB_TOKEN) {
    headers.Authorization = `Bearer ${process.env.GITHUB_TOKEN}`;
  }
  const selected = selectPreviousRelease(
    await fetchReleases(repository, headers),
    candidateTag,
    channel,
    bootstrapTag,
  );
  const manifestOutput = channel === "desktop" ? "cask_url" : "formula_url";
  let output = `bootstrap=${selected.bootstrap}\ntag=${selected.tag}\nversion=${selected.version}\n`;
  if (!selected.bootstrap) {
    output += `${manifestOutput}=${selected.manifestUrl}\n`;
  }
  appendFileSync(outputPath, output);
  console.log(
    selected.bootstrap
      ? `Authorized one-time ${channel} bootstrap for ${candidateTag}.`
      : `Resolved previous stable Homebrew release ${selected.tag}.`,
  );
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  main().catch((error) => {
    console.error(error instanceof Error ? error.message : String(error));
    process.exit(1);
  });
}
