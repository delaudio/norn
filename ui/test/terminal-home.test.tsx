import { expect, test } from "bun:test";
import type { ScrollBoxRenderable } from "@opentui/core";
import { createTestRenderer, KeyCodes } from "@opentui/core/testing";
import { ShellStore } from "../src/app/store";
import { mountApp } from "../src/app/ui";
import { BackendClient } from "../src/transport/backendClient";
import { CurrentTransport } from "./fixtures/current-provider-backend";

async function fixture(provider: "github" | "bitbucket", width = 120) {
  const transport = new CurrentTransport(provider);
  const client = new BackendClient(transport);
  const store = new ShellStore();
  await store.connect(client, "current-repo");
  const renderer = await createTestRenderer({ width, height: 30 });
  const ui = mountApp(renderer.renderer, store, client);
  const unsubscribe = client.onOperationEvent((event) => store.applyEvent(event));
  const settle = async () => {
    for (let i = 0; i < 15; i++) {
      await Bun.sleep(1);
      await renderer.renderOnce();
    }
  };
  const key = async (value: string) => {
    if (value === "return") renderer.mockInput.pressEnter();
    else if (value === "tab") renderer.mockInput.pressTab();
    else renderer.mockInput.pressKey(value);
    await settle();
    const deadline = Date.now() + 2000;
    while (
      (store.getSnapshot().snapshotLoading ||
        store.getSnapshot().diffLoading ||
        store.getSnapshot().localForm?.loading ||
        transport.activeReads > 0) &&
      Date.now() < deadline
    ) {
      await Bun.sleep(1);
      await renderer.renderOnce();
    }
    await renderer.renderOnce();
  };
  const cleanup = async () => {
    unsubscribe();
    ui.unmount();
    renderer.renderer.destroy();
    await client.close();
  };
  await settle();
  return { transport, client, store, renderer, key, settle, cleanup };
}

