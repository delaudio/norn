// Real OpenTUI input/rendering + production stdio transport + no-default Rust.
// Every repository, settings file, stored run and agent belongs to this process.
import assert from "node:assert/strict";
import { mkdir, mkdtemp, readFile, rename, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { delimiter, dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { createTestRenderer, KeyCodes } from "@opentui/core/testing";
import { ShellStore } from "../src/app/store";
import { mountApp } from "../src/app/ui";
import { BackendClient } from "../src/transport/backendClient";
import { ProcessBackendTransport } from "../src/transport/processTransport";
import { newPng, oldPng } from "../test/fixtures/raster-bytes";

const original = process.cwd();
const executableSuffix = process.platform === "win32" ? ".exe" : "";
const binary = resolve(
  original,
  process.env.NORN_WORKFLOW_BACKEND_BIN ?? `src-tauri/target/debug/norn-backend${executableSuffix}`,
);
const cli = resolve(
  original,
  process.env.NORN_WORKFLOW_CLI_BIN ?? `src-tauri/target/debug/norn${executableSuffix}`,
);
for (const file of [binary, cli])
  assert(await Bun.file(file).exists(), `Required Rust binary is missing: ${file}`);
const gitBinary = Bun.which("git");
assert(gitBinary, "Git is required");
const rustc = Bun.which("rustc");
assert(rustc, "Rust compiler is required for the owned fake agent");
const root = await mkdtemp(join(tmpdir(), "norn-owned-native-workflow-"));
const removeOwnedRoot = async () => {
  for (let attempt = 0; ; attempt++) {
    try {
      await rm(root, { recursive: true, force: true });
      break;
    } catch (error) {
      if (attempt >= 20) throw error;
      await Bun.sleep(100);
    }
  }
};
const repo = join(root, "checkout with spaces");
const config = join(root, "config");
const agent = join(root, "agent");
for (const path of [repo, config, agent]) await mkdir(path, { recursive: true });
const agentBinary = join(agent, process.platform === "win32" ? "claude.exe" : "claude");
const compile = Bun.spawnSync(
  [
    rustc,
    "--edition=2021",
    fileURLToPath(new URL("./support/owned-agent.rs", import.meta.url)),
    "-o",
    agentBinary,
  ],
  { stdout: "pipe", stderr: "pipe" },
);
assert.equal(compile.exitCode, 0, compile.stderr.toString());
const env: Record<string, string> = {
  PATH: [agent, dirname(gitBinary), "/usr/bin", "/bin"].join(delimiter),
  LANG: "C.UTF-8",
  GIT_CONFIG_NOSYSTEM: "1",
  GIT_CONFIG_GLOBAL: process.platform === "win32" ? "NUL" : "/dev/null",
  NORN_DATA_DIR: join(root, "workspace"),
  NORN_CONFIG_DIR: config,
  NORN_REVIEW_DATA_DIR: join(root, "reviews"),
  NORN_REVIEW_ENGINE: "real",
  NORN_REVIEW_AGENT_BIN: agentBinary,
  NORN_OWNED_AGENT_DIR: agent,
  NORN_BROWSER_OPEN: "0",
  NORN_BACKEND_REPO_PATH: repo,
};
if (process.platform === "win32")
  for (const key of ["SystemRoot", "WINDIR", "TEMP", "TMP"]) {
    if (process.env[key]) env[key] = process.env[key]!;
  }
const git = (...args: string[]) => {
  const result = Bun.spawnSync([gitBinary, "-C", repo, ...args], {
    env,
    stdout: "pipe",
    stderr: "pipe",
  });
  assert.equal(result.exitCode, 0, result.stderr.toString());
};
git("init", "-q");
git("config", "user.name", "Owned Workflow");
git("config", "user.email", "owned@example.invalid");
git("remote", "add", "origin", "https://github.com/owned/workflow.git");
await writeFile(join(repo, "same.txt"), "base\n");
await writeFile(join(repo, "picture.png"), Buffer.from(oldPng, "base64"));
await writeFile(
  join(repo, ".norn.yaml"),
  "version: 0.1\nreview:\n  profile: security\nprofiles:\n  default: {}\n  security: {}\n",
);
git("add", "same.txt", "picture.png", ".norn.yaml");
git("-c", "commit.gpgsign=false", "commit", "-qm", "owned base");
git("branch", "comparison");
await writeFile(join(repo, "same.txt"), "staged\n");
await writeFile(join(repo, "picture.png"), Buffer.from(newPng, "base64"));
git("add", "same.txt", "picture.png");
await writeFile(join(repo, "same.txt"), "unstaged\n");
await writeFile(join(repo, "picture.png"), Buffer.from(oldPng, "base64"));
await writeFile(join(repo, "private-untracked.txt"), "EXCLUDED_OWNED_UNTRACKED_MARKER\n");
await writeFile(
  join(config, "settings.json"),
  JSON.stringify({
    aiProvider: "claude",
    repos: [{ provider: "github", workspace: "owned", repo: "workflow", localPath: repo }],
    defaultDiffView: "unified",
    theme: "dark",
  }),
);
const clients = new Set<BackendClient>();
const open = async () => {
  const transport = new ProcessBackendTransport(binary, env);
  const exited = new Promise<void>((resolveExit) => transport.onExit(() => resolveExit()));
  const methods: string[] = [];
  const request = transport.request.bind(transport);
  transport.request = (method, params) => {
    methods.push(method);
    return request(method, params);
  };
  const client = new BackendClient(transport);
  clients.add(client);
  const store = new ShellStore();
  await store.connect(client);
  assert.equal(store.getSnapshot().status, "ready");
  const renderer = await createTestRenderer({ width: 120, height: 30 });
  const app = mountApp(renderer.renderer, store, client);
  const tick = async () => {
    await Bun.sleep(10);
    await renderer.renderOnce();
  };
  const until = async (condition: () => boolean, label: string) => {
    const deadline = Date.now() + 15_000;
    while (!condition() && Date.now() < deadline) await tick();
    assert(
      condition(),
      `${label}: ${JSON.stringify({
        error: store.getSnapshot().error,
        notice: store.getSnapshot().notice,
        operation: store.getSnapshot().operation?.state,
        focus: store.getSnapshot().focus,
        layers: store.getSnapshot().findingLayers,
        profile: store.getSnapshot().profileOpen,
        lastMethods: methods.slice(-6),
      })}`,
    );
  };
  let closed = false;
  const stop = async () => {
    if (closed) return;
    closed = true;
    app.unmount();
    store.cancelSyntax();
    try {
      await transport.request("shutdown", {});
    } finally {
      let timer: ReturnType<typeof setTimeout> | undefined;
      try {
        await Promise.race([
          exited,
          new Promise<never>((_, reject) => {
            timer = setTimeout(
              () => reject(new Error("Owned backend did not exit after shutdown.")),
              5000,
            );
          }),
        ]);
      } finally {
        if (timer) clearTimeout(timer);
        client.close();
        clients.delete(client);
        renderer.renderer.destroy();
      }
    }
  };
  return { client, store, renderer, tick, until, stop };
};
try {
  process.chdir(repo);
  const cliReview = Bun.spawnSync(
    [
      cli,
      "review",
      "--scope",
      "working-tree",
      "--ai-provider",
      "claude",
      "--allow-provider-diff",
      "--json",
    ],
    { cwd: repo, env, stdout: "pipe", stderr: "pipe" },
  );
  assert.equal(cliReview.exitCode, 0, cliReview.stderr.toString());
  assert.equal(JSON.parse(cliReview.stdout.toString()).status, "succeeded");
  assert((await readFile(join(agent, "payload"), "utf8")).includes("same.txt"));
  const first = await open();
  // Production readonly explorer IPC runs in all three native OS lanes.
  const explorer = await first.client.repositoryExploreInfo({
    repository: { provider: "github", workspace: "owned", repo: "workflow" },
  });
  const inventory = await first.client.repositoryExploreFiles({
    context: explorer.context,
    offset: 0,
    limit: 32,
    query: "",
    changedOnly: false,
    version: null,
  });
  assert(
    inventory.files.some(
      (file) => file.path === "private-untracked.txt" && file.status === "untracked",
    ),
  );
  const live = await first.client.repositoryExploreContent({
    context: explorer.context,
    path: "same.txt",
    offset: 0,
    maxBytes: 32768,
    contentSha256: null,
  });
  assert.equal(live.state, "text");
  assert.equal(live.text, "unstaged\n");
  assert.equal(live.contentSha256, new Bun.CryptoHasher("sha256").update(live.text).digest("hex"));
  const found = await first.client.repositoryExploreFind({
    context: explorer.context,
    path: "same.txt",
    source: "content",
    query: "staged",
    offset: 0,
    limit: 32,
    contentSha256: live.contentSha256!,
  });
  assert.equal(found.total, 1);
  assert.equal(found.matches[0]?.startUtf16, 2);
  const blame = await first.client.repositoryExploreBlame({
    context: explorer.context,
    path: "same.txt",
    offset: 0,
    limit: 32,
    contentSha256: live.contentSha256!,
  });
  assert.equal(blame.totalLines, 1);
  assert.equal(blame.lines[0]?.line, 1);
  const untracked = await first.client.repositoryExploreDiff({
    context: explorer.context,
    path: "private-untracked.txt",
    offset: 0,
    maxBytes: 32768,
    contentSha256: null,
  });
  assert(untracked.text.includes("+EXCLUDED_OWNED_UNTRACKED_MARKER"));
  const raster = await first.client.repositoryExploreContent({
    context: explorer.context,
    path: "picture.png",
    offset: 0,
    maxBytes: 32768,
    contentSha256: null,
  });
  assert.equal(raster.state, "binary");
  assert.equal(raster.text, "");
  try {
    await first.until(() => first.store.getSnapshot().localInfo !== null, "Current capture");
    assert.equal(first.store.getSnapshot().localInfo!.snapshot.repository.repo, "workflow");
    first.renderer.mockInput.pressKey(KeyCodes.F4);
    await first.until(() => !!first.store.getSnapshot().localForm?.options, "Source options");
    first.renderer.mockInput.pressKey(KeyCodes.ARROW_DOWN);
    await first.until(
      () =>
        first.store.getSnapshot().localForm?.sources[
          first.store.getSnapshot().localForm!.sourceIndex
        ]?.source.kind === "configured" && !first.store.getSnapshot().localForm?.loading,
      "Explicit configured source",
    );
    first.renderer.mockInput.pressTab();
    await first.tick();
    first.renderer.mockInput.pressKey(KeyCodes.ARROW_DOWN);
    await first.tick();
    first.renderer.mockInput.pressEnter();
    await first.until(
      () =>
        first.store.getSnapshot().localForm === null &&
        first.store.getSnapshot().localInfo?.comparison.mode === "branch",
      "Explicit committed branch base",
    );
    assert.deepEqual(first.store.getSnapshot().localInfo!.comparison.layers, ["committed"]);
    first.renderer.mockInput.pressKey(KeyCodes.F4);
    await first.until(() => !!first.store.getSnapshot().localForm?.options, "Working-tree options");
    first.renderer.mockInput.pressTab();
    await first.tick();
    first.renderer.mockInput.pressTab();
    await first.tick();
    first.renderer.mockInput.pressKey(" ");
    await first.tick();
    assert.deepEqual(first.store.getSnapshot().localForm!.layers, ["staged", "unstaged"]);
    first.renderer.mockInput.pressEnter();
    await first.until(
      () =>
        first.store.getSnapshot().localForm === null &&
        first.store.getSnapshot().files.length === 4,
      "Layer selection",
    );
    const selected = first.store.getSnapshot().localInfo!.snapshot;
    await first.until(
      () =>
        first.store.getSnapshot().preview?.before?.available === true &&
        first.store.getSnapshot().preview?.after?.available === true,
      "Captured raster before/after",
    );
    assert.equal(first.store.getSnapshot().preview!.before!.dataBase64, oldPng);
    assert.equal(first.store.getSnapshot().preview!.after!.dataBase64, newPng);
    first.renderer.mockInput.pressEnter();
    await first.until(
      () =>
        first.store.getSnapshot().operation?.state === "succeeded" &&
        first.store.getSnapshot().history.length === 1 &&
        first.store.getSnapshot().localRunId !== null &&
        first.store.getSnapshot().findings.length === 1 &&
        first.store.getSnapshot().threads.length === 1,
      "Native review and saved run",
    );
    const run = first.store.getSnapshot().localRunId;
    assert(run);
    assert.equal(first.store.getSnapshot().findings.length, 1);
    assert((await readFile(join(agent, "payload"), "utf8")).includes("unstaged"));
    assert(
      !(await readFile(join(agent, "payload"), "utf8")).includes("EXCLUDED_OWNED_UNTRACKED_MARKER"),
    );
    assert(!first.renderer.captureCharFrame().includes("private-owned-workflow-session"));
    for (
      let attempt = 0;
      attempt < 4 && first.store.getSnapshot().focus !== "findings";
      attempt++
    ) {
      first.renderer.mockInput.pressTab();
      await first.tick();
    }
    assert.equal(first.store.getSnapshot().focus, "findings");
    assert.deepEqual(first.store.getSnapshot().findingLayers, ["staged", "unstaged"]);
    first.renderer.mockInput.pressEnter();
    await first.tick();
    assert.equal(
      first.store.getSnapshot().files[first.store.getSnapshot().selectedFile]?.localLayer,
      "staged",
    );
    await writeFile(join(agent, "mode"), "slow");
    await rm(join(agent, "started"));
    await rm(join(agent, "completed"));
    first.renderer.mockInput.pressEnter();
    await first.until(
      () =>
        first.store.getSnapshot().operation?.state === "running" &&
        Bun.file(join(agent, "started")).size > 0,
      "Owned slow child start",
    );
    first.renderer.mockInput.pressKey("x");
    await first.until(
      () => first.store.getSnapshot().operation?.state === "cancelled",
      "Native cancellation",
    );
    assert(!(await Bun.file(join(agent, "completed")).exists()));
    await first.stop();
    process.chdir(root);
    await rename(repo, join(root, "removed checkout"));
    await mkdir(repo);
    process.chdir(repo);
    const restarted = await open();
    try {
      restarted.renderer.mockInput.pressEscape();
      await Bun.sleep(30);
      await restarted.until(
        () => restarted.store.getSnapshot().localForm === null,
        "Dismiss unavailable live source",
      );
      restarted.renderer.mockInput.pressKey(KeyCodes.F7);
      await restarted.until(
        () =>
          !!restarted.store.getSnapshot().savedLocal &&
          !restarted.store.getSnapshot().savedLocal!.loading,
        "Saved catalog after restart",
      );
      const index = restarted.store
        .getSnapshot()
        .savedLocal!.items.findIndex((item) => item.snapshot.snapshotId === selected.snapshotId);
      assert(index >= 0);
      for (let i = 0; i < index; i++) restarted.renderer.mockInput.pressKey(KeyCodes.ARROW_DOWN);
      restarted.renderer.mockInput.pressEnter();
      await restarted.until(
        () => restarted.store.getSnapshot().localRunId === run,
        "Actual saved run without checkout",
      );
      assert.equal(restarted.store.getSnapshot().findings[0]?.title, "Owned workflow finding");
      assert.equal(
        restarted.store.getSnapshot().localInfo!.snapshot.snapshotId,
        selected.snapshotId,
      );
    } finally {
      await restarted.stop();
    }
  } finally {
    await first.stop();
  }
  console.log(
    "Owned OpenTUI/Rust workflow passed: source/layers, diff/finding, fake native review, cancel, restart and missing checkout.",
  );
} catch (error) {
  // Preserve the scenario failure even if Windows cleanup also reports a lock.
  console.error(error);
  throw error;
} finally {
  process.chdir(original);
  for (const client of clients) client.close();
  await removeOwnedRoot();
}
