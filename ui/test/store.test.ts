import { expect, test } from "bun:test";
import { ShellStore, visibleFindings } from "../src/app/store";
import { BackendClient } from "../src/transport/backendClient";
import { FakeBackendTransport } from "../src/transport/fakeTransport";
import type { TargetIdentity } from "../src/transport/protocol";

const repositories = [
  { provider: "github" as const, workspace: "acme", repo: "payments", localPath: null },
  { provider: "github" as const, workspace: "acme", repo: "billing", localPath: null },
];

test("connect loads repositories and move wraps the selection", async () => {
  const store = new ShellStore();
  await store.connect(new BackendClient(new FakeBackendTransport({ repositories })));
  expect(store.getSnapshot().status).toBe("ready");
  expect(store.getSnapshot().repositories).toHaveLength(2);
  store.move(1);
  expect(store.current?.repo).toBe("billing");
  store.move(1);
  expect(store.current?.repo).toBe("payments");
});

test("a late review start preserves its original target and remains cancellable", async () => {
  const store = new ShellStore();
  const client = new BackendClient(new FakeBackendTransport({ repositories, delayMs: 20 }));
  await store.connect(client);
  const pending = store.startReview(client);
  store.move(1);
  await pending;
  expect(store.current?.repo).toBe("billing");
  expect(store.getSnapshot().operation?.targetLabel).toContain("payments");
  const id = store.getSnapshot().operation!.id;
  await store.cancel(client);
  expect((await client.operationStatus(id)).state).toBe("cancelled");
});

test("operation events are applied only for the active operation", async () => {
  const store = new ShellStore();
  const client = new BackendClient(new FakeBackendTransport({ repositories }));
  await store.connect(client);
  await store.startReview(client);
  const operationId = store.getSnapshot().operation?.id;
  expect(operationId).toBeDefined();
  const target: TargetIdentity = {
    kind: "provider",
    provider: "github",
    workspace: "acme",
    repo: "payments",
  };
  store.applyEvent({
    operationId: "unknown",
    sequence: 9,
    target,
    event: { kind: "state", state: "failed" },
  });
  expect(store.getSnapshot().operation?.state).not.toBe("failed");
  store.applyEvent({
    operationId: operationId!,
    sequence: 10,
    target,
    event: { kind: "state", state: "cancelled" },
  });
  expect(store.getSnapshot().operation?.state).toBe("cancelled");
});

const files = [
  { path: "src/a.ts", status: "modified" as const, additions: 2, deletions: 1, oldPath: null },
  { path: "src/b.ts", status: "added" as const, additions: 5, deletions: 0, oldPath: null },
];
const findings = [
  {
    id: "f1",
    title: "Off-by-one",
    severity: "high" as const,
    summary: "s",
    anchor: { path: "src/b.ts", startLine: 1, endLine: null, side: "new" as const },
  },
  { id: "f2", title: "No anchor", severity: "low" as const, summary: "s", anchor: null },
];

test("snapshot loads files and findings and lazily loads the selected diff", async () => {
  const store = new ShellStore();
  const client = new BackendClient(new FakeBackendTransport({ repositories, files, findings }));
  await store.connect(client);
  await new Promise((resolve) => setTimeout(resolve, 5));
  expect(store.getSnapshot().files).toHaveLength(2);
  expect(store.getSnapshot().findings).toHaveLength(2);
  expect(store.getSnapshot().diff?.path).toBe("src/a.ts");

  store.cycleFocus(); // repositories -> files
  store.move(1); // select src/b.ts
  await new Promise((resolve) => setTimeout(resolve, 5));
  expect(store.getSnapshot().diff?.path).toBe("src/b.ts");
});

test("jumping to a finding selects its anchor file and reports a missing anchor", async () => {
  const store = new ShellStore();
  const client = new BackendClient(new FakeBackendTransport({ repositories, files, findings }));
  await store.connect(client);
  await new Promise((resolve) => setTimeout(resolve, 5));

  store.cycleFocus(); // repositories -> files
  store.cycleFocus(); // files -> findings
  // findings[0] has an anchor into src/b.ts
  store.move(1); // -> findings[1], no anchor
  expect(store.getSnapshot().notice).toContain("no anchor");
  store.move(1); // -> findings[0] (wrap), jumps to src/b.ts line 1
  await new Promise((resolve) => setTimeout(resolve, 5));
  expect(store.getSnapshot().files[store.getSnapshot().selectedFile]?.path).toBe("src/b.ts");
  expect(store.getSnapshot().highlightLine).toBe(1);
});