for (const provider of ["github", "bitbucket"] as const) {
  for (const width of [120, 50]) {
    test(`terminal ${provider} home has filters, PRs and AI review at ${width} columns`, async () => {
      const app = await fixture(provider, width);
      try {
        const frame = app.renderer.captureCharFrame();
        for (const label of [
          "PR DESCRIPTION",
          "Open",
          "Draft",
          "Merged",
          "Local",
          "PULL REQUESTS",
          "AI REVIEW",
        ])
          expect(frame).toContain(label);
        const lines = frame.split("\n");
        const menu = lines.findIndex(
          (line) => line.includes("Open") && line.includes("Draft") && line.includes("Local"),
        );
        const reviewHeader = lines.findIndex((line) => line.includes("AI REVIEW"));
        const listHeader = lines.findIndex((line) => line.includes("PULL REQUESTS"));
        const descriptionHeader = lines.findIndex((line) => line.includes("PR DESCRIPTION"));
        expect(menu).toBeGreaterThanOrEqual(0);
        expect(menu).toBeLessThan(reviewHeader);
        expect(reviewHeader).toBe(listHeader);
        expect(lines[reviewHeader]!.indexOf("AI REVIEW")).toBeLessThan(
          lines[listHeader]!.indexOf("PULL REQUESTS"),
        );
        expect(descriptionHeader).toBeGreaterThan(listHeader);
        expect(lines[descriptionHeader]!.indexOf("PR DESCRIPTION")).toBe(
          lines[listHeader]!.indexOf("PULL REQUESTS"),
        );
        expect(frame).not.toContain("PR FILTER");
        expect(frame).not.toContain("TARGETS");
        expect(app.store.getSnapshot().terminalHomeOpen).toBe(true);
        await app.key("f");
        expect(app.store.getSnapshot().terminalFilter).toBe("DRAFT");
        expect(app.renderer.captureCharFrame()).toContain(`${provider} change 8`);
        expect(
          app.transport.calls.filter((call) => call.method === "provider.prs").at(-1)?.params
            .filter,
        ).toBe("DRAFT");
        await app.key("f");
        await app.key("f");
        expect(app.store.getSnapshot().terminalFilter).toBe("LOCAL");
        expect(app.renderer.captureCharFrame()).toContain("LOCAL CHANGES");
        expect(
          app.transport.calls.some((call) =>
            ["review.start", "local.snapshot"].includes(call.method),
          ),
        ).toBe(false);
        await app.key("f");
        expect(app.store.getSnapshot().terminalFilter).toBe("OPEN");
      } finally {
        await app.cleanup();
      }
    });
  }
  test(`g then b opens the authenticated ${provider} diff viewer and keeps home selection`, async () => {
    const app = await fixture(provider);
    try {
      await app.key("g");
      expect(app.renderer.captureCharFrame()).toContain("Open diff");
      expect(app.renderer.captureCharFrame()).toContain("b · Browser diff viewer");
      expect(app.transport.calls.some((call) => call.method === "provider.snapshot")).toBe(false);
      await app.key("b");
      const browser = app.transport.calls.filter(
        (call) => call.method === "provider.snapshot.browser",
      );
      expect(browser).toHaveLength(1);
      expect(browser[0]?.params).toMatchObject({
        snapshot: { repository: { provider, workspace: "synthetic", repo: "review" }, prId: 7 },
      });
      expect(app.transport.calls.some((call) => call.method === "provider.browser")).toBe(false);
      expect(app.store.getSnapshot().terminalHomeOpen).toBe(true);
      expect(app.store.getSnapshot().terminalDiffPrompt).toBe(false);
      expect(app.store.getSnapshot().notice).toContain("Browser diff:");
      expect(app.store.getSnapshot().error).toBeNull();
    } finally {
      await app.cleanup();
    }
  });
  test(`g then g opens native ${provider} diff and g returns to the same filter and PR`, async () => {
    const app = await fixture(provider);
    try {
      await app.key("f");
      await app.key("g");
      await app.key("g");
      expect(app.store.getSnapshot().terminalHomeOpen).toBe(false);
      expect(app.store.getSnapshot().files[0]?.path).toBe("readme.ts");
      expect(app.store.getSnapshot().diff?.text).toContain(`${provider} PR 8`);
      await app.key("g");
      expect(app.store.getSnapshot().terminalHomeOpen).toBe(true);
      expect(app.store.getSnapshot().terminalFilter).toBe("DRAFT");
      expect(app.renderer.captureCharFrame()).toContain(`#8 ${provider} change 8`);
      expect(app.transport.calls.some((call) => call.method === "review.start")).toBe(false);
    } finally {
      await app.cleanup();
    }
  });
}

test("Settings is a modal over the terminal home; paste cannot launch diff or review", async () => {
  const app = await fixture("github");
  try {
    await app.key("tab");
    expect(app.store.getSnapshot().terminalHomeFocus).toBe("description");
    await app.key("tab");
    expect(app.store.getSnapshot().terminalHomeFocus).toBe("review");
    await app.key("s");
    expect(app.store.getSnapshot().settingsOpen).toBe(true);
    const frame = app.renderer.captureCharFrame();
    expect(frame).toContain("AI REVIEW");
    expect(frame).toContain("Settings");
    for (const unwanted of [
      "Jira URL",
      "Repo 1",
      "Codex model",
      "OpenCode model",
      "Automatic PR sync",
      "add repo",
    ])
      expect(frame).not.toContain(unwanted);
    expect(frame).toContain("Claude model");
    await app.renderer.mockInput.pasteBracketedText("ggbaq");
    await app.settle();
    expect(
      app.transport.calls.some((call) =>
        ["provider.snapshot.browser", "review.start", "local.snapshot"].includes(call.method),
      ),
    ).toBe(false);
    app.renderer.mockInput.pressEscape();
    await Bun.sleep(30);
    await app.settle();
    expect(app.store.getSnapshot().settingsOpen).toBe(false);
    expect(app.store.getSnapshot().terminalHomeFocus).toBe("review");
    expect(app.store.getSnapshot().terminalHomeOpen).toBe(true);
    expect(app.renderer.captureCharFrame()).toContain("AI REVIEW");
  } finally {
    await app.cleanup();
  }
});

