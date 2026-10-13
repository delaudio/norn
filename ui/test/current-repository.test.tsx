import { expect, test } from "bun:test";
import { createTestRenderer } from "@opentui/core/testing";
import { launchMode } from "../src/app/launchMode";
import { ShellStore } from "../src/app/store";
import { mountApp } from "../src/app/ui";
import { BackendClient } from "../src/transport/backendClient";
import { FakeProviderTransport } from "../src/transport/fakeProviderTransport";
import { CurrentTransport } from "./fixtures/current-provider-backend";

test("production launch defaults to current repo and rejects ambiguous arguments", () => {
  expect(launchMode([])).toBe("current-repo");
  expect(launchMode(["--current-repo"])).toBe("current-repo");
  expect(launchMode(["--workspace"])).toBe("workspace");
  expect(() => launchMode(["--workspace", "--current-repo"])).toThrow("Choose either");
  expect(() => launchMode(["--unknown"])).toThrow("Unknown argument");
});

for (const provider of ["github", "bitbucket"] as const) {
  for (const [width, height] of [
    [120, 30],
    [50, 15],
  ]) {
    test(`startup shows only current ${provider} PRs at ${width} columns without a review or capture`, async () => {
      const transport = new CurrentTransport(provider);
      const client = new BackendClient(transport);
      const store = new ShellStore();
      await store.connect(client, launchMode([]));
      const renderer = await createTestRenderer({ width, height });
      const ui = mountApp(renderer.renderer, store, client);
      const settle = async () => {
        for (let i = 0; i < 12; i++) {
          await Bun.sleep(0);
          await renderer.renderOnce();
        }
      };
      try {
        await settle();
        const frame = renderer.captureCharFrame();
        expect(frame).toContain("PULL REQUESTS");
        expect(frame).toContain(`${provider} change 7`);
        expect(frame).not.toContain(`${provider === "github" ? "bitbucket" : "github"} change 7`);
        expect(store.getSnapshot().picker).toHaveLength(0);
        expect(
          transport.calls.some((call) =>
            [
              "repository.list",
              "review.targets",
              "workspace.scopes",
              "local.snapshot",
              "review.start",
            ].includes(call.method),
          ),
        ).toBe(false);
        expect(
          transport.calls
            .filter((call) => call.method === "provider.prs")
            .every(
              (call) => (call.params.repository as { provider: string }).provider === provider,
            ),
        ).toBe(true);
        renderer.mockInput.pressKey(" ");
        await settle();
        expect(store.getSnapshot().providerOpen).toBe(false);
        expect(store.getSnapshot().files[0]?.path).toBe("readme.ts");
        expect(store.getSnapshot().providerPreview).toBe(true);
        expect(store.getSnapshot().error).toBeNull();
        expect(
          transport.calls.some((call) => ["review.start", "local.snapshot"].includes(call.method)),
        ).toBe(false);
      } finally {
        ui.unmount();
        renderer.renderer.destroy();
        await client.close();
      }
    });
  }
  test(`settings reload retains the current ${provider} scope without capturing it`, async () => {
    const transport = new CurrentTransport(provider);
    const client = new BackendClient(transport);
    const store = new ShellStore();
    await store.connect(client, "current-repo");
    await store.applySettings(client, (await client.settings()).settings);
    expect(store.getSnapshot().repositories.map((repo) => repo.provider)).toEqual([provider]);
    expect(store.getSnapshot().picker.every((entry) => entry.provider === provider)).toBe(true);
    expect(transport.calls.some((call) => call.method === "local.snapshot")).toBe(false);
    await client.close();
  });
}

test("an unconfigured current repository is read from Git without guessing from configuration", async () => {
  const transport = new CurrentTransport("bitbucket");
  const original = transport.request.bind(transport);
  transport.request = async (method, params) => {
    const result = await original(method, params);
    return method === "local.options"
      ? {
          ...result,
          repository: { provider: "bitbucket", workspace: "unconfigured", repo: "owned" },
        }
      : result;
  };
  const client = new BackendClient(transport);
  const store = new ShellStore();
  await store.connect(client, "current-repo");
  expect(store.getSnapshot().repositories).toEqual([
    { provider: "bitbucket", workspace: "unconfigured", repo: "owned", localPath: null },
  ]);
  expect(transport.calls.some((call) => call.method === "repository.list")).toBe(false);
  await client.close();
});

test("failed Git resolution stays visible and does not fall back to unrelated repositories", async () => {
  const transport = new CurrentTransport("github");
  transport.failure = true;
  const client = new BackendClient(transport);
  const store = new ShellStore();
  await store.connect(client, "current-repo");
  expect(store.getSnapshot().status).toBe("error");
  expect(store.getSnapshot().error).toContain("No supported Git remote");
  expect(store.getSnapshot().error).toContain("--workspace");
  expect(store.getSnapshot().picker).toHaveLength(0);
  expect(transport.calls.some((call) => call.method === "review.targets")).toBe(false);
  await client.close();
});

test("explicit workspace mode keeps multi-repository access", async () => {
  const transport = new FakeProviderTransport();
  const client = new BackendClient(transport);
  const store = new ShellStore();
  await store.connect(client, launchMode(["--workspace"]));
  expect(store.getSnapshot().repositories).toHaveLength(2);
  expect(store.getSnapshot().providerOpen).toBe(false);
  expect(transport.calls.some((call) => call.method === "review.targets")).toBe(true);
  await client.close();
});