test("a stale diff response is ignored after switching files", async () => {
  const store = new ShellStore();
  const client = new BackendClient(
    new FakeBackendTransport({ repositories, files, findings, delayMs: 20 }),
  );
  await store.connect(client);
  await new Promise((resolve) => setTimeout(resolve, 30));
  store.cycleFocus();
  store.move(1);
  store.move(1); // back to src/a.ts before the delayed response resolves
  await new Promise((resolve) => setTimeout(resolve, 40));
  expect(store.getSnapshot().diff?.path).toBe("src/a.ts");
});

test("an old-side anchor highlights the deleted line", async () => {
  const oldSide = [
    {
      id: "f1",
      title: "Old line",
      severity: "high" as const,
      summary: "s",
      anchor: { path: "src/a.ts", startLine: 1, endLine: null, side: "old" as const },
    },
    { id: "f2", title: "No anchor", severity: "low" as const, summary: "s", anchor: null },
  ];
  const store = new ShellStore();
  const client = new BackendClient(
    new FakeBackendTransport({ repositories, files, findings: oldSide }),
  );
  await store.connect(client);
  await new Promise((resolve) => setTimeout(resolve, 5));
  store.cycleFocus();
  store.cycleFocus();
  store.move(1);
  store.move(1);
  await new Promise((resolve) => setTimeout(resolve, 5));
  expect(store.getSnapshot().highlightSide).toBe("old");
  expect(store.getSnapshot().highlightLine).toBe(1);
});

test("an anchor line absent from the diff surfaces a notice", async () => {
  const stale = [
    {
      id: "f1",
      title: "Stale line",
      severity: "high" as const,
      summary: "s",
      anchor: { path: "src/a.ts", startLine: 99, endLine: null, side: "new" as const },
    },
    { id: "f2", title: "No anchor", severity: "low" as const, summary: "s", anchor: null },
  ];
  const store = new ShellStore();
  const client = new BackendClient(
    new FakeBackendTransport({ repositories, files, findings: stale }),
  );
  await store.connect(client);
  await new Promise((resolve) => setTimeout(resolve, 5));
  store.cycleFocus();
  store.cycleFocus();
  store.move(1);
  store.move(1);
  await new Promise((resolve) => setTimeout(resolve, 5));
  expect(store.getSnapshot().notice).toContain("not in the current diff");
  expect(store.getSnapshot().highlightLine).toBeNull();
});

test("switching repositories clears a pending diff loading state", async () => {
  const store = new ShellStore();
  const client = new BackendClient(
    new FakeBackendTransport({ repositories, files, findings, delayMs: 20 }),
  );
  await store.connect(client);
  await new Promise((resolve) => setTimeout(resolve, 30));
  store.cycleFocus();
  store.move(1);
  expect(store.getSnapshot().diffLoading).toBe(true);
  const switching = store.selectRepository(1);
  expect(store.getSnapshot().diffLoading).toBe(false);
  await switching;
});

test("an old-side finding resolves through a renamed file's old path", async () => {
  const renamed = [
    {
      path: "src/renamed.ts",
      status: "renamed" as const,
      additions: 1,
      deletions: 0,
      oldPath: "src/original.ts",
    },
  ];
  const renameFindings = [
    {
      id: "f1",
      title: "Old path anchor",
      severity: "high" as const,
      summary: "s",
      anchor: { path: "src/original.ts", startLine: 1, endLine: null, side: "old" as const },
    },
    { id: "f2", title: "No anchor", severity: "low" as const, summary: "s", anchor: null },
  ];
  const store = new ShellStore();
  const client = new BackendClient(
    new FakeBackendTransport({ repositories, files: renamed, findings: renameFindings }),
  );
  await store.connect(client);
  await new Promise((resolve) => setTimeout(resolve, 5));
  store.cycleFocus();
  store.cycleFocus();
  store.move(1);
  store.move(1);
  await new Promise((resolve) => setTimeout(resolve, 5));
  expect(store.getSnapshot().files[store.getSnapshot().selectedFile]?.path).toBe("src/renamed.ts");
  expect(store.getSnapshot().highlightSide).toBe("old");
  expect(store.getSnapshot().highlightLine).toBe(1);
});

