import { expect, test } from "bun:test";
import { createTestRenderer, KeyCodes } from "@opentui/core/testing";
import { ProviderStore } from "../src/app/providerStore";
import { ShellStore } from "../src/app/store";
import { mountApp } from "../src/app/ui";
import { BackendClient } from "../src/transport/backendClient";
import {
  FakeProviderTransport,
  providerFixtureRepositories,
} from "../src/transport/fakeProviderTransport";

const settle = async () => {
  for (let i = 0; i < 12; i++) await Bun.sleep(0);
};
const repositories = providerFixtureRepositories.map((repository) => ({
  ...repository,
  localPath: null,
}));
test("approval preview is read-only; repeated confirmation sends once and uncertainty recovers by reads", async () => {
  const transport = new FakeProviderTransport();
  transport.approvalFault = "uncertain";
  const client = new BackendClient(transport),
    store = new ProviderStore();
  store.load(client, repositories, []);
  await settle();
  try {
    await store.previewApproval(client);
    expect(store.getSnapshot().approvalPreview?.proof.actor.accountId).toBe("owned-reviewer");
    expect(transport.calls.filter((c) => c.method.endsWith("confirm"))).toHaveLength(0);
    await Promise.all([store.confirmApproval(client), store.confirmApproval(client)]);
    expect(transport.calls.filter((c) => c.method.endsWith("confirm"))).toHaveLength(1);
    expect(store.getSnapshot().approvalResult?.entry?.state).toBe("uncertain");
    await store.approvalStatus(client, true);
    expect(transport.calls.at(-1)?.method).toBe("provider.approval.reconcile");
    store.closeApproval();
    await store.previewApproval(client);
    expect(store.getSnapshot().approvalPreview?.approvalId).toBeNull();
    await store.confirmApproval(client);
    expect(transport.calls.filter((c) => c.method.endsWith("confirm"))).toHaveLength(1);
  } finally {
    store.dispose();
    client.close();
  }
});
test("canceled, moved, expired and late approval previews never remain confirmable", async () => {
  const transport = new FakeProviderTransport(),
    client = new BackendClient(transport),
    store = new ProviderStore();
  store.load(client, repositories, []);
  await settle();
  let release!: () => void;
  try {
    transport.gate = new Promise<void>((resolve) => {
      release = resolve;
    });
    const pending = store.previewApproval(client);
    store.closeApproval();
    transport.gate = null;
    release();
    await pending;
    expect(store.getSnapshot().approvalOpen).toBe(false);
    expect(store.getSnapshot().approvalPreview).toBeNull();
    await store.previewApproval(client);
    store.move(1);
    await store.confirmApproval(client);
    expect(transport.calls.filter((c) => c.method.endsWith("confirm"))).toHaveLength(0);
    await store.previewApproval(client);
    const original = transport.request.bind(transport);
    transport.request = async (method, params) => {
      const value = await original(method, params);
      return method.endsWith("preview") ? { ...value, expiresAt: 1 } : value;
    };
    store.closeApproval();
    await store.previewApproval(client);
    await store.confirmApproval(client);
    expect(store.getSnapshot().approvalError).toContain("expired");
    expect(transport.calls.filter((c) => c.method.endsWith("confirm"))).toHaveLength(0);
  } finally {
    store.dispose();
    client.close();
  }
});
test("client rejects foreign scope, actor and token assertions before exposing confirmable state", async () => {
  const transport = new FakeProviderTransport(),
    client = new BackendClient(transport);
  const request = { repository: providerFixtureRepositories[0]!, prId: 7 };
  const original = transport.request.bind(transport);
  for (const fault of ["scope", "actor", "token", "extra"]) {
    transport.request = async (method, params) => {
      const value = await original(method, params);
      const proof = value.proof as Record<string, unknown>;
      if (fault === "scope")
        return { ...value, proof: { ...proof, request: { ...request, prId: 8 } } };
      if (fault === "actor")
        return {
          ...value,
          entry: {
            formatVersion: 1,
            proof: { ...proof, actor: { accountId: "foreign", displayName: "Foreign" } },
            state: "rejected",
            remoteReviewId: null,
            updatedAt: 1,
            attempt: 1,
          },
        };
      if (fault === "token") return { ...value, proof: { ...proof, alreadyApproved: true } };
      return { ...value, approved: true };
    };
    await expect(client.providerApprovalPreview(request)).rejects.toThrow();
  }
  client.close();
});
for (const provider of ["github", "bitbucket"] as const)
  for (const width of [120, 50])
    test(`${provider} native approval keys require preview and ignore pasted confirmation at ${width} columns`, async () => {
      const transport = new FakeProviderTransport(),
        client = new BackendClient(transport),
        store = new ShellStore();
      await store.connect(client);
      const renderer = await createTestRenderer({ width, height: width === 50 ? 18 : 30 });
      const ui = mountApp(renderer.renderer, store, client);
      const render = async () => {
        for (let i = 0; i < 8; i++) {
          await Bun.sleep(0);
          await renderer.renderOnce();
        }
      };
      try {
        renderer.mockInput.pressKey(KeyCodes.F2);
        await render();
        if (provider === "bitbucket") {
          for (let i = 0; i < 4; i++) renderer.mockInput.pressArrow("down");
          await render();
        }
        await renderer.mockInput.pasteBracketedText("ay\ny\n");
        await render();
        expect(
          transport.calls.filter((c) => c.method.startsWith("provider.approval")),
        ).toHaveLength(0);
        renderer.mockInput.pressKey("a");
        await render();
        expect(renderer.captureCharFrame()).toContain("APPROVE PULL REQUEST");
        expect(renderer.captureCharFrame()).toContain("Owned reviewer");
        expect(renderer.captureCharFrame()).toContain("Head:");
        await renderer.mockInput.pasteBracketedText("y\ny\n");
        await render();
        expect(transport.calls.filter((c) => c.method.endsWith("confirm"))).toHaveLength(0);
        renderer.mockInput.pressKey("y");
        renderer.mockInput.pressKey("y");
        await render();
        expect(transport.calls.filter((c) => c.method.endsWith("confirm"))).toHaveLength(1);
        for (let i = 0; i < 8; i++) renderer.mockInput.pressArrow("down");
        await render();
        expect(renderer.captureCharFrame()).toContain("captured commit");
        expect(
          transport.calls.find((c) => c.method.endsWith("confirm"))?.params.request,
        ).toMatchObject({ repository: { provider }, prId: 7 });
        renderer.mockInput.pressKey("r");
        await render();
        expect(transport.calls.at(-1)?.method).toBe("provider.approval.reconcile");
        renderer.mockInput.pressEscape();
        await Bun.sleep(60);
        await render();
        expect(renderer.captureCharFrame()).toContain("PULL REQUESTS");
      } finally {
        ui.unmount();
        store.cancelSyntax();
        client.close();
        renderer.renderer.destroy();
      }
    });