test("Enter loads the selected review; only a explicitly starts an agent", async () => {
  const app = await fixture("github");
  try {
    await app.key("return");
    expect(app.store.getSnapshot().terminalHomeOpen).toBe(true);
    expect(app.transport.calls.some((call) => call.method === "review.history")).toBe(true);
    expect(app.transport.calls.some((call) => call.method === "review.start")).toBe(false);
    await app.key("a");
    expect(app.transport.calls.filter((call) => call.method === "review.start")).toHaveLength(1);
    expect(app.store.getSnapshot().operation?.targetLabel).toContain("#7");
    const deadline = Date.now() + 2000;
    while (
      !app.store.getSnapshot().operation?.logs.some((log) => log.text === "collected file") &&
      Date.now() < deadline
    ) {
      await Bun.sleep(1);
      await app.settle();
    }
    expect(app.renderer.captureCharFrame()).toContain("collected file");
  } finally {
    await app.cleanup();
  }
});

test("a pasted shortcut sequence cannot act on the home or diff-choice modal", async () => {
  const app = await fixture("github");
  try {
    await app.renderer.mockInput.pasteBracketedText("ggba");
    await app.settle();
    expect(app.store.getSnapshot().terminalDiffPrompt).toBe(false);
    await app.key("g");
    await app.renderer.mockInput.pasteBracketedText("b");
    await app.settle();
    expect(app.store.getSnapshot().terminalDiffPrompt).toBe(true);
    app.renderer.mockInput.pressEscape();
    await Bun.sleep(30);
    await app.settle();
    expect(app.store.getSnapshot().terminalDiffPrompt).toBe(false);
    expect(
      app.transport.calls.some((call) =>
        ["review.start", "provider.snapshot", "provider.snapshot.browser"].includes(call.method),
      ),
    ).toBe(false);
  } finally {
    await app.cleanup();
  }
});

test("Local filter is visible without capture; explicit configuration captures tracked layers", async () => {
  const app = await fixture("github");
  try {
    await app.key("f");
    await app.key("f");
    await app.key("f");
    expect(app.transport.calls.some((call) => call.method === "local.snapshot")).toBe(false);
    await app.key("return");
    expect(app.store.getSnapshot().localForm).not.toBeNull();
    expect(app.transport.calls.some((call) => call.method === "local.snapshot")).toBe(false);
    await app.key("return");
    expect(app.store.getSnapshot().localForm).toBeNull();
    expect(app.store.getSnapshot().localInfo?.snapshot.repository).toEqual({
      provider: "github",
      workspace: "synthetic",
      repo: "review",
    });
    expect(app.store.getSnapshot().terminalHomeOpen).toBe(true);
    expect(app.transport.calls.filter((call) => call.method === "local.snapshot")).toHaveLength(1);
    expect(app.transport.calls.some((call) => call.method === "review.start")).toBe(false);
  } finally {
    await app.cleanup();
  }
});

test("the AI panel reads an actual scoped saved run rather than global history", async () => {
  const app = await fixture("bitbucket");
  const request = app.transport.request.bind(app.transport);
  app.transport.request = async (method, params) => {
    if (method === "review.history") {
      app.transport.calls.push({ method, params });
      return {
        runs: [
          {
            runId: "owned-saved",
            createdAt: "2026-10-13T00:00:00Z",
            status: "completed",
            turnKind: "initial",
            threadId: null,
            findingsCount: 0,
          },
        ],
      };
    }
    if (method === "review.output") {
      app.transport.calls.push({ method, params });
      return { target: params.target, runId: "owned-saved", text: "Owned saved AI review content" };
    }
    return request(method, params);
  };
  try {
    await app.key("return");
    expect(app.renderer.captureCharFrame()).toContain("Owned saved AI review content");
    expect(
      app.transport.calls.find((call) => call.method === "review.output")?.params,
    ).toMatchObject({
      target: {
        provider: "bitbucket",
        workspace: "synthetic",
        repo: "review",
        prId: 7,
        runId: "owned-saved",
      },
    });
    expect(
      app.transport.calls.some((call) => ["review.jobs", "review.start"].includes(call.method)),
    ).toBe(false);
  } finally {
    await app.cleanup();
  }
});