test("stored review targets are selected with their prId", async () => {
  const targets = [
    {
      provider: "github" as const,
      workspace: "acme",
      repo: "payments",
      prId: 42,
      runId: "run-42",
      title: "Fix nulls",
      status: "succeeded",
    },
    {
      provider: "github" as const,
      workspace: "acme",
      repo: "billing",
      prId: 7,
      runId: "run-7",
      title: "Retry",
      status: "failed",
    },
  ];
  const transport = new FakeBackendTransport({
    repositories: [{ provider: "github", workspace: "acme", repo: "payments", localPath: null }],
    targets,
    files,
    findings,
  });
  const original = transport.request.bind(transport);
  const requested: Array<Record<string, unknown>> = [];
  transport.request = (method, params) => {
    if (method === "review.findings") {
      requested.push(params.target as Record<string, unknown>);
    }
    return original(method, params);
  };
  const store = new ShellStore();
  await store.connect(new BackendClient(transport));
  await new Promise((resolve) => setTimeout(resolve, 5));
  // Working-tree entry first, then the stored reviews.
  expect(store.getSnapshot().picker).toHaveLength(3);
  expect(requested.at(-1)?.prId).toBeUndefined();
  store.move(1);
  await new Promise((resolve) => setTimeout(resolve, 5));
  expect(requested.at(-1)?.prId).toBe(42);
  store.move(1);
  await new Promise((resolve) => setTimeout(resolve, 5));
  expect(requested.at(-1)?.prId).toBe(7);
});

test("a successful diff load clears a previous diff error", async () => {
  const transport = new FakeBackendTransport({ repositories, files, findings });
  const original = transport.request.bind(transport);
  let failNext = false;
  transport.request = (method, params) => {
    if (method === "diff.file" && failNext) {
      failNext = false;
      return Promise.reject(new Error("boom"));
    }
    return original(method, params);
  };
  const store = new ShellStore();
  await store.connect(new BackendClient(transport));
  await new Promise((resolve) => setTimeout(resolve, 5));
  store.cycleFocus();
  failNext = true;
  store.move(1);
  await new Promise((resolve) => setTimeout(resolve, 5));
  expect(store.getSnapshot().diffError).toContain("boom");
  store.move(1);
  await new Promise((resolve) => setTimeout(resolve, 5));
  expect(store.getSnapshot().diffError).toBeNull();
  expect(store.getSnapshot().diff?.path).toBe("src/a.ts");
});

test("rapid file moves coalesce diff requests to the latest selection", async () => {
  const transport = new FakeBackendTransport({ repositories, files, findings, delayMs: 10 });
  const original = transport.request.bind(transport);
  let diffCalls = 0;
  transport.request = (method, params) => {
    if (method === "diff.file") {
      diffCalls += 1;
    }
    return original(method, params);
  };
  const store = new ShellStore();
  await store.connect(new BackendClient(transport));
  await new Promise((resolve) => setTimeout(resolve, 40));
  diffCalls = 0;
  store.cycleFocus();
  for (let step = 0; step < 20; step += 1) {
    store.move(1);
  }
  await new Promise((resolve) => setTimeout(resolve, 80));
  expect(diffCalls).toBeLessThanOrEqual(2);
  const snapshot = store.getSnapshot();
  expect(snapshot.diff?.path).toBe(snapshot.files[snapshot.selectedFile]?.path);
});

test("rapid target moves coalesce snapshot requests", async () => {
  const transport = new FakeBackendTransport({ repositories, files, findings, delayMs: 10 });
  const original = transport.request.bind(transport);
  let fileCalls = 0;
  transport.request = (method, params) => {
    if (method === "review.files") {
      fileCalls += 1;
    }
    return original(method, params);
  };
  const store = new ShellStore();
  await store.connect(new BackendClient(transport));
  await new Promise((resolve) => setTimeout(resolve, 40));
  fileCalls = 0;
  for (let step = 0; step < 20; step += 1) {
    store.move(1);
  }
  await new Promise((resolve) => setTimeout(resolve, 80));
  expect(fileCalls).toBeLessThanOrEqual(2);
});

test("toggleDiffMode switches between unified and split", () => {
  const store = new ShellStore();
  expect(store.getSnapshot().diffMode).toBe("unified");
  store.toggleDiffMode();
  expect(store.getSnapshot().diffMode).toBe("split");
  store.toggleDiffMode();
  expect(store.getSnapshot().diffMode).toBe("unified");
});

test("toggleDiffExpanded toggles and resets when a diff loads", async () => {
  const store = new ShellStore();
  expect(store.getSnapshot().diffExpanded).toBe(false);
  store.toggleDiffExpanded();
  expect(store.getSnapshot().diffExpanded).toBe(true);
  const client = new BackendClient(new FakeBackendTransport({ repositories, files, findings }));
  await store.connect(client);
  await new Promise((resolve) => setTimeout(resolve, 5));
  expect(store.getSnapshot().diffExpanded).toBe(false);
});

test("openBrowser surfaces the authenticated session URL", async () => {
  const store = new ShellStore();
  const client = new BackendClient(new FakeBackendTransport({ repositories }));
  await store.connect(client);
  await new Promise((resolve) => setTimeout(resolve, 5));
  await store.openBrowser(client);
  expect(store.getSnapshot().notice).toContain("/session/");
});

