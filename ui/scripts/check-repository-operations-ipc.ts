// Real Rust Git operations, owned clones/settings, no provider network or credentials.
import assert from "node:assert/strict";
import { chmod, mkdir, mkdtemp, readFile, realpath, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { delimiter, dirname, join, resolve } from "node:path";
import type {
  RepositoryOperation,
  RepositoryOperationAction,
  RepositoryOperationPrepared,
} from "../../src/protocol/generated/protocol";
import { BackendClient } from "../src/transport/backendClient";
import { ProcessBackendTransport } from "../src/transport/processTransport";
import { ownedGitProxy } from "./ownedGitProxy";

const project = resolve(process.cwd()),
  git = Bun.which("git");
assert(git);
const binary = join(
  project,
  "src-tauri/target/debug",
  process.platform === "win32" ? "norn-backend.exe" : "norn-backend",
);
assert(await Bun.file(binary).exists());
const root = await realpath(await mkdtemp(join(tmpdir(), "norn-owned-repository-operations-")));
const config = join(root, "config"),
  repo = join(root, "checkout with spaces"),
  zsh = join(root, "zsh");
const clients: BackendClient[] = [];
let proxy: ReturnType<typeof ownedGitProxy> | undefined;
const environment: Record<string, string> = {
  PATH: [dirname(git), "/usr/bin", "/bin"].join(delimiter),
  NORN_CONFIG_DIR: config,
  NORN_DATA_DIR: join(root, "workspace"),
  NORN_REVIEW_DATA_DIR: join(root, "reviews"),
  NORN_BACKEND_REPO_PATH: repo,
  ZDOTDIR: zsh,
  GIT_CONFIG_GLOBAL: process.platform === "win32" ? "NUL" : "/dev/null",
  GIT_CONFIG_NOSYSTEM: "1",
  NO_PROXY: "",
  no_proxy: "",
  NORN_BROWSER_OPEN: "0",
};
if (process.platform === "win32")
  for (const key of ["SystemRoot", "WINDIR", "TEMP", "TMP"])
    if (process.env[key]) environment[key] = process.env[key]!;
const runGit = (...args: string[]) => {
  const result = Bun.spawnSync([git, "-C", repo, ...args], {
    env: environment,
    stdout: "pipe",
    stderr: "pipe",
  });
  assert.equal(result.exitCode, 0, result.stderr.toString());
  return result.stdout.toString().trim();
};
async function removeOwnedFixture() {
  for (let attempt = 0; ; attempt++) {
    try {
      await rm(root, { recursive: true, force: true });
      break;
    } catch (error) {
      if (attempt >= 20) throw error;
      await Bun.sleep(100);
    }
  }
}
const identity = { provider: "github" as const, workspace: "owned", repo: "operations" };
try {
  for (const path of [config, repo, zsh]) await mkdir(path, { recursive: true });
  runGit("init", "-b", "main");
  runGit("config", "user.name", "Owned");
  runGit("config", "user.email", "owned@example.invalid");
  runGit("config", "core.attributesFile", process.platform === "win32" ? "NUL" : "/dev/null");
  runGit("remote", "add", "origin", "https://github.com/owned/operations.git");
  await writeFile(join(repo, "owned.txt"), "base\n");
  runGit("add", "--all");
  runGit("-c", "commit.gpgsign=false", "commit", "-m", "base");
  runGit("branch", "feature-ü;$(owned)");
  await writeFile(
    join(config, "settings.json"),
    JSON.stringify({
      repos: [{ ...identity, localPath: repo }],
      defaultDiffView: "unified",
      theme: "dark",
    }),
  );
  const open = async () => {
    const client = new BackendClient(new ProcessBackendTransport(binary, environment));
    clients.push(client);
    assert((await client.start()).capabilities.includes("repository-operations"));
    return client;
  };
  const client = await open();
  const list = await client.repositoryOperationsList({
    repository: identity,
    offset: 0,
    limit: 100,
    version: null,
  });
  const firstPage = await client.repositoryOperationsList({
    repository: identity,
    offset: 0,
    limit: 1,
    version: null,
  });
  assert.equal(firstPage.branches.length, 1);
  assert.equal(firstPage.nextOffset, 1);
  assert(firstPage.checkout.version);
  const secondPage = await client.repositoryOperationsList({
    repository: identity,
    offset: 1,
    limit: 1,
    version: firstPage.checkout.version,
  });
  assert.equal(secondPage.branches.length, 1);
  assert.equal(secondPage.nextOffset, null);
  assert.notEqual(firstPage.branches[0]!.reference, secondPage.branches[0]!.reference);
  runGit("branch", "owned-page-change");
  await assert.rejects(() =>
    client.repositoryOperationsList({
      repository: identity,
      offset: 1,
      limit: 1,
      version: firstPage.checkout.version!,
    }),
  );
  assert.equal(list.checkout.state, "ready");
  assert.equal(list.checkout.branch, "main");
  const context = list.checkout.context;
  assert(context);
  const poll = async (operation: RepositoryOperation) => {
    const deadline = Date.now() + 110000;
    while (["starting", "checking", "running", "inspecting"].includes(operation.state)) {
      assert(Date.now() < deadline, "Owned Git operation did not settle");
      await Bun.sleep(20);
      operation = (
        await client.repositoryOperationsStatus(
          { context, operationId: operation.operationId },
          operation,
        )
      ).operation;
    }
    return operation;
  };
  const preview = async (action: RepositoryOperationAction) =>
    client.repositoryOperationsPreview({ context, action });
  const apply = async (prepared: RepositoryOperationPrepared) =>
    poll(
      (
        await client.repositoryOperationsApply(
          { context, action: prepared.request.action, token: prepared.token, confirm: true },
          prepared,
        )
      ).operation,
    );
  const prepared = await preview({
    kind: "checkout",
    branchKind: "local",
    reference: "refs/heads/feature-ü;$(owned)",
  });
  assert(prepared.summary.includes("feature-ü;$(owned)"));
  await assert.rejects(() =>
    client.repositoryOperationsApply(
      { context, action: { kind: "stash" }, token: prepared.token, confirm: true },
      prepared,
    ),
  );
  const checked = await apply(prepared);
  assert.equal(checked.state, "completed", JSON.stringify(checked));
  assert.equal(runGit("symbolic-ref", "--short", "HEAD"), "feature-ü;$(owned)");
  assert.equal(checked.after?.branch, "feature-ü;$(owned)");
  await assert.rejects(() => apply(prepared));
  const name =
    process.platform === "win32"
      ? "--untracked ü & $(owned);.txt"
      : "--untracked ü & $(owned);\n\u001b.txt";
  await writeFile(join(repo, name), "keep untracked\n");
  await writeFile(join(repo, "owned.txt"), "keep modified\n");
  await assert.rejects(() =>
    preview({ kind: "checkout", branchKind: "local", reference: "refs/heads/main" }),
  );
  const stashed = await apply(await preview({ kind: "stash" }));
  assert.equal(stashed.state, "completed", JSON.stringify(stashed));
  assert.equal(runGit("show", `refs/stash^3:${name}`), "keep untracked");
  assert.equal(runGit("show", "refs/stash:owned.txt"), "keep modified");
  assert.equal(
    stashed.after?.dirty,
    false,
    JSON.stringify({ operation: stashed, actualStatus: runGit("status", "--porcelain=v1", "-z") }),
  );
  assert.equal(stashed.stateChanged, true);
  assert.equal(stashed.after?.stashSha, runGit("rev-parse", "refs/stash"));
  const stale = await preview({
    kind: "checkout",
    branchKind: "local",
    reference: "refs/heads/main",
  });
  await writeFile(join(repo, "fresh-untracked.txt"), "retain\n");
  const rejected = await apply(stale);
  assert.equal(rejected.state, "failed", JSON.stringify(rejected));
  assert.equal(runGit("symbolic-ref", "--short", "HEAD"), "feature-ü;$(owned)");
  assert.equal(await readFile(join(repo, "fresh-untracked.txt"), "utf8"), "retain\n");
  await client.close();
  const restarted = await open();
  const actual = await restarted.repositoryOperationsList({
    repository: identity,
    offset: 0,
    limit: 100,
    version: null,
  });
  assert.equal(actual.checkout.branch, "feature-ü;$(owned)");
  assert.equal(actual.checkout.dirty, true);
  await assert.rejects(() =>
    restarted.repositoryOperationsApply(
      { context, action: stale.request.action, token: stale.token, confirm: true },
      stale,
    ),
  );
  await restarted.close();
  await rm(join(repo, "fresh-untracked.txt"));
  runGit("switch", "main");
  const bare = join(root, "remote.git"),
    seed = join(root, "seed");
  const local = (...args: string[]) => {
    const out = Bun.spawnSync([git, ...args], { env: environment, stdout: "pipe", stderr: "pipe" });
    assert.equal(out.exitCode, 0, out.stderr.toString());
    return out.stdout.toString().trim();
  };
  local("clone", "--bare", repo, bare);
  local("clone", bare, seed);
  local("-C", seed, "config", "user.name", "Owned");
  local("-C", seed, "config", "user.email", "owned@example.invalid");
  local(
    "-C",
    seed,
    "config",
    "core.attributesFile",
    process.platform === "win32" ? "NUL" : "/dev/null",
  );
  proxy = ownedGitProxy(git, root, environment);
  runGit("config", "http.proxy", proxy.url);
  runGit("config", "http.noProxy", "");
  runGit("config", "http.sslVerify", "true");
  runGit("config", "branch.main.remote", "origin");
  runGit("config", "branch.main.merge", "refs/heads/main");
  let update = 0;
  for (const provider of ["github", "bitbucket"] as const) {
    const selected = { ...identity, provider };
    runGit(
      "remote",
      "set-url",
      "origin",
      `http://${provider === "github" ? "github.com" : "bitbucket.org"}/owned/operations.git`,
    );
    await writeFile(
      join(config, "settings.json"),
      JSON.stringify({
        repos: [{ ...selected, localPath: repo }],
        defaultDiffView: "unified",
        theme: "dark",
      }),
    );
    await writeFile(join(seed, "owned.txt"), `remote ${++update}\n`);
    local("-C", seed, "add", "--", "owned.txt");
    local("-C", seed, "-c", "commit.gpgsign=false", "commit", "-m", `remote ${update}`);
    local("-C", seed, "push", bare, "main");
    const expectedHead = local("-C", seed, "rev-parse", "HEAD");
    const remoteClient = await open();
    const page = await remoteClient.repositoryOperationsList({
      repository: selected,
      offset: 0,
      limit: 100,
      version: null,
    });
    assert(page.checkout.context);
    const remoteContext = page.checkout.context;
    const finish = async (operation: RepositoryOperation) => {
      const deadline = Date.now() + 110000;
      while (["starting", "checking", "running", "inspecting"].includes(operation.state)) {
        assert(Date.now() < deadline);
        await Bun.sleep(20);
        operation = (
          await remoteClient.repositoryOperationsStatus(
            { context: remoteContext, operationId: operation.operationId },
            operation,
          )
        ).operation;
      }
      return operation;
    };
    const request = async (action: RepositoryOperationAction) => {
      const approval = await remoteClient.repositoryOperationsPreview({
        context: remoteContext,
        action,
      });
      return finish(
        (
          await remoteClient.repositoryOperationsApply(
            { ...approval.request, token: approval.token, confirm: true },
            approval,
          )
        ).operation,
      );
    };
    const beforeFetch = runGit("rev-parse", "HEAD");
    const fetched = await request({ kind: "fetch" });
    assert.equal(fetched.state, "completed", JSON.stringify(fetched));
    assert.equal(runGit("rev-parse", "HEAD"), beforeFetch);
    assert.equal(runGit("rev-parse", "refs/remotes/origin/main"), expectedHead);
    const pulled = await request({ kind: "pull" });
    assert.equal(pulled.state, "completed", JSON.stringify(pulled));
    assert.equal(runGit("rev-parse", "HEAD"), expectedHead);
    assert.equal(await readFile(join(repo, "owned.txt"), "utf8"), `remote ${update}\n`);
    proxy.fail(true);
    const failed = await request({ kind: "fetch" });
    proxy.fail(false);
    assert.equal(failed.state, "failed", JSON.stringify(failed));
    assert.equal(failed.after?.headSha, expectedHead);
    assert.equal(runGit("rev-parse", "HEAD"), expectedHead);
    // Cancellation must settle before any later write, and cannot replay a job.
    proxy.delay(true);
    const cancelApproval = await remoteClient.repositoryOperationsPreview({
      context: remoteContext,
      action: { kind: "fetch" },
    });
    let cancelOperation = (
      await remoteClient.repositoryOperationsApply(
        { ...cancelApproval.request, token: cancelApproval.token, confirm: true },
        cancelApproval,
      )
    ).operation;
    await assert.rejects(() =>
      remoteClient.repositoryOperationsPreview({
        context: remoteContext,
        action: { kind: "stash" },
      }),
    );
    await remoteClient.repositoryOperationsCancel(
      { context: remoteContext, operationId: cancelOperation.operationId },
      cancelOperation,
    );
    cancelOperation = await finish(cancelOperation);
    proxy.delay(false);
    assert.equal(cancelOperation.state, "canceled", JSON.stringify(cancelOperation));
    assert.equal(runGit("rev-parse", "HEAD"), expectedHead);
    const changedConfig = await remoteClient.repositoryOperationsPreview({
      context: remoteContext,
      action: { kind: "fetch" },
    });
    runGit("config", "core.ignorecase", "false");
    const changed = await finish(
      (
        await remoteClient.repositoryOperationsApply(
          { ...changedConfig.request, token: changedConfig.token, confirm: true },
          changedConfig,
        )
      ).operation,
    );
    assert.equal(changed.state, "failed");
    assert.equal(changed.exitCode, null);
    runGit("config", "--unset", "core.ignorecase");
    if (provider === "bitbucket") {
      runGit("switch", "-c", "local-commits");
      await writeFile(join(repo, "local-only.txt"), "preserve my commits\n");
      runGit("add", "--", "local-only.txt");
      runGit("-c", "commit.gpgsign=false", "commit", "-m", "local only");
      const localHead = runGit("rev-parse", "HEAD");
      runGit("branch", "--set-upstream-to=origin/main", "local-commits");
      await writeFile(join(seed, "remote-only.txt"), "independent remote commit\n");
      local("-C", seed, "add", "--", "remote-only.txt");
      local("-C", seed, "-c", "commit.gpgsign=false", "commit", "-m", "divergence");
      local("-C", seed, "push", bare, "main");
      const divergent = await request({ kind: "pull" });
      assert.equal(divergent.state, "failed");
      assert.equal(runGit("rev-parse", "HEAD"), localHead);
      assert.equal(await readFile(join(repo, "local-only.txt"), "utf8"), "preserve my commits\n");
      assert.equal(
        runGit("rev-parse", "refs/remotes/origin/main"),
        local("-C", seed, "rev-parse", "HEAD"),
      );
      // Selecting origin/local-commits chooses the existing local branch, never
      // resets it to the older remote commit.
      runGit("update-ref", "refs/remotes/origin/local-commits", expectedHead);
      runGit("switch", "main");
      const preserve = await request({
        kind: "checkout",
        branchKind: "remote",
        reference: "refs/remotes/origin/local-commits",
      });
      assert.equal(preserve.state, "completed", JSON.stringify(preserve));
      assert.equal(runGit("rev-parse", "HEAD"), localHead);
      runGit("switch", "main");
      runGit("update-ref", "refs/remotes/other/owned", expectedHead);
      const detached = await request({
        kind: "checkout",
        branchKind: "remote",
        reference: "refs/remotes/other/owned",
      });
      assert.equal(detached.state, "completed", JSON.stringify(detached));
      assert.equal(detached.after?.branch, null);
      await assert.rejects(() =>
        remoteClient.repositoryOperationsPreview({
          context: remoteContext,
          action: { kind: "pull" },
        }),
      );
      const back = await request({
        kind: "checkout",
        branchKind: "local",
        reference: "refs/heads/main",
      });
      assert.equal(back.state, "completed");
    }
    if (provider === "github") {
      runGit("update-ref", "refs/remotes/origin/new-tracking", expectedHead);
      const tracking = await request({
        kind: "checkout",
        branchKind: "remote",
        reference: "refs/remotes/origin/new-tracking",
      });
      assert.equal(tracking.state, "completed", JSON.stringify(tracking));
      assert.equal(tracking.after?.branch, "new-tracking");
      assert.equal(tracking.after?.upstream, "refs/remotes/origin/new-tracking");
      const main = await request({
        kind: "checkout",
        branchKind: "local",
        reference: "refs/heads/main",
      });
      assert.equal(main.state, "completed");
      runGit("switch", "-c", "ignored-target");
      await writeFile(join(repo, "ignored-owned.txt"), "tracked target\n");
      runGit("add", "--", "ignored-owned.txt");
      runGit("-c", "commit.gpgsign=false", "commit", "-m", "ignored target");
      runGit("switch", "main");
      await writeFile(join(repo, ".git/info/exclude"), "ignored-owned.txt\n");
      await writeFile(join(repo, "ignored-owned.txt"), "keep ignored content\n");
      const ignored = await request({
        kind: "checkout",
        branchKind: "local",
        reference: "refs/heads/ignored-target",
      });
      assert.equal(ignored.state, "failed");
      assert.equal(runGit("rev-parse", "HEAD"), expectedHead);
      assert.equal(
        await readFile(join(repo, "ignored-owned.txt"), "utf8"),
        "keep ignored content\n",
      );
      // A real trusted post-checkout hook can change HEAD after Git succeeds.
      // The final observed state must not be reported as approved success.
      await rm(join(repo, "ignored-owned.txt"));
      const hooks = join(root, "owned-hooks");
      await mkdir(hooks);
      const hook = join(hooks, "post-checkout");
      await writeFile(hook, '#!/bin/sh\ngit update-ref HEAD "$1"\n');
      await chmod(hook, 0o700);
      runGit("config", "core.hooksPath", hooks);
      const drift = await request({
        kind: "checkout",
        branchKind: "local",
        reference: "refs/heads/ignored-target",
      });
      assert.equal(drift.exitCode, 0);
      assert.equal(drift.state, "uncertain", JSON.stringify(drift));
      assert(drift.recoveryRequired);
      assert.equal(drift.after?.headSha, expectedHead);
      assert.equal(drift.after?.branch, "ignored-target");
      runGit("config", "--unset", "core.hooksPath");
      const saveHookChanges = await request({ kind: "stash" });
      assert.equal(saveHookChanges.state, "completed");
      const restoreMain = await request({
        kind: "checkout",
        branchKind: "local",
        reference: "refs/heads/main",
      });
      assert.equal(restoreMain.state, "completed");
      const expiring = await remoteClient.repositoryOperationsPreview({
        context: remoteContext,
        action: { kind: "fetch" },
      });
      console.log("Checking actual 30-second backend approval expiry");
      await Bun.sleep(30100);
      await assert.rejects(() =>
        remoteClient.repositoryOperationsApply(
          { ...expiring.request, token: expiring.token, confirm: true },
          expiring,
        ),
      );
      assert.equal(runGit("rev-parse", "HEAD"), expectedHead);
    }
    await remoteClient.close();
  }
  // A pre-existing conflict is preserved. Fetch remains safe and available;
  // checkout, stash and pull must refuse without resolving/discarding anything.
  runGit("switch", "-c", "owned-conflict");
  await writeFile(join(repo, "owned.txt"), "conflict side A\n");
  runGit("add", "--", "owned.txt");
  runGit("-c", "commit.gpgsign=false", "commit", "-m", "side A");
  runGit("switch", "main");
  await writeFile(join(repo, "owned.txt"), "conflict side B\n");
  runGit("add", "--", "owned.txt");
  runGit("-c", "commit.gpgsign=false", "commit", "-m", "side B");
  const merge = Bun.spawnSync([git, "-C", repo, "merge", "--no-edit", "owned-conflict"], {
    env: environment,
    stdout: "pipe",
    stderr: "pipe",
  });
  assert.notEqual(merge.exitCode, 0);
  const conflictBytes = await readFile(join(repo, "owned.txt"));
  const pendingBytes = await readFile(join(repo, ".git/MERGE_HEAD"));
  const conflictClient = await open(),
    conflictRepository = { ...identity, provider: "bitbucket" as const };
  const conflictPage = await conflictClient.repositoryOperationsList({
    repository: conflictRepository,
    offset: 0,
    limit: 100,
    version: null,
  });
  assert(conflictPage.checkout.conflicted);
  assert(conflictPage.checkout.pending);
  assert(conflictPage.checkout.context);
  const conflictContext = conflictPage.checkout.context;
  for (const action of [
    { kind: "stash" },
    { kind: "pull" },
    { kind: "checkout", branchKind: "local", reference: "refs/heads/owned-conflict" },
  ] as RepositoryOperationAction[])
    await assert.rejects(() =>
      conflictClient.repositoryOperationsPreview({ context: conflictContext, action }),
    );
  const conflictApproval = await conflictClient.repositoryOperationsPreview({
    context: conflictContext,
    action: { kind: "fetch" },
  });
  let conflictFetch = (
    await conflictClient.repositoryOperationsApply(
      { ...conflictApproval.request, token: conflictApproval.token, confirm: true },
      conflictApproval,
    )
  ).operation;
  const conflictDeadline = Date.now() + 110000;
  while (["starting", "checking", "running", "inspecting"].includes(conflictFetch.state)) {
    assert(Date.now() < conflictDeadline);
    await Bun.sleep(20);
    conflictFetch = (
      await conflictClient.repositoryOperationsStatus(
        { context: conflictContext, operationId: conflictFetch.operationId },
        conflictFetch,
      )
    ).operation;
  }
  assert.equal(conflictFetch.state, "completed", JSON.stringify(conflictFetch));
  assert.deepEqual(await readFile(join(repo, "owned.txt")), conflictBytes);
  assert.deepEqual(await readFile(join(repo, ".git/MERGE_HEAD")), pendingBytes);
  await conflictClient.close();
  const unborn = join(root, "unborn");
  await mkdir(unborn);
  local("-C", unborn, "init", "-b", "main");
  local(
    "-C",
    unborn,
    "config",
    "core.attributesFile",
    process.platform === "win32" ? "NUL" : "/dev/null",
  );
  local("-C", unborn, "remote", "add", "origin", "https://github.com/owned/operations.git");
  for (const [path, expected] of [
    [unborn, "unborn"],
    [join(root, "missing"), "missing"],
    [seed, "invalid"],
  ] as const) {
    await writeFile(
      join(config, "settings.json"),
      JSON.stringify({
        repos: [{ ...identity, localPath: path }],
        defaultDiffView: "unified",
        theme: "dark",
      }),
    );
    const selectedClient = await open();
    const selectedPage = await selectedClient.repositoryOperationsList({
      repository: identity,
      offset: 0,
      limit: 100,
      version: null,
    });
    assert.equal(selectedPage.checkout.state, expected, JSON.stringify(selectedPage));
    if (expected === "unborn") {
      assert(selectedPage.checkout.context);
      assert.equal(selectedPage.branches.length, 0);
      await assert.rejects(() =>
        selectedClient.repositoryOperationsPreview({
          context: selectedPage.checkout.context!,
          action: { kind: "stash" },
        }),
      );
      await assert.rejects(() =>
        selectedClient.repositoryOperationsPreview({
          context: selectedPage.checkout.context!,
          action: { kind: "pull" },
        }),
      );
    }
    await assert.rejects(() =>
      selectedClient.repositoryOperationsPreview({ context, action: { kind: "stash" } }),
    );
    await selectedClient.close();
  }
  assert(proxy.requests.some((value) => value.startsWith("github.com/")));
  assert(proxy.requests.some((value) => value.startsWith("bitbucket.org/")));
  console.log(
    "PASS actual native IPC: both providers, loopback-only smart Git, fetch/ff-pull/divergence, failed proxy, cancellation, config drift, actual expiry, tracking/detached/local-commit semantics, ignored-file protection, hook drift uncertainty and conflict/missing/invalid/unborn state",
  );
  console.log(
    "PASS actual native IPC: typed literal checkout, action binding, one-use approval, dirty refusal, include-untracked stash, stale refusal and restart without replay",
  );
} finally {
  await proxy?.close();
  for (const client of clients) await client.close();
  await removeOwnedFixture();
}