test("a failed PR capture exposes the cause and Enter retries the same selected target", async () => {
  const app = await fixture("github");
  try {
    app.transport.fault = "snapshot";
    await app.key("return");
    expect(app.store.getSnapshot().error).not.toBeNull();
    expect(app.renderer.captureCharFrame()).toContain(app.store.getSnapshot().error!);
    app.transport.fault = null;
    await app.key("return");
    expect(app.store.getSnapshot().error).toBeNull();
    expect(app.store.getSnapshot().files[0]?.path).toBe("readme.ts");
    expect(app.store.getSnapshot().terminalHomeOpen).toBe(true);
    expect(app.transport.calls.some((call) => call.method === "review.start")).toBe(false);
  } finally {
    await app.cleanup();
  }
});

test("missing stored output does not hide valid scoped findings", async () => {
  const app = await fixture("github");
  const request = app.transport.request.bind(app.transport);
  app.transport.request = async (method, params) => {
    if (method === "review.history")
      return {
        runs: [
          {
            runId: "owned-partial",
            createdAt: "2026-10-13T00:00:00Z",
            status: "completed",
            turnKind: "initial",
            findingsCount: 1,
          },
        ],
      };
    if (method === "review.output") throw new Error("Owned stored output exceeds the limit");
    if (method === "review.findings")
      return {
        findings: [
          {
            id: "owned-finding",
            title: "Owned retained finding",
            severity: "high",
            summary: "Scoped finding remains readable",
            anchor: null,
          },
        ],
      };
    return request(method, params);
  };
  try {
    await app.key("return");
    expect(app.store.getSnapshot().terminalReview?.output).toBeNull();
    expect(app.renderer.captureCharFrame()).toContain("Owned retained finding");
    expect(app.renderer.captureCharFrame()).toContain("partially unavailable");
    expect(app.store.getSnapshot().terminalReview?.runId).toBe("owned-partial");
  } finally {
    await app.cleanup();
  }
});

test("default current-repository home routes F8 using Git identity without loading a PR", async () => {
  const app = await fixture("bitbucket");
  try {
    expect(app.store.getSnapshot().picker).toHaveLength(0);
    app.renderer.mockInput.pressKey(KeyCodes.F8);
    await app.settle();
    expect(app.store.getSnapshot().explorerOpen).toBe(true);
    const inventory = app.transport.calls.find((call) => call.method === "repository.explore.info");
    expect(inventory?.params.repository).toEqual({
      provider: "bitbucket",
      workspace: "synthetic",
      repo: "review",
    });
    expect(
      app.transport.calls.some((call) =>
        ["provider.snapshot", "review.start", "local.snapshot"].includes(call.method),
      ),
    ).toBe(false);
  } finally {
    await app.cleanup();
  }
});

for (const provider of ["github", "bitbucket"] as const) {
  for (const width of [120, 50]) {
    test(`the selected ${provider} PR description loads independently at ${width} columns`, async () => {
      const app = await fixture(provider, width);
      try {
        expect(app.renderer.captureCharFrame()).toMatch(/Actual synthetic[│\s]+description/);
        const detail = app.transport.calls.filter((call) => call.method === "provider.pr.detail");
        expect(detail).toHaveLength(1);
        expect(detail[0]?.params).toEqual({
          repository: { provider, workspace: "synthetic", repo: "review" },
          prId: 7,
        });
        expect(app.transport.calls.some((call) => call.method === "provider.snapshot")).toBe(false);
        app.transport.fault = "snapshot";
        await app.key("return");
        expect(app.store.getSnapshot().error).not.toBeNull();
        expect(app.renderer.captureCharFrame()).toMatch(/Actual synthetic[│\s]+description/);
        expect(app.store.getSnapshot().terminalHomeOpen).toBe(true);
      } finally {
        await app.cleanup();
      }
    });
  }
}