test("image files load a bounded preview", async () => {
  const store = new ShellStore();
  const client = new BackendClient(
    new FakeBackendTransport({
      repositories,
      files: [
        { path: "images/logo.png", status: "modified", additions: 0, deletions: 0, oldPath: null },
      ],
      findings: [],
    }),
  );
  await store.connect(client);
  await new Promise((resolve) => setTimeout(resolve, 5));
  expect(store.getSnapshot().preview?.path).toBe("images/logo.png");
  expect(store.getSnapshot().preview?.after?.mimeType).toBe("image/png");
});

test("a running operation survives a target switch", async () => {
  const store = new ShellStore();
  const client = new BackendClient(
    new FakeBackendTransport({ repositories, files, findings, delayMs: 5 }),
  );
  await store.connect(client);
  await new Promise((resolve) => setTimeout(resolve, 10));
  await store.startReview(client);
  expect(store.getSnapshot().operation?.targetLabel).toContain("payments");
  store.move(1); // switch to the second repository
  await new Promise((resolve) => setTimeout(resolve, 20));
  expect(store.getSnapshot().operation).not.toBeNull();
  expect(store.getSnapshot().operation?.targetLabel).toContain("payments");
});

test("loads the stored review history for a target", async () => {
  const store = new ShellStore();
  const client = new BackendClient(
    new FakeBackendTransport({
      repositories,
      files,
      findings,
      history: [
        {
          runId: "run-1",
          createdAt: "2026-01-01T00:00:00Z",
          status: "succeeded",
          turnKind: "initial",
          findingsCount: 2,
        },
      ],
    }),
  );
  await store.connect(client);
  await new Promise((resolve) => setTimeout(resolve, 5));
  expect(store.getSnapshot().history).toHaveLength(1);
  expect(store.getSnapshot().history[0]?.findingsCount).toBe(2);
});

test("composing a reply submits it as an operation", async () => {
  const store = new ShellStore();
  const client = new BackendClient(
    new FakeBackendTransport({
      repositories,
      targets: [
        {
          provider: "github",
          workspace: "acme",
          repo: "payments",
          prId: 42,
          runId: "run-42",
          title: "Review",
          status: "succeeded",
        },
      ],
      files,
      findings,
    }),
  );
  await store.connect(client);
  await new Promise((resolve) => setTimeout(resolve, 5));
  store.move(1);
  store.move(1); // select the stored review target
  await new Promise((resolve) => setTimeout(resolve, 5));
  store.beginReply();
  expect(store.getSnapshot().composing).toBe(true);
  store.appendReplyText("Please add tests");
  store.backspaceReply();
  store.appendReplyText("s");
  expect(store.getSnapshot().composerText).toBe("Please add tests");
  await store.submitReply(client);
  expect(store.getSnapshot().composing).toBe(false);
  expect(store.getSnapshot().operation?.state).toBe("accepted");
});

test("finding grouping preserves identity and filters navigate only visible rows", async () => {
  const store = new ShellStore();
  const client = new BackendClient(new FakeBackendTransport({ repositories, files, findings }));
  await store.connect(client);
  store.cycleFocus();
  store.cycleFocus();
  store.move(1);
  expect(visibleFindings(store.getSnapshot())[store.getSnapshot().selectedFinding]?.id).toBe("f2");
  store.cycleFindingGroup();
  store.cycleFindingGroup();
  expect(visibleFindings(store.getSnapshot())[store.getSnapshot().selectedFinding]?.id).toBe("f2");
  store.cycleFindingFilter(); // critical: no matches
  expect(visibleFindings(store.getSnapshot())).toHaveLength(0);
  expect(store.getSnapshot().highlightLine).toBeNull();
  expect(store.move(1)).toBe(false);
  store.cycleFindingFilter(); // high
  expect(visibleFindings(store.getSnapshot()).map((finding) => finding.id)).toEqual(["f1"]);
  store.move(1);
  expect(store.getSnapshot().selectedFinding).toBe(0);
});

