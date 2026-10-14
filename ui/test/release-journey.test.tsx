import { expect, test } from "bun:test";
import { createTestRenderer } from "@opentui/core/testing";
import type { WorkspaceDraft } from "../../src/protocol/generated/protocol";
import { ShellStore } from "../src/app/store";
import { mountApp } from "../src/app/ui";
import { BackendClient } from "../src/transport/backendClient";
import { FakePublicationTransport } from "../src/transport/fakePublicationTransport";

// One native UI journey across the same scope. Provider sending is simulated;
// real Rust persistence/subprocesses are independently exercised by native CI.
const options = {
  repositories: [{ provider: "github", workspace: "synthetic", repo: "review", localPath: null }],
  targets: [
    {
      provider: "github",
      workspace: "synthetic",
      repo: "review",
      prId: 7,
      runId: "owned-run",
      title: "Owned review",
      status: "succeeded",
    },
  ],
  files: [
    { path: "src/example.ts", status: "modified", additions: 1, deletions: 1, oldPath: null },
  ],
  diffText: "@@ -12 +12 @@\n-old owned line\n+new owned line\n",
  findings: [
    {
      id: "owned-finding",
      title: "Validate owned input",
      severity: "high",
      summary: "Owned finding body",
      anchor: { path: "src/example.ts", startLine: 12, endLine: null, side: "old" },
    },
  ],
  threads: [
    {
      id: "owned-thread",
      title: "Owned conversation",
      latestRunId: "owned-run",
      messages: [
        {
          id: "owned-message",
          role: "assistant",
          content: "Owned conversation output",
          truncated: false,
        },
      ],
      truncated: false,
    },
  ],
};
const key = "drafts.github:synthetic/review/7";
async function open(drafts: WorkspaceDraft[]) {
  const transport = new FakePublicationTransport(drafts, options);
  const client = new BackendClient(transport);
  const store = new ShellStore();
  await store.connect(client);
  await store.selectPicker(1);
  const renderer = await createTestRenderer({ width: 120, height: 40 });
  const app = mountApp(renderer.renderer, store, client);
  const waitText = async (text: string) => {
    const deadline = Date.now() + 2000;
    while (!renderer.captureCharFrame().includes(text)) {
      if (Date.now() > deadline)
        throw new Error(`Journey missing ${text}\n${renderer.captureCharFrame()}`);
      await Bun.sleep(1);
      await renderer.renderOnce();
    }
    // Flush the native input subscription after the component has painted.
    await Bun.sleep(0);
    await renderer.renderOnce();
  };
  return {
    transport,
    client,
    store,
    renderer,
    app,
    waitText,
    async stop() {
      app.unmount();
      store.cancelSyntax();
      renderer.renderer.destroy();
      await client.close();
    },
  };
}

test("native review → diff → finding → conversation → staged draft → restart → explicit simulated publication", async () => {
  const first = await open([]);
  let saved: WorkspaceDraft[] = [];
  try {
    await first.waitText("new owned line");
    expect(first.store.getSnapshot().findings[0]?.id).toBe("owned-finding");
    first.renderer.mockInput.pressKey("t");
    await first.waitText("Owned conversation output");
    first.renderer.mockInput.pressKey("t");
    first.renderer.mockInput.pressKey("i");
    await first.waitText("Stage finding");
    first.renderer.mockInput.pressEnter();
    await first.waitText("Owned finding body");
    expect(first.transport.calls).not.toContain("publication.apply");
    first.renderer.mockInput.pressKey("y");
    await first.waitText("Draft staging confirmed locally");
    const record = await first.client.readWorkspace(key);
    saved = record.payload.value as WorkspaceDraft[];
    expect(saved).toHaveLength(1);
    expect(saved[0]).toMatchObject({
      path: "src/example.ts",
      from: 12,
      source: "aiFinding",
      findingRef: { reviewRunId: "owned-run", findingId: "owned-finding" },
      reviewHeadSha: "b".repeat(40),
      reviewBaseSha: "a".repeat(40),
      publicationMode: "inline",
    });
    expect(first.transport.calls).not.toContain("publication.apply");
  } finally {
    await first.stop();
  }

  const restarted = await open(saved);
  try {
    await restarted.waitText("new owned line");
    restarted.renderer.mockInput.pressKey("d");
    await restarted.waitText("1 staged comments");
    await restarted.renderer.mockInput.pasteBracketedText("p\ny\n");
    expect(restarted.transport.calls).not.toContain("publication.apply");
    restarted.renderer.mockInput.pressKey("p");
    await restarted.waitText("Publish comments");
    restarted.renderer.mockInput.pressKey(" ");
    await restarted.waitText("[x]");
    restarted.renderer.mockInput.pressEnter();
    await restarted.waitText("Exact preview");
    expect(restarted.transport.calls).not.toContain("publication.apply");
    restarted.renderer.mockInput.pressKey("y");
    await restarted.waitText("published");
    expect(
      restarted.transport.calls.filter((method) => method === "publication.apply"),
    ).toHaveLength(1);
    expect((await restarted.client.readWorkspace(key)).payload.value).toEqual([]);
    expect(restarted.transport.calls).not.toContain("auth.set");
  } finally {
    await restarted.stop();
  }
});