test("a late description cannot replace the next selected PR and filters hide stale metadata", async () => {
  const app = await fixture("github");
  const request = app.transport.request.bind(app.transport);
  let release!: () => void;
  const gate = new Promise<void>((resolve) => {
    release = resolve;
  });
  app.transport.request = async (method, params) => {
    const result = await request(method, params);
    if (method === "provider.pr.detail") {
      if (params.prId === 7) await gate;
      return {
        ...result,
        detail: {
          ...(result.detail as Record<string, unknown>),
          descriptionRaw:
            params.prId === 7 ? "Old delayed PR description" : "Current PR eight description",
        },
      };
    }
    return result;
  };
  try {
    // Enter explicitly reloads details for 7, without tying them to its diff.
    await app.key("return");
    await app.key("j");
    await app.settle();
    expect(app.renderer.captureCharFrame()).toContain("Current PR eight description");
    release();
    await app.settle();
    expect(app.renderer.captureCharFrame()).toContain("Current PR eight description");
    expect(app.renderer.captureCharFrame()).not.toContain("Old delayed PR description");
    await app.key("f");
    await app.key("f");
    await app.key("f");
    expect(app.renderer.captureCharFrame()).toContain("LOCAL CHANGES");
    expect(app.renderer.captureCharFrame()).not.toContain("Current PR eight description");
  } finally {
    release();
    await app.cleanup();
  }
});

for (const provider of ["github", "bitbucket"] as const) {
  test(`a retries ${provider} evidence before starting exactly one review`, async () => {
    const app = await fixture(provider);
    try {
      app.transport.fault = "snapshot";
      await app.key("a");
      expect(app.store.getSnapshot().error).not.toBeNull();
      expect(app.transport.calls.some((call) => call.method === "review.start")).toBe(false);
      app.transport.fault = null;
      await app.key("a");
      const starts = app.transport.calls.filter((call) => call.method === "review.start");
      expect(starts).toHaveLength(1);
      expect(starts[0]?.params).toMatchObject({
        target: { provider, workspace: "synthetic", repo: "review", prId: 7 },
        snapshot: { repository: { provider, workspace: "synthetic", repo: "review" }, prId: 7 },
      });
    } finally {
      await app.cleanup();
    }
  });
}

test("description and AI output have independent keyboard scroll and focus", async () => {
  const app = await fixture("github");
  const request = app.transport.request.bind(app.transport);
  app.transport.request = async (method, params) => {
    if (method === "review.history")
      return {
        runs: [
          {
            runId: "owned-saved",
            createdAt: "2026-10-13T00:00:00Z",
            status: "completed",
            turnKind: "initial",
            threadId: null,
            findingsCount: 0,
          },
        ],
      };
    if (method === "review.output")
      return {
        target: params.target,
        runId: "owned-saved",
        text: Array.from({ length: 60 }, (_, i) => `AI output line ${i}`).join("\n"),
      };
    const result = await request(method, params);
    if (method === "provider.pr.detail")
      return {
        ...result,
        detail: {
          ...(result.detail as Record<string, unknown>),
          descriptionRaw: Array.from({ length: 60 }, (_, i) => `Description line ${i}`).join("\n"),
        },
      };
    return result;
  };
  try {
    await app.key("return");
    const review = app.renderer.renderer.root.findDescendantById(
      "home-review-scroll",
    ) as ScrollBoxRenderable;
    const description = app.renderer.renderer.root.findDescendantById(
      "home-description-scroll",
    ) as ScrollBoxRenderable;
    expect(review.scrollTop).toBe(0);
    await app.key("tab");
    expect(app.store.getSnapshot().terminalHomeFocus).toBe("description");
    await app.key("\u001b[6~");
    expect(description.scrollTop).toBeGreaterThan(0);
    expect(review.scrollTop).toBe(0);
    const descriptionPosition = description.scrollTop;
    await app.key("tab");
    await app.key("\u001b[6~");
    expect(review.scrollTop).toBeGreaterThan(0);
    expect(description.scrollTop).toBe(descriptionPosition);
    await app.key("tab");
    expect(app.store.getSnapshot().terminalHomeFocus).toBe("filters");
    await app.key("tab");
    expect(app.store.getSnapshot().terminalHomeFocus).toBe("list");
    await app.key("j");
    expect(description.scrollTop).toBe(0);
    expect(review.scrollTop).toBe(0);
    app.renderer.mockInput.pressTab({ shift: true });
    await app.settle();
    expect(app.store.getSnapshot().terminalHomeFocus).toBe("filters");
    app.renderer.mockInput.pressTab({ shift: true });
    await app.settle();
    expect(app.store.getSnapshot().terminalHomeFocus).toBe("review");
  } finally {
    await app.cleanup();
  }
});
