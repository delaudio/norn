import { expect, test } from "bun:test";
import { createTestRenderer, KeyCodes } from "@opentui/core/testing";
import { ShellStore } from "../src/app/store";
import { mountApp } from "../src/app/ui";
import { BackendClient } from "../src/transport/backendClient";
import { FakeProfileTransport } from "../src/transport/fakeProfileTransport";
import { FakeBackendTransport } from "../src/transport/fakeTransport";

const repositories = ["github", "bitbucket"].map((provider) => ({
  provider,
  workspace: "owned",
  repo: "same",
  localPath: null,
}));
const settle = async () => {
  for (let i = 0; i < 12; i++) await Bun.sleep(0);
};

for (const width of [120, 50])
  test(`native profile picker selects actual names/default and ignores pasted actions at ${width}`, async () => {
    const transport = new FakeProfileTransport({ repositories });
    const client = new BackendClient(transport);
    const store = new ShellStore();
    await store.connect(client);
    const renderer = await createTestRenderer({ width, height: width === 50 ? 18 : 30 });
    const app = mountApp(renderer.renderer, store, client);
    const render = async () => {
      await settle();
      await renderer.renderOnce();
    };
    try {
      renderer.mockInput.pressKey(KeyCodes.F6);
      await render();
      expect(renderer.captureCharFrame()).toContain("REVIEW PROFILES");
      expect(renderer.captureCharFrame()).toContain("Repository default (default)");
      await renderer.mockInput.pasteBracketedText("q\nx\ny\nsecurity");
      await render();
      expect(store.getSnapshot().profileCursor).toBe(0);
      expect(store.getSnapshot().selectedReviewProfile).toBeNull();
      renderer.mockInput.pressArrow("down");
      renderer.mockInput.pressArrow("down");
      renderer.mockInput.pressEnter();
      await render();
      expect(store.getSnapshot().selectedReviewProfile).toBe("security");
      expect(store.getSnapshot().profileOpen).toBe(false);
      expect(transport.calls.filter((c) => c.method === "review.start")).toHaveLength(0);
      await store.startReview(client);
      expect(transport.calls.find((c) => c.method === "review.start")?.params.profile).toBe(
        "security",
      );
    } finally {
      app.unmount();
      store.cancelSyntax();
      client.close();
      renderer.renderer.destroy();
    }
  });

test("default remains null, changed catalog resets removed selection, refresh errors and cancel do not launch", async () => {
  const transport = new FakeProfileTransport({ repositories });
  const client = new BackendClient(transport);
  const store = new ShellStore();
  try {
    await store.connect(client);
    await store.openReviewProfiles(client);
    store.moveReviewProfile(2);
    store.chooseReviewProfile();
    expect(store.getSnapshot().selectedReviewProfile).toBe("security");
    transport.profiles = ["strict"];
    transport.defaultProfile = "strict";
    await store.openReviewProfiles(client);
    expect(store.getSnapshot().selectedReviewProfile).toBeNull();
    expect(store.getSnapshot().notice).toContain("unavailable");
    store.chooseReviewProfile();
    await store.startReview(client);
    expect(transport.calls.find((c) => c.method === "review.start")?.params.profile).toBeNull();
  } finally {
    store.cancelSyntax();
    client.close();
  }
});

test("late and cancelled catalog is fenced by full provider/target generation", async () => {
  const transport = new FakeProfileTransport({ repositories });
  const client = new BackendClient(transport);
  const store = new ShellStore();
  try {
    await store.connect(client);
    let release!: () => void;
    transport.gate = new Promise<void>((resolve) => {
      release = resolve;
    });
    const pending = store.openReviewProfiles(client);
    store.dismissReviewProfiles();
    release();
    await pending;
    expect(store.getSnapshot().profileCatalog).toBeNull();
    expect(store.getSnapshot().profileOpen).toBe(false);
    transport.gate = new Promise<void>((resolve) => {
      release = resolve;
    });
    const stale = store.openReviewProfiles(client);
    store.move(1);
    release();
    await stale;
    await settle();
    expect(store.getSnapshot().profileCatalog).toBeNull();
    expect(store.getSnapshot().selectedReviewProfile).toBeNull();
    transport.gate = null;
    transport.error = "Owned policy invalid";
    await store.openReviewProfiles(client);
    expect(store.getSnapshot().profileError).toBe("Owned policy invalid");
    store.chooseReviewProfile();
    expect(store.getSnapshot().profileOpen).toBe(true);
    store.dismissReviewProfiles();
    expect(transport.calls.some((c) => c.method === "review.start")).toBe(false);
  } finally {
    store.cancelSyntax();
    client.close();
  }
});

