import assert from "node:assert/strict";
import test from "node:test";

import { nextPageUrl, selectPreviousRelease } from "./resolve-previous-homebrew-release.mjs";

function release(tag, { complete = true, desktop = false, prerelease = false } = {}) {
  const version = tag.slice(1);
  const names = desktop
    ? [
        "norn-cask.rb",
        `Norn-${version}-macos-arm64.dmg`,
        ...(complete ? [`Norn-${version}-macos-x86_64.dmg`] : []),
      ]
    : [
        "norn.rb",
        `norn-${version}-macos-arm64.tar.gz`,
        ...(complete ? [`norn-${version}-macos-x86_64.tar.gz`] : []),
      ];
  return {
    tag_name: tag,
    draft: false,
    prerelease,
    assets: names.map((name) => ({
      name,
      browser_download_url: `https://example.invalid/${tag}/${name}`,
    })),
  };
}

test("selects the newest complete stable release before the candidate", () => {
  const selected = selectPreviousRelease(
    [release("v1.3.0", { complete: false }), release("v1.2.0"), release("v1.1.0")],
    "v1.4.0",
  );
  assert.equal(selected.tag, "v1.2.0");
  assert.equal(selected.bootstrap, false);
  assert.equal(selected.version, "1.2.0");
  assert.match(selected.manifestUrl, /v1\.2\.0\/norn\.rb$/);
});

test("selects a previous desktop release only when both notarized DMGs exist", () => {
  const selected = selectPreviousRelease(
    [release("v1.3.0", { complete: false, desktop: true }), release("v1.2.0", { desktop: true })],
    "v1.4.0",
    "desktop",
  );
  assert.equal(selected.tag, "v1.2.0");
  assert.match(selected.manifestUrl, /v1\.2\.0\/norn-cask\.rb$/);
});

test("rejects prereleases and releases newer than the candidate", () => {
  assert.throws(
    () =>
      selectPreviousRelease([release("v2.0.0"), release("v1.9.0", { prerelease: true })], "v1.8.0"),
    /No earlier stable release/,
  );
});

test("allows only an explicitly matching one-time bootstrap tag", () => {
  const selected = selectPreviousRelease([], "v1.0.0", "formula", "v1.0.0");
  assert.deepEqual(selected, {
    bootstrap: true,
    tag: "",
    version: "",
    manifestUrl: "",
  });
  assert.throws(
    () => selectPreviousRelease([], "v1.0.1", "formula", "v1.0.0"),
    /No earlier stable release/,
  );
});

test("a configured bootstrap tag never bypasses a complete prior release", () => {
  const selected = selectPreviousRelease(
    [release("v1.0.0")],
    "v1.1.0",
    "formula",
    "v1.1.0",
  );
  assert.equal(selected.bootstrap, false);
  assert.equal(selected.tag, "v1.0.0");
});

test("parses only GitHub API next-page links", () => {
  assert.equal(
    nextPageUrl(
      '<https://api.github.com/repositories/1/releases?per_page=100&page=2>; rel="next", <https://api.github.com/repositories/1/releases?per_page=100&page=4>; rel="last"',
    ),
    "https://api.github.com/repositories/1/releases?per_page=100&page=2",
  );
  assert.equal(nextPageUrl(undefined), undefined);
  assert.throws(
    () => nextPageUrl('<https://example.invalid/releases?page=2>; rel="next"'),
    /unexpected origin/,
  );
});