test("selecting a thread pins diff findings and reply to that thread's latest run", async () => {
  const requests: Array<{ method: string; params: Record<string, unknown> }> = [];
  class RecordingTransport extends FakeBackendTransport {
    override async request(method: string, params: Record<string, unknown>) {
      requests.push({ method, params });
      return super.request(method, params);
    }
  }
  const transport = new RecordingTransport({
    repositories,
    files,
    findings,
    targets: [
      {
        provider: "github",
        workspace: "acme",
        repo: "payments",
        prId: 7,
        runId: "run-1",
        title: "PR",
        status: "succeeded",
      },
    ],
    threads: [
      { id: "t1", title: "First", latestRunId: "run-1", messages: [], truncated: false },
      {
        id: "t2",
        title: "Second",
        latestRunId: "run-2",
        messages: [{ id: "m1", role: "assistant", content: "Unicode ✓", truncated: false }],
        truncated: false,
      },
    ],
  });
  const client = new BackendClient(transport);
  const store = new ShellStore();
  await store.connect(client);
  await store.selectPicker(2);
  store.beginReply();
  store.appendReplyText("old draft");
  store.cancelReply();
  store.toggleThreads();
  store.move(1);
  store.beginReply();
  expect(store.getSnapshot().composing).toBe(false);
  expect(store.getSnapshot().notice).toContain("load the selected thread");
  await store.selectThread();
  expect(store.getSnapshot().composerText).toBe("");
  expect(store.getSnapshot().selectedThread).toBe(1);
  expect(store.getSnapshot().threads[1]?.messages[0]?.content).toBe("Unicode ✓");
  store.beginReply();
  store.appendReplyText("Continue second thread");
  await store.submitReply(client);
  const reply = [...requests].reverse().find((request) => request.method === "review.reply");
  expect(reply?.params.target).toMatchObject({ runId: "run-2", prId: 7 });
  const load = [...requests].reverse().find((request) => request.method === "review.findings");
  expect(load?.params.target).toMatchObject({ runId: "run-2" });
});

test("fix preview requires a saved run, explicit approval and no automatic retry", async () => {
  const transport = new FakeBackendTransport({
    repositories,
    targets: [
      {
        provider: "github",
        workspace: "acme",
        repo: "payments",
        prId: 9,
        runId: "run-fix",
        title: "Fix",
        status: "succeeded",
      },
    ],
  });
  const client = new BackendClient(transport);
  const store = new ShellStore();
  await store.connect(client);
  await store.previewFix(client);
  expect(store.getSnapshot().fixPreview).toBeNull();
  await store.selectPicker(2);
  await store.previewFix(client);
  expect(store.getSnapshot().operation).toBeNull();
  const preview = store.getSnapshot().fixPreview!;
  expect(preview.effects.join(" ")).toContain("edit permissions");
  store.dismissFix();
  await store.confirmFix(client);
  expect(store.getSnapshot().operation).toBeNull();
  await store.previewFix(client);
  await store.confirmFix(client);
  const operation = store.getSnapshot().operation!;
  expect(operation.id).toContain("fake-fix");
  expect(store.getSnapshot().fixPreview).toBeNull();
  await store.confirmFix(client);
  expect(store.getSnapshot().operation?.id).toBe(operation.id);
  store.applyEvent({
    operationId: operation.id,
    target: preview.target,
    sequence: 1,
    event: { kind: "state", state: "succeeded" },
  });
  await Bun.sleep(5);
  expect(store.getSnapshot().fixState?.filesTouched).toEqual(["src/app.ts"]);
});

test("changing targets clears and fences a late fix approval", async () => {
  const client = new BackendClient(
    new FakeBackendTransport({
      repositories,
      delayMs: 15,
      targets: [
        {
          provider: "github",
          workspace: "acme",
          repo: "payments",
          prId: 9,
          runId: "run-fix",
          title: "Fix",
          status: "succeeded",
        },
      ],
    }),
  );
  const store = new ShellStore();
  await store.connect(client);
  await store.selectPicker(2);
  const pending = store.previewFix(client);
  await store.selectPicker(0);
  await pending;
  expect(store.getSnapshot().fixPreview).toBeNull();
  expect(store.getSnapshot().fixPreviewLoading).toBe(false);
});

test("commit composition previews exact text and push requires separate approval", async () => {
  class RecordingTransport extends FakeBackendTransport {
    requests: string[] = [];
    async request(method: string, params: Record<string, unknown>) {
      this.requests.push(method);
      return super.request(method, params);
    }
  }
  const transport = new RecordingTransport({
    repositories,
    targets: [
      {
        provider: "github",
        workspace: "acme",
        repo: "payments",
        prId: 9,
        runId: "run-fix",
        title: "Fix",
        status: "succeeded",
      },
    ],
  });
  const client = new BackendClient(transport);
  const store = new ShellStore();
  await store.connect(client);
  await store.selectPicker(2);
  await store.beginCommit(client);
  expect(store.getSnapshot().composerKind).toBe("commit");
  expect(store.getSnapshot().composing).toBe(true);
  store.appendReplyText(" ✓");
  await store.previewCommit(client);
  const preview = store.getSnapshot().fixPreview!;
  expect(preview.action).toBe("commit");
  expect(preview.commitMessage).toBe("fix: guard null ✓");
  expect(store.getSnapshot().operation).toBeNull();
  expect(store.getSnapshot().composing).toBe(false);
  await store.confirmFix(client);
  const commit = store.getSnapshot().operation!;
  expect(commit.id).toContain("fake-commit");
  store.applyEvent({
    operationId: commit.id,
    target: preview.target,
    sequence: 1,
    event: { kind: "state", state: "succeeded" },
  });
  await Bun.sleep(5);
  expect(store.getSnapshot().fixState?.phase).toBe("readyToPush");
  expect(store.getSnapshot().fixState?.commitSha).toBe("c".repeat(40));
  await store.previewPush(client);
  expect(store.getSnapshot().fixPreview?.action).toBe("push");
  expect(transport.requests).not.toContain("review.push");
  await store.confirmFix(client);
  expect(store.getSnapshot().operation?.id).toContain("fake-push");
  expect(transport.requests).not.toContain("review.reply");
  expect(transport.requests.filter((method) => method === "review.commit")).toHaveLength(1);
  expect(transport.requests.filter((method) => method === "review.push")).toHaveLength(1);
});