test("malformed, cross-provider, oversized and unknown catalog fields are rejected", async () => {
  const transport = new FakeProfileTransport();
  const client = new BackendClient(transport);
  const target = {
    kind: "provider" as const,
    provider: "github" as const,
    workspace: "owned",
    repo: "same",
    prId: 7,
    runId: "saved",
  };
  const mutations = [
    (v: Record<string, unknown>) => ({ ...v, secret: "forbidden" }),
    (v: Record<string, unknown>) => ({ ...v, target: { ...target, provider: "bitbucket" } }),
    (v: Record<string, unknown>) => ({ ...v, target: { ...target, runId: "other" } }),
    (v: Record<string, unknown>) => ({ ...v, profiles: [" invalid "] }),
    (v: Record<string, unknown>) => ({ ...v, profiles: ["line\ncontrol"] }),
    (v: Record<string, unknown>) => ({
      ...v,
      profiles: Array.from({ length: 101 }, (_, i) => `p${i}`),
    }),
    (v: Record<string, unknown>) => ({ ...v, defaultProfile: "missing" }),
    (v: Record<string, unknown>) => ({ ...v, notice: 9 }),
    (v: Record<string, unknown>) => ({ ...v, defaultProfile: 9 }),
    (v: Record<string, unknown>) => ({ ...v, profiles: ["same", "same"] }),
    (v: Record<string, unknown>) => ({ ...v, notice: "x".repeat(65537) }),
  ];
  try {
    for (const mutate of mutations) {
      transport.mutate = mutate;
      await expect(client.reviewProfiles(target)).rejects.toThrow();
    }
  } finally {
    client.close();
  }
});

test("older backend retains manual F6 selection", async () => {
  const client = new BackendClient(new FakeBackendTransport({ repositories }));
  const store = new ShellStore();
  try {
    await store.connect(client);
    await store.openReviewProfiles(client);
    expect(store.getSnapshot().composerKind).toBe("profile");
    expect(store.getSnapshot().profileOpen).toBe(false);
  } finally {
    store.cancelSyntax();
    client.close();
  }
});

test("narrow catalog keeps a long Unicode selected name and controls visible", async () => {
  const transport = new FakeProfileTransport({ repositories });
  transport.profiles = Array.from({ length: 99 }, (_, i) => `profile-${i}`);
  const long = "界".repeat(128);
  transport.profiles.push(long);
  transport.defaultProfile = null;
  const client = new BackendClient(transport);
  const store = new ShellStore();
  await store.connect(client);
  await store.openReviewProfiles(client);
  store.moveReviewProfile(100);
  const renderer = await createTestRenderer({ width: 50, height: 18 });
  const app = mountApp(renderer.renderer, store, client);
  try {
    await settle();
    await renderer.renderOnce();
    const frame = renderer.captureCharFrame();
    expect(frame).toContain("Enter choose");
    expect(frame).toContain("Esc cancel");
    expect(
      frame
        .split("\n")
        .filter((line) => line.includes("界"))
        .join("")
        .replace(/[^界]/g, "").length,
    ).toBeGreaterThanOrEqual(128);
    store.chooseReviewProfile();
    expect(store.getSnapshot().selectedReviewProfile).toBe(long);
  } finally {
    app.unmount();
    store.cancelSyntax();
    client.close();
    renderer.renderer.destroy();
  }
});
