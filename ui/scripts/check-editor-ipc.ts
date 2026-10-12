// Production Rust editor RPCs, owned repositories/config and compiled launchers.
import assert from "node:assert/strict";
import {
  copyFile,
  mkdir,
  mkdtemp,
  readFile,
  realpath,
  rm,
  symlink,
  writeFile,
} from "node:fs/promises";
import { tmpdir } from "node:os";
import { delimiter, dirname, join, resolve } from "node:path";
import type {
  RepositoryEditor,
  RepositoryEditorPrepareParams,
} from "../../src/protocol/generated/protocol";
import { BackendClient } from "../src/transport/backendClient";
import { ProcessBackendTransport } from "../src/transport/processTransport";

const project = resolve(process.cwd());
const suffix = process.platform === "win32" ? ".exe" : "";
const backend = resolve(project, `src-tauri/target/debug/norn-backend${suffix}`);
const git = Bun.which("git"),
  rustc = Bun.which("rustc");
assert(git && rustc && (await Bun.file(backend).exists()));
const root = await realpath(await mkdtemp(join(tmpdir(), "norn-owned-editor-ipc-")));
const config = join(root, "config"),
  editors = join(root, "editors"),
  repo = join(root, "checkout with spaces");
const clients: BackendClient[] = [];
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
try {
  for (const directory of [config, editors, repo, join(root, "zsh")])
    await mkdir(directory, { recursive: true });
  const zed = join(editors, `zed${suffix}`),
    nvim = join(editors, `nvim${suffix}`);
  const compile = Bun.spawnSync(
    [rustc, "--edition=2021", join(project, "ui/scripts/support/owned-editor.rs"), "-o", zed],
    { stdout: "pipe", stderr: "pipe" },
  );
  assert.equal(compile.exitCode, 0, compile.stderr.toString());
  await copyFile(zed, nvim);
  const env: Record<string, string> = {
    PATH: [editors, dirname(git), "/usr/bin", "/bin"].join(delimiter),
    NORN_CONFIG_DIR: config,
    NORN_DATA_DIR: join(root, "workspace"),
    NORN_REVIEW_DATA_DIR: join(root, "reviews"),
    NORN_OWNED_EDITOR_DIR: editors,
    ZDOTDIR: join(root, "zsh"),
    GIT_CONFIG_NOSYSTEM: "1",
    GIT_CONFIG_GLOBAL: process.platform === "win32" ? "NUL" : "/dev/null",
    NORN_BROWSER_OPEN: "0",
    NORN_BACKEND_REPO_PATH: repo,
  };
  if (process.platform === "win32")
    for (const key of ["SystemRoot", "WINDIR", "TEMP", "TMP"])
      if (process.env[key]) env[key] = process.env[key]!;
  const runGit = (...args: string[]) => {
    const result = Bun.spawnSync([git, "-C", repo, ...args], {
      env,
      stdout: "pipe",
      stderr: "pipe",
    });
    assert.equal(result.exitCode, 0, result.stderr?.toString() ?? "Owned Git failed.");
  };
  runGit("init", "-q");
  runGit("config", "user.name", "Owned Editor");
  runGit("config", "user.email", "owned@example.invalid");
  runGit("remote", "add", "origin", "https://github.com/owned/editor.git");
  const path =
      process.platform === "win32"
        ? "--literal ü & $(owned);.txt"
        : "--literal ü & $(owned);\n\u001bfile.txt",
    file = join(repo, path);
  await writeFile(file, "owned source\n");
  runGit("add", "--", path);
  runGit("-c", "commit.gpgsign=false", "commit", "-qm", "owned base");
  await writeFile(
    join(config, "settings.json"),
    JSON.stringify({
      repos: [{ provider: "github", workspace: "owned", repo: "editor", localPath: repo }],
      defaultDiffView: "unified",
      theme: "dark",
    }),
  );
  const open = async (extra: Record<string, string> = {}) => {
    const client = new BackendClient(new ProcessBackendTransport(backend, { ...env, ...extra }));
    clients.push(client);
    assert((await client.start()).capabilities.includes("repository-editor"));
    return client;
  };
  const client = await open();
  const info = await client.repositoryExploreInfo({
    repository: { provider: "github", workspace: "owned", repo: "editor" },
  });
  const params: RepositoryEditorPrepareParams = {
    context: info.context,
    path,
    line: 7,
    mode: "graphical",
  };
  const poll = async (client: BackendClient, editor: RepositoryEditor) => {
    const deadline = Date.now() + 25000;
    while (
      editor.state === "starting" ||
      editor.state === "running" ||
      editor.state === "launched"
    ) {
      assert(Date.now() < deadline, "Owned launcher did not settle");
      await Bun.sleep(20);
      editor = (
        await client.repositoryEditorStatus(
          { context: editor.context, launchId: editor.launchId },
          editor,
        )
      ).editor;
    }
    return editor;
  };
  const prepared = await client.repositoryEditorPrepare(params);
  assert(prepared.token);
  assert.equal(prepared.integration, "zed");
  await assert.rejects(
    client.repositoryEditorOpen(
      { context: { ...params.context, rootId: "d".repeat(64) }, token: prepared.token },
      params,
    ),
  );
  const opened = await client.repositoryEditorOpen(
    { context: params.context, token: prepared.token },
    params,
  );
  assert.equal((await poll(client, opened.editor)).state, "exited");
  const normalize = (value: string) => value.replace(/^\\\\\?\\/u, "");
  assert.equal(
    normalize(await readFile(join(editors, "argv.txt"), "utf8")),
    `${normalize(file)}:7:1`,
  );
  await assert.rejects(
    client.repositoryEditorOpen({ context: params.context, token: prepared.token }, params),
  );
  const stale = await client.repositoryEditorPrepare(params);
  assert(stale.token);
  await writeFile(file, "changed after preview\n");
  const staleOpen = await client.repositoryEditorOpen(
    { context: params.context, token: stale.token },
    params,
  );
  assert.equal((await poll(client, staleOpen.editor)).state, "failed");
  const missing = { ...params, path: "missing.txt" };
  await assert.rejects(client.repositoryEditorPrepare(missing));
  await assert.rejects(client.repositoryEditorPrepare({ ...params, path: "../outside.txt" }));
  const outside = join(root, "outside checkout");
  await mkdir(outside);
  await writeFile(join(outside, "outside.txt"), "OWNED_OUTSIDE_EXCLUDED\n");
  await symlink(outside, join(repo, "escape"), process.platform === "win32" ? "junction" : "dir");
  await assert.rejects(client.repositoryEditorPrepare({ ...params, path: "escape/outside.txt" }));
  const otherRepo = join(root, "bitbucket clone");
  await mkdir(otherRepo);
  for (const args of [
    ["init", "-q"],
    ["remote", "add", "origin", "https://bitbucket.org/owned/editor.git"],
  ]) {
    const result: Bun.SyncSubprocess = Bun.spawnSync([git, "-C", otherRepo, ...args], {
      env,
      stdout: "pipe",
      stderr: "pipe",
    });
    assert.equal(result.exitCode, 0, result.stderr?.toString() ?? "Owned Git failed.");
  }
  await writeFile(join(otherRepo, path), "Owned Bitbucket source\n");
  const settings = {
    repos: [
      { provider: "github", workspace: "owned", repo: "editor", localPath: repo },
      { provider: "bitbucket", workspace: "owned", repo: "editor", localPath: otherRepo },
    ],
    defaultDiffView: "unified",
    theme: "dark",
  };
  await writeFile(join(config, "settings.json"), JSON.stringify(settings));
  const otherInfo = await client.repositoryExploreInfo({
    repository: { provider: "bitbucket", workspace: "owned", repo: "editor" },
  });
  assert.notEqual(info.context.rootId, otherInfo.context.rootId);
  const otherParams = { ...params, context: otherInfo.context };
  const otherPrepared = await client.repositoryEditorPrepare(otherParams);
  assert(otherPrepared.token);
  const otherOpened = await client.repositoryEditorOpen(
    { context: otherInfo.context, token: otherPrepared.token },
    otherParams,
  );
  assert.equal((await poll(client, otherOpened.editor)).state, "exited");
  assert.equal(
    normalize(await readFile(join(editors, "argv.txt"), "utf8")),
    `${normalize(join(otherRepo, path))}:7:1`,
  );
  const originBound = await client.repositoryEditorPrepare(params);
  assert(originBound.token);
  settings.repos[0]!.localPath = otherRepo;
  await writeFile(join(config, "settings.json"), JSON.stringify(settings));
  const changedRoot = await client.repositoryEditorOpen(
    { context: params.context, token: originBound.token },
    params,
  );
  assert.equal((await poll(client, changedRoot.editor)).state, "failed");
  settings.repos[0]!.localPath = repo;
  await writeFile(join(config, "settings.json"), JSON.stringify(settings));
  const terminal = { ...params, mode: "terminal" } as const;
  const expiring = await client.repositoryEditorPrepare(params);
  assert(expiring.token);
  await Bun.sleep(expiring.expiresInMs + 1000);
  await assert.rejects(
    client.repositoryEditorOpen({ context: params.context, token: expiring.token }, params),
  );
  const unavailable = await client.repositoryEditorPrepare(terminal);
  assert.equal(unavailable.token, null);
  assert(unavailable.message);
  const failed = await open({ NORN_OWNED_EDITOR_FAILURE: "7" });
  const failure = await failed.repositoryEditorPrepare(params);
  assert(failure.token);
  const failedOpen = await failed.repositoryEditorOpen(
    { context: params.context, token: failure.token },
    params,
  );
  const failureStatus = await poll(failed, failedOpen.editor);
  assert.equal(failureStatus.state, "failed");
  assert.equal(failureStatus.exitCode, 7);
  console.log(
    "PASS: real GUI launcher argv/exit, scoped expiring one-use authorization, stale file/root, both provider clones, symlink escape, missing/traversal and unavailable terminal integration",
  );
} finally {
  for (const client of clients) await client.close();
  await removeOwnedRoot();
}