test("a late commit-state response cannot open the composer on another target", async () => {
  const client = new BackendClient(
    new FakeBackendTransport({
      repositories,
      delayMs: 15,
      targets: [
        {
          provider: "github",
          workspace: "acme",
          repo: "payments",
          prId: 9,
          runId: "run-fix",
          title: "Fix",
          status: "succeeded",
        },
      ],
    }),
  );
  const store = new ShellStore();
  await store.connect(client);
  await store.selectPicker(2);
  const pending = store.beginCommit(client);
  await store.selectPicker(0);
  await pending;
  expect(store.getSnapshot().composing).toBe(false);
  expect(store.getSnapshot().fixPreview).toBeNull();
});

test("sync and conflict resolution preview distinct effects and require explicit approval", async () => {
  const client = new BackendClient(
    new FakeBackendTransport({
      repositories,
      targets: [
        {
          provider: "github",
          workspace: "acme",
          repo: "payments",
          prId: 9,
          runId: "run-fix",
          title: "Fix",
          status: "succeeded",
        },
      ],
    }),
  );
  const store = new ShellStore();
  await store.connect(client);
  await store.selectPicker(2);
  await store.previewBranch(client, "sync");
  expect(store.getSnapshot().fixPreview?.action).toBe("sync");
  expect(store.getSnapshot().operation).toBeNull();
  store.dismissFix();
  expect(store.getSnapshot().operation).toBeNull();
  await store.previewBranch(client, "conflicts");
  const preview = store.getSnapshot().fixPreview!;
  expect(preview.action).toBe("conflicts");
  expect(preview.effects.join(" ")).toContain("resolve conflicts");
  await store.confirmFix(client);
  const operation = store.getSnapshot().operation!;
  expect(operation.id).toContain("fake-conflicts");
  store.applyEvent({
    operationId: operation.id,
    sequence: 1,
    target: preview.target,
    event: { kind: "state", state: "succeeded" },
  });
  await Bun.sleep(5);
  expect(store.getSnapshot().fixState?.phase).toBe("readyToCommit");
});

class ControlledOperationsTransport extends FakeBackendTransport {
  requests: { method: string; params: Record<string, unknown> }[] = [];
  state = "running";
  sequence = 0;
  nextRun = "run-old";
  requestDelay = 0;
  statusDelay = 0;
  error: { code: string; message: string } | null = null;
  private next = 1;
  private id = "";
  async request(method: string, params: Record<string, unknown>) {
    this.requests.push({ method, params });
    if (["review.start", "review.reply", "review.fix"].includes(method)) {
      this.id = `controlled-${this.next++}`;
      if (this.requestDelay) await Bun.sleep(this.requestDelay);
      return { operationId: this.id, state: "accepted", sequence: 0 };
    }
    if (method === "operation.status" || method === "operation.cancel") {
      if (this.statusDelay) await Bun.sleep(this.statusDelay);
      if (method === "operation.cancel") this.state = "cancelled";
      return {
        operationId: params.operationId,
        state: this.state,
        sequence: this.sequence,
        error: this.error,
      };
    }
    if (method === "review.targets")
      return {
        targets: [
          {
            provider: "github",
            workspace: "acme",
            repo: "payments",
            prId: 9,
            runId: this.nextRun,
            title: "Review",
            status: "succeeded",
          },
        ],
      };
    return super.request(method, params);
  }
}

async function controlledReview() {
  const transport = new ControlledOperationsTransport({ repositories, files, findings });
  const client = new BackendClient(transport);
  const store = new ShellStore();
  await store.connect(client);
  await store.selectPicker(2);
  const target: TargetIdentity = {
    kind: "provider",
    provider: "github",
    workspace: "acme",
    repo: "payments",
    prId: 9,
    runId: "run-old",
  };
  return { transport, client, store, target };
}

