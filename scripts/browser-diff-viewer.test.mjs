import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const serverSource = readFileSync("src-tauri/src/tui/diff_server.rs", "utf8");
const browserSource = readFileSync("src/browser-diff/BrowserDiffApp.tsx", "utf8");
const browserHtml = readFileSync("browser-diff.html", "utf8");
const browserViteConfig = readFileSync("vite.browser-diff.config.ts", "utf8");

test("browser viewer reuses the maintained desktop diff implementation", () => {
  assert.match(browserSource, /import \{ DiffViewer \} from "@\/components\/diff\/DiffViewer"/);
  assert.match(browserSource, /parseUnifiedDiff\(remoteState\.diff \?\? ""\)/);
  assert.match(browserSource, /mergeImageDiffstat\(/);
  assert.match(browserSource, /<DiffViewer/);

  assert.doesNotMatch(serverSource, /const HTML_PAGE/);
  assert.doesNotMatch(serverSource, /function parseUnifiedDiff/);
  assert.doesNotMatch(serverSource, /function renderSplitDiff/);
});

test("browser bundle is isolated, relative, and suitable for authenticated session routes", () => {
  assert.match(browserHtml, /src="\/src\/browser-diff\/main\.tsx"/);
  assert.match(browserViteConfig, /base: "\.\/"/);
  assert.match(browserViteConfig, /publicDir: false/);
  assert.match(browserViteConfig, /outDir: "dist\/browser-diff"/);
  assert.match(browserViteConfig, /emptyOutDir: true/);
});

test("server authenticates and constrains every browser bundle asset", () => {
  assert.match(serverSource, /browser_assets\.get\(route\)/);
  assert.match(serverSource, /MAX_BROWSER_ASSET_BYTES/);
  assert.match(serverSource, /MAX_BROWSER_ASSET_TOTAL_BYTES/);
  assert.match(serverSource, /Browser diff assets must not contain symbolic links/);
  assert.match(serverSource, /script-src 'self'/);
});
