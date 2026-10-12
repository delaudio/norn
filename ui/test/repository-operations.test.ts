import { expect, test } from "bun:test";
import { RepositoryOperationsStore } from "../src/app/repositoryOperationsStore";
import { BackendClient } from "../src/transport/backendClient";
import { RepositoryOperationsFixture } from "./fixtures/repository-operations-backend";

const repository = { provider: "github", workspace: "owned", repo: "same" } as const;

test("preview never applies, explicit confirmation is single-use and observed state refreshes", async () => {
  const fixture = new RepositoryOperationsFixture(),
    client = new BackendClient(fixture);
  let refreshed = 0;
  const store = new RepositoryOperationsStore(client, () => {
    refreshed++;
  });
  try {
    await store.load(repository);
    await store.checkout();
    expect(fixture.operationCalls.some((call) => call.method.endsWith(".apply"))).toBe(false);
    await store.confirm();
    await store.confirm();
    expect(fixture.operationCalls.filter((call) => call.method.endsWith(".apply"))).toHaveLength(1);
    expect(store.getSnapshot().operation?.state).toBe("completed");
    expect(refreshed).toBe(1);
    expect(store.getSnapshot().prepared).toBeNull();
  } finally {
    store.dispose();
    await client.close();
  }
});
test("expired approvals cannot execute and navigation fences late clone results", async () => {
  const fixture = new RepositoryOperationsFixture(),
    client = new BackendClient(fixture),
    store = new RepositoryOperationsStore(client);
  try {
    fixture.expiresInMs = 1;
    await store.load(repository);
    await store.checkout();
    await Bun.sleep(5);
    await store.confirm();
    expect(fixture.operationCalls.some((call) => call.method.endsWith(".apply"))).toBe(false);
    let release!: () => void;
    fixture.waitList = new Promise((resolve) => {
      release = resolve;
    });
    const first = store.load(repository);
    fixture.waitList = null;
    const other = { ...repository, provider: "bitbucket" as const };
    await store.load(other);
    release();
    await first;
    expect(store.getSnapshot().checkout?.repository.provider).toBe("bitbucket");
  } finally {
    store.dispose();
    await client.close();
  }
});
test("busy jobs cannot switch scope; cancellation waits for actual final status", async () => {
  const fixture = new RepositoryOperationsFixture(),
    client = new BackendClient(fixture),
    store = new RepositoryOperationsStore(client);
  try {
    fixture.hold = true;
    await store.load(repository);
    await store.preview({ kind: "fetch" });
    await store.confirm();
    expect(store.getSnapshot().busy).toBe(true);
    await store.load({ ...repository, provider: "bitbucket" });
    expect(store.getSnapshot().repository).toEqual(repository);
    await store.cancel();
    expect(store.getSnapshot().busy).toBe(true);
    fixture.hold = false;
    fixture.finalState = "canceled";
    const deadline = Date.now() + 2000;
    while (store.getSnapshot().busy && Date.now() < deadline) await Bun.sleep(10);
    expect(store.getSnapshot().operation?.state).toBe("canceled");
    expect(store.getSnapshot().busy).toBe(false);
    expect(fixture.operationCalls.filter((call) => call.method.endsWith(".apply"))).toHaveLength(1);
  } finally {
    store.dispose();
    await client.close();
  }
});
test("foreign operation results fail closed without retrying Git", async () => {
  const fixture = new RepositoryOperationsFixture(),
    client = new BackendClient(fixture),
    store = new RepositoryOperationsStore(client);
  try {
    fixture.malformed = true;
    await store.load(repository);
    await store.checkout();
    await store.confirm();
    expect(store.getSnapshot().error).toContain("Outcome unknown");
    expect(fixture.operationCalls.filter((call) => call.method.endsWith(".apply"))).toHaveLength(1);
  } finally {
    store.dispose();
    await client.close();
  }
});