test("late reply ownership survives navigation without reopening another target's composer", async () => {
  const { transport, client, store } = await controlledReview();
  transport.requestDelay = 20;
  store.beginReply();
  store.appendReplyText("Please explain");
  const pending = store.submitReply(client);
  await store.selectPicker(1);
  await pending;
  expect(store.getSnapshot().operation?.targetLabel).toContain("payments");
  expect(store.getSnapshot().picker[store.getSnapshot().selectedPicker]?.repo).toBe("billing");
  expect(store.getSnapshot().composing).toBe(false);
  const id = store.getSnapshot().operation!.id;
  await store.cancel(client);
  expect(
    transport.requests
      .slice()
      .reverse()
      .find((r) => r.method === "operation.cancel")?.params.operationId,
  ).toBe(id);
});

test("late approved fix stays cancellable without attaching its result to another picker target", async () => {
  const { transport, client, store, target } = await controlledReview();
  await store.previewFix(client);
  transport.requestDelay = 20;
  const pending = store.confirmFix(client);
  await store.selectPicker(1);
  await pending;
  const operation = store.getSnapshot().operation!;
  expect(operation.targetLabel).toContain("payments");
  store.applyEvent({
    operationId: operation.id,
    target,
    sequence: 1,
    event: { kind: "state", state: "failed" },
  });
  await Bun.sleep(5);
  expect(store.getSnapshot().fixState).toBeNull();
  await store.cancel(client);
  expect(
    transport.requests
      .slice()
      .reverse()
      .find((r) => r.method === "operation.cancel")?.params.operationId,
  ).toBe(operation.id);
});

test("wrong-target, duplicate and old events cannot change the tracked operation", async () => {
  const { transport, client, store, target } = await controlledReview();
  await store.startReview(client);
  await Bun.sleep(5);
  const id = store.getSnapshot().operation!.id;
  store.applyEvent({
    operationId: id,
    sequence: 1,
    target: { ...target, repo: "billing" },
    event: { kind: "state", state: "failed" },
  });
  expect(store.getSnapshot().operation?.state).toBe("running");
  store.applyEvent({
    operationId: id,
    sequence: 1,
    target,
    event: { kind: "progress", message: "correct" },
  });
  const count = store.getSnapshot().operation!.logs.length;
  store.applyEvent({
    operationId: id,
    sequence: 1,
    target,
    event: { kind: "state", state: "failed" },
  });
  store.applyEvent({
    operationId: id,
    sequence: 0,
    target,
    event: { kind: "state", state: "failed" },
  });
  expect(store.getSnapshot().operation?.logs).toHaveLength(count);
  expect(store.getSnapshot().operation?.state).toBe("running");
  expect(transport.requests.filter((r) => r.method === "operation.status")).toHaveLength(1);
});

test("burst gaps coalesce status reads and preserve bounded logs", async () => {
  const { transport, client, store, target } = await controlledReview();
  await store.startReview(client);
  await Bun.sleep(5);
  transport.statusDelay = 20;
  transport.sequence = 120;
  const id = store.getSnapshot().operation!.id;
  for (let sequence = 3; sequence <= 120; sequence++)
    store.applyEvent({
      operationId: id,
      sequence,
      target,
      event: { kind: "progress", message: `chunk ${sequence}` },
    });
  await Bun.sleep(70);
  expect(store.getSnapshot().operation?.sequence).toBe(120);
  expect(store.getSnapshot().operation!.logs.length).toBeLessThanOrEqual(100);
  expect(
    transport.requests.filter((r) => r.method === "operation.status").length,
  ).toBeLessThanOrEqual(3);
});

test("completion adopts the new saved run and preserves finding identity", async () => {
  const { transport, client, store, target } = await controlledReview();
  store.cycleFocus();
  store.cycleFocus();
  store.move(1);
  const selectedId = visibleFindings(store.getSnapshot())[store.getSnapshot().selectedFinding]?.id;
  await store.startReview(client);
  await Bun.sleep(5);
  const id = store.getSnapshot().operation!.id;
  transport.nextRun = "run-new";
  transport.state = "succeeded";
  transport.sequence = 1;
  store.applyEvent({
    operationId: id,
    target,
    sequence: 1,
    event: { kind: "state", state: "succeeded" },
  });
  await Bun.sleep(15);
  expect(store.getSnapshot().picker[store.getSnapshot().selectedPicker]?.runId).toBe("run-new");
  expect(
    transport.requests
      .slice()
      .reverse()
      .find((r) => r.method === "review.findings")?.params.target,
  ).toMatchObject({ runId: "run-new" });
  expect(visibleFindings(store.getSnapshot())[store.getSnapshot().selectedFinding]?.id).toBe(
    selectedId,
  );
});

