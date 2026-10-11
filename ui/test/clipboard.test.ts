import { expect, test } from "bun:test";
import { ShellStore } from "../src/app/store";
import { BackendClient } from "../src/transport/backendClient";
import { FakeBackendTransport } from "../src/transport/fakeTransport";

const repositories = [{ provider: "github", workspace: "acme", repo: "app", localPath: null }];
const targets = [
  {
    provider: "github",
    workspace: "acme",
    repo: "app",
    prId: 7,
    runId: "run",
    title: "Stored review",
    status: "succeeded",
  },
];
test("copy is explicit, exact and reports emission without claiming clipboard acknowledgement", async () => {
  const client = new BackendClient(
    new FakeBackendTransport({ repositories, targets, reviewOutput: "## Entire output\n世界\n" }),
  );
  const store = new ShellStore();
  const captures: string[] = [];
  const copy = (text: string) => {
    captures.push(text);
    return true;
  };
  await store.connect(client);
  store.move(1);
  await Bun.sleep(1);
  expect(captures).toHaveLength(0);
  await store.copyReviewOutput(client, copy);
  expect(captures).toEqual(["## Entire output\n世界\n"]);
  expect(store.getSnapshot().notice).toContain("Paste to verify");
  await store.copyReviewOutput(client, () => false);
  expect(store.getSnapshot().notice).toContain("unavailable");
  store.markClosed();
});
test("late navigation, oversized output and missing stored scope never reach the clipboard", async () => {
  const client = new BackendClient(new FakeBackendTransport({ repositories, targets, delayMs: 5 }));
  const store = new ShellStore();
  let copies = 0;
  const copy = () => {
    copies++;
    return true;
  };
  await store.connect(client);
  await store.copyReviewOutput(client, copy);
  store.move(1);
  const pending = store.copyReviewOutput(client, copy);
  store.move(-1);
  await pending;
  expect(copies).toBe(0);
  store.move(1);
  await store.copyReviewOutput(
    new BackendClient(new FakeBackendTransport({ reviewOutput: "世界".repeat(30000) })),
    copy,
  );
  expect(copies).toBe(0);
  expect(store.getSnapshot().notice).toContain("size limit");
  store.markClosed();
});
