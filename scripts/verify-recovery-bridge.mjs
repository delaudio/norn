#!/usr/bin/env node
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";
import vm from "node:vm";
import ts from "typescript";

export const recoveryRevision = "72440eaef4177b809664bf6b563647d7659f3e52";
const root = fileURLToPath(new URL("../", import.meta.url));
function archived(path) {
  return execFileSync("git", ["show", `${recoveryRevision}:${path}`], {
    cwd: root,
    encoding: "utf8",
    timeout: 10000,
    maxBuffer: 2 * 1024 * 1024,
  });
}
export function verifyRecoveryBridge() {
  for (const name of ["docs/user-data-recovery.md", "docs/opentui-migration/user-data-recovery.md"])
    assert.ok(
      readFileSync(new URL(`../${name}`, import.meta.url), "utf8").includes(recoveryRevision),
      "Recovery guide must pin the complete merged bridge.",
    );
  const config = JSON.parse(archived("src-tauri/tauri.conf.json"));
  assert.equal(config.identifier, "app.norn.desktop");
  assert.equal(config.build.frontendDist, "../dist");
  assert.match(
    archived("src-tauri/Cargo.toml"),
    /custom-protocol\s*=\s*\["desktop",\s*"tauri\/custom-protocol"\]/,
  );
  assert.match(archived("src/components/settings/SettingsDialog.tsx"), /<DesktopDataExport\s*\/>/);
  assert.match(
    archived("src/components/settings/DesktopDataExport.tsx"),
    /downloadDesktopData\(\)/,
  );
  const identity = archived("src-tauri/src/runtime_identity.rs");
  assert.ok(identity.includes("app.lachesi.desktop") && identity.includes("Library/WebKit"));
  const moduleExports = {};
  const compiled = ts.transpileModule(archived("src/lib/desktopDataExport.ts"), {
    compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
  }).outputText;
  vm.runInNewContext(compiled, { exports: moduleExports, TextEncoder }, { timeout: 1000 });
  const routes = [];
  for (const platform of ["macOS/WebKit", "Linux/WebKitGTK", "Windows/WebView2"])
    for (const source of ["canonical", "legacy"]) {
      const prefix = source === "canonical" ? "norn.reviewPrompt.v1." : "lachesi.reviewPrompt.";
      const items = {
        [`${prefix}github:synthetic/recovery`]: "synthetic archived prompt",
        [source === "canonical" ? "norn.theme.v1" : "lachesi.theme"]: "light",
        "norn.menuBarSnapshot.v1": "excluded cache",
        "credential.token": "excluded synthetic marker",
      };
      const keys = Object.keys(items);
      const before = JSON.stringify(items);
      const result = moduleExports.collectDesktopData({
        length: keys.length,
        key: (index) => keys[index] ?? null,
        getItem: (key) => items[key] ?? null,
      });
      assert.equal(result.schemaVersion, 1);
      assert.equal(result.source, "desktopLocalStorage");
      assert.equal(result.entries.length, 2);
      assert.ok(!JSON.stringify(result).includes("excluded"));
      assert.equal(JSON.stringify(items), before);
      routes.push({ platform, source, exportedEntries: result.entries.length });
    }
  return {
    revision: recoveryRevision,
    applicationId: config.identifier,
    mode: "archived-source DOM synthetic verification",
    nativeProfileRecoveryObserved: false,
    routes,
  };
}
if (process.argv[1] && fileURLToPath(import.meta.url) === resolve(process.argv[1]))
  console.log(JSON.stringify(verifyRecoveryBridge(), null, 2));
