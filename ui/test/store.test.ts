import { expect, test } from "bun:test";
import { ShellStore } from "../src/app/store";
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

test("a late review start for a previous target is ignored", async () => {
  const store = new ShellStore();
  const client = new BackendClient(new FakeBackendTransport({ repositories, delayMs: 20 }));
  await store.connect(client);
  const pending = store.startReview(client);
  store.move(1);
  await pending;
  expect(store.getSnapshot().operation).toBeNull();
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
