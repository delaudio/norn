import { expect, test } from "bun:test";
import { createTestRenderer } from "@opentui/core/testing";
import { ShellStore } from "../src/app/store";
import { parseDiffSyntax } from "../src/app/syntaxTokens";
import { mountApp } from "../src/app/ui";
import { BackendClient } from "../src/transport/backendClient";
import { FakeBackendTransport } from "../src/transport/fakeTransport";

for (const split of [false, true])
  test(`native ${split ? "split" : "unified"} syntax preserves source and colored spans`, async () => {
    const renderer = await createTestRenderer({ width: 140, height: 30 });
    const store = new ShellStore({
      highlight: async (path, diff) => parseDiffSyntax(path, diff),
      cancel() {},
    });
    const client = new BackendClient(
      new FakeBackendTransport({
        repositories: [{ provider: "github", workspace: "acme", repo: "app", localPath: null }],
        files: [
          { path: "src/app.ts", status: "modified", additions: 1, deletions: 1, oldPath: null },
        ],
        diffText: "@@ -1 +1 @@\n-const old = 'before';\n+const next = '世界';",
        findings: [],
      }),
    );
    await store.connect(client);
    if (split) store.toggleDiffMode();
    const ui = mountApp(renderer.renderer, store, client);
    try {
      for (let index = 0; index < 4; index++) await renderer.renderOnce();
      const text = renderer.captureCharFrame();
      expect(text).toContain("const old = 'before';");
      expect(text).toContain("const next = '世界';");
      expect(text).toContain("typescript");
      const spans = renderer.captureSpans().lines.flatMap((line) => line.spans);
      expect(
        spans.some(
          (span) =>
            span.text.includes("const") && span.fg.toInts().slice(0, 3).join(",") === "199,146,234",
        ),
      ).toBe(true);
      expect(
        spans.some(
          (span) =>
            span.text.includes("世界") && span.fg.toInts().slice(0, 3).join(",") === "165,214,167",
        ),
      ).toBe(true);
    } finally {
      ui.unmount();
      client.close();
      renderer.renderer.destroy();
    }
  });
