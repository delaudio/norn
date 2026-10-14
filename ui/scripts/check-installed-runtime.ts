// Owned temporary installation: no user config, providers, credentials or PATH runtime.
import assert from "node:assert/strict";
import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
// @ts-expect-error The installer is a Node ESM entry also usable from Bun.
import { installLocal } from "../../scripts/install-local.mjs";

const root = await mkdtemp(join(tmpdir(), "norn-installed-runtime-"));
const suffix = process.platform === "win32" ? ".exe" : "";
try {
  const prefix = join(root, "prefix with spaces");
  const config = join(root, "owned config");
  await mkdir(config);
  installLocal({
    prefix,
    sourceDirectory: resolve(process.env.NORN_TEST_BUILD_DIR ?? "src-tauri/target/debug"),
    components: ["cli", "tui"],
  });
  const env = {
    PATH: join(root, "no runtimes here"),
    SystemRoot: process.env.SystemRoot,
    TEMP: root,
    TMP: root,
    HOME: root,
    USERPROFILE: root,
    NORN_CONFIG_DIR: config,
    NORN_DATA_DIR: join(root, "data"),
    NORN_REVIEW_DATA_DIR: join(root, "reviews"),
  };
  async function output(name: string, args: string[]) {
    const child = Bun.spawn([join(prefix, "bin", name + suffix), ...args], {
      cwd: root,
      env,
      stdin: "ignore",
      stdout: "pipe",
      stderr: "pipe",
    });
    const timer = setTimeout(() => child.kill(), 15_000);
    try {
      const [stdout, stderr, code] = await Promise.all([
        new Response(child.stdout).text(),
        new Response(child.stderr).text(),
        child.exited,
      ]);
      assert.equal(code, 0, stderr);
      return stdout;
    } finally {
      clearTimeout(timer);
    }
  }
  assert.match(await output("norn", []), /Usage:/);
  assert.match(await output("norn", ["--version"]), /norn/);
  assert.match(await output("norn-tui", ["--help"]), /Norn OpenTUI workspace/);
  assert.match(await output("norn-ui", ["--version"]), /norn-ui/);
  const client = join(prefix, "bin", "norn-ui" + suffix);
  let receive!: (message: unknown) => void;
  const message = new Promise<unknown>((resolve) => {
    receive = resolve;
  });
  const worker = Bun.spawn([client, "--syntax-worker"], {
    env,
    stdin: "ignore",
    stdout: "ignore",
    stderr: "pipe",
    ipc: (data) => receive(data),
  });
  const timer = setTimeout(() => worker.kill(), 10_000);
  try {
    worker.send({ path: "example.ts", diff: "@@ -1 +1 @@\n-const n = 1;\n+const n = 2;\n" });
    const result = await Promise.race([
      message,
      worker.exited.then(() => {
        throw new Error("Syntax worker exited before replying");
      }),
    ]);
    assert.equal((result as { status: string }).status, "ready");
    assert(Object.keys((result as { rows: object }).rows).length > 0);
  } finally {
    clearTimeout(timer);
    worker.kill();
    await worker.exited;
  }
  // Production transport starts the installed backend, performs hello and orderly shutdown.
  const { ProcessBackendTransport } = await import("../src/transport/processTransport");
  const transport = new ProcessBackendTransport(join(prefix, "bin", "norn-backend" + suffix), env);
  try {
    const ready = await transport.start();
    assert(ready);
  } finally {
    await transport.close();
  }
  // Exercise the compiled production client in an owned controlling terminal.
  const windows = process.platform === "win32";
  const driver = Bun.which(windows ? "pwsh" : "python3");
  assert(driver, "Native PTY/ConPTY driver is required");
  const args = windows
    ? [
        driver,
        "-NoProfile",
        "-File",
        resolve("ui/scripts/support/check-editor-conpty.ps1"),
        "-ProjectRoot",
        process.cwd(),
        "-Case",
        "success",
      ]
    : [
        driver,
        resolve("ui/scripts/support/check-editor-pty.py"),
        "--root",
        process.cwd(),
        "--case",
        "success",
      ];
  const terminal = Bun.spawn(args, {
    env: { ...process.env, NORN_TEST_INSTALLED_BIN: join(prefix, "bin") },
    stdout: "inherit",
    stderr: "inherit",
  });
  const terminalTimer = setTimeout(() => terminal.kill(), 120_000);
  try {
    assert.equal(await terminal.exited, 0, "Installed terminal lifecycle failed");
  } finally {
    clearTimeout(terminalTimer);
  }

  const installedHtml = join(prefix, "share", "norn", "browser-diff", "browser-diff.html");
  await writeFile(
    installedHtml,
    `${await readFile(installedHtml, "utf8")}<!-- owned installed viewer -->`,
  );
  // browser.open reads only an owned local Git fixture; no provider is contacted.
  const git = Bun.which("git");
  assert(git);
  const repo = join(root, "owned browser checkout");
  await mkdir(repo);
  const gitEnv = {
    ...env,
    PATH: dirname(git),
    GIT_CONFIG_NOSYSTEM: "1",
    GIT_CONFIG_GLOBAL: windows ? "NUL" : "/dev/null",
  };
  for (const args of [
    ["init", "-q"],
    ["config", "user.name", "Owned"],
    ["config", "user.email", "owned@example.invalid"],
  ]) {
    const result: ReturnType<typeof Bun.spawnSync> = Bun.spawnSync([git, "-C", repo, ...args], {
      env: gitEnv,
    });
    assert.equal(result.exitCode, 0, result.stderr?.toString() ?? "Owned Git setup failed");
  }
  await writeFile(join(repo, "owned.txt"), "before\n");
  for (const args of [
    ["add", "owned.txt"],
    ["-c", "commit.gpgsign=false", "commit", "-qm", "Owned base"],
  ]) {
    assert.equal(Bun.spawnSync([git, "-C", repo, ...args], { env: gitEnv }).exitCode, 0);
  }
  await writeFile(join(repo, "owned.txt"), "after\n");
  await writeFile(
    join(config, "settings.json"),
    JSON.stringify({
      defaultDiffView: "unified",
      theme: "dark",
      repos: [{ provider: "github", workspace: "owned", repo: "browser", localPath: repo }],
    }),
  );
  const browser = new ProcessBackendTransport(join(prefix, "bin", `norn-backend${suffix}`), {
    ...gitEnv,
    NORN_BACKEND_REPO_PATH: repo,
    NORN_BROWSER_OPEN: "0",
  });
  try {
    await browser.start();
    const result = await browser.request("browser.open", {
      target: {
        kind: "provider",
        provider: "github",
        workspace: "owned",
        repo: "browser",
        prId: null,
        runId: null,
      },
    });
    const url = String(result.url);
    assert(url.startsWith("http://127.0.0.1:"));
    const page = await fetch(url, { signal: AbortSignal.timeout(10_000) });
    assert.equal(page.status, 200);
    const html = await page.text();
    assert.match(html, /browser-diff/);
    assert.match(html, /owned installed viewer/);
    const asset = /src="([^"]+\.js)"/.exec(html)?.[1];
    assert(asset, "Built viewer script must be referenced");
    const script = await fetch(new URL(asset, url), { signal: AbortSignal.timeout(10_000) });
    assert.equal(script.status, 200, "Installed browser script must be served");
    assert((await script.text()).length > 1000);
  } finally {
    await browser.close();
  }
  console.log(
    "Installed CLI, OpenTUI, syntax worker, IPC and browser assets pass; CLI/UI need no external Bun/Node.",
  );
} finally {
  await rm(root, { recursive: true, force: true });
}
