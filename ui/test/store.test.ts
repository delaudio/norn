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