test("an unresolved start blocks another review and fix after navigation", async () => {
  const { transport, client, store } = await controlledReview();
  transport.requestDelay = 30;
  const pending = store.startReview(client);
  await store.selectPicker(1);
  await store.startReview(client);
  await store.previewFix(client);
  await pending;
  expect(transport.requests.filter((r) => r.method === "review.start")).toHaveLength(1);
  expect(transport.requests.filter((r) => r.method === "review.fix.preview")).toHaveLength(0);
});

test("authoritative polling recovers a missing terminal reply event and preserves the draft", async () => {
  const { transport, client, store } = await controlledReview();
  store.beginReply();
  store.appendReplyText("Keep this draft");
  await store.submitReply(client);
  await Bun.sleep(5);
  transport.state = "failed";
  transport.sequence = 3;
  transport.error = { code: "cancelled", message: "native interruption" };
  await store.pollOperation();
  expect(store.getSnapshot().operation?.state).toBe("failed");
  expect(store.getSnapshot().operation?.logs.at(-1)?.text).toContain("native interruption");
  expect(store.getSnapshot().composing).toBe(true);
  expect(store.getSnapshot().composerText).toBe("Keep this draft");
  const starts = transport.requests.filter((r) =>
    ["review.start", "review.reply", "review.fix"].includes(r.method),
  ).length;
  await store.refreshOperation();
  expect(
    transport.requests.filter((r) =>
      ["review.start", "review.reply", "review.fix"].includes(r.method),
    ),
  ).toHaveLength(starts);
});

test("loads the bounded global review history", async () => {
  const store = new ShellStore();
  const client = new BackendClient(
    new FakeBackendTransport({
      repositories,
      jobs: [
        {
          id: "job-1",
          workspace: "acme",
          repo: "payments",
          prId: 42,
          prTitle: "Title",
          sourceBranch: "feature/x",
          destinationBranch: "main",
          status: "succeeded",
          trigger: "manual",
          threadId: null,
          error: null,
          createdAt: "2026-01-01T00:00:00Z",
          startedAt: null,
          finishedAt: null,
        },
      ],
    }),
  );
  await store.connect(client);
  await store.loadJobs(client);
  expect(store.getSnapshot().jobs).toHaveLength(1);
  expect(store.getSnapshot().notice).toContain("Review history");
});

test("loads bounded closed-PR analytics", async () => {
  const store = new ShellStore();
  const client = new BackendClient(
    new FakeBackendTransport({
      repositories,
      closedMetrics: [
        {
          workspace: "acme",
          repo: "payments",
          prId: 1,
          title: "Title",
          authorDisplayName: "Author",
          authorAccountId: null,
          state: "MERGED",
          sourceBranch: "feature/x",
          destinationBranch: "main",
          createdOn: "",
          updatedOn: "",
          additions: 1,
          deletions: 0,
          filesChanged: 1,
          diffstatCached: true,
          risk: {
            hasAiReview: true,
            impact: "high",
            totalFindings: 2,
            highOrCriticalFindings: 1,
            severityCounts: [],
            categoryCounts: [],
          },
          syncedAt: "",
        },
      ],
    }),
  );
  await store.connect(client);
  await store.loadClosedMetrics(client);
  expect(store.getSnapshot().closedMetrics).toHaveLength(1);
  expect(store.getSnapshot().notice).toContain("Closed PRs");
});

test("refresh updates jobs without resetting the selection", async () => {
  const store = new ShellStore();
  const client = new BackendClient(
    new FakeBackendTransport({
      repositories,
      jobs: [
        {
          id: "job-1",
          workspace: "acme",
          repo: "payments",
          prId: 42,
          prTitle: "Title",
          sourceBranch: "feature/x",
          destinationBranch: "main",
          status: "succeeded",
          trigger: "manual",
          threadId: null,
          error: null,
          createdAt: "2026-01-01T00:00:00Z",
          startedAt: null,
          finishedAt: null,
        },
      ],
    }),
  );
  await store.connect(client);
  const selectedBefore = store.getSnapshot().selectedPicker;
  await store.refresh(client);
  expect(store.getSnapshot().jobs).toHaveLength(1);
  expect(store.getSnapshot().lastSyncAt).not.toBeNull();
  expect(store.getSnapshot().syncing).toBe(false);
  expect(store.getSnapshot().selectedPicker).toBe(selectedBefore);
});

test("refresh clears the syncing flag when the connection is closed", async () => {
  const store = new ShellStore();
  const client = new BackendClient(new FakeBackendTransport({ repositories, jobs: [] }));
  await store.connect(client);
  store.markClosed();
  await store.refresh(client);
  expect(store.getSnapshot().syncing).toBe(false);
});
