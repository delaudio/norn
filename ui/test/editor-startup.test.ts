import { expect, test } from "bun:test";
import { ShellStore } from "../src/app/store";
import { BackendClient } from "../src/transport/backendClient";
import { FakeBackendTransport } from "../src/transport/fakeTransport";

test("workspace stays connecting until initial snapshot reads finish", async () => {
  let release!: () => void;
  const gate = new Promise<void>((resolve) => {
    release = resolve;
  });
  let reading = false;
  class StartupFixture extends FakeBackendTransport {
    override async request(method: string, params: Record<string, unknown>) {
      if (method === "review.files") {
        reading = true;
        await gate;
      }
      return super.request(method, params);
    }
  }
  const fixture = new StartupFixture({
    repositories: [{ provider: "github", workspace: "owned", repo: "editor", localPath: null }],
    targets: [
      {
        provider: "github",
        workspace: "owned",
        repo: "editor",
        prId: 1,
        runId: "owned-run",
        title: "Owned startup",
        status: "succeeded",
      },
    ],
  });
  const client = new BackendClient(fixture),
    store = new ShellStore();
  try {
    const connecting = store.connect(client);
    const deadline = Date.now() + 1000;
    while (!reading) {
      if (Date.now() > deadline) throw new Error("Initial snapshot was not requested");
      await Bun.sleep(0);
    }
    expect(store.getSnapshot().status).toBe("connecting");
    release();
    await connecting;
    expect(store.getSnapshot().status).toBe("ready");
  } finally {
    release();
    store.cancelSyntax();
    await client.close();
  }
});
