// Physical-terminal demo: synthetic image bytes only. No Rust backend, Git,
// application settings, credential store, provider or agent is used.

import { writeFile } from "node:fs/promises";
import { createCliRenderer } from "@opentui/core";
import { ShellStore } from "../src/app/store";
import { theme } from "../src/app/theme";
import { mountApp } from "../src/app/ui";
import { BackendClient } from "../src/transport/backendClient";
import { FakeBackendTransport } from "../src/transport/fakeTransport";

if (Bun.argv.includes("--help")) {
  console.log(
    "Synthetic Norn terminal image smoke. Run in Kitty/Sixel outside tmux; inspect both colors, resize wide/narrow, q exits. Optional --report NEW_PATH writes synthetic terminal diagnostics without overwriting files. No backend or user data is accessed.",
  );
  process.exit(0);
}
if (!process.stdin.isTTY || !process.stdout.isTTY)
  throw new Error("Run this image smoke in an interactive terminal.");
const reportIndex = Bun.argv.indexOf("--report");
const reportPath = reportIndex >= 0 ? Bun.argv[reportIndex + 1] : undefined;
if (reportIndex >= 0 && (!reportPath || reportPath.startsWith("--")))
  throw new Error("--report requires a new diagnostic file path.");
class ImageSmokeTransport extends FakeBackendTransport {
  async request(method: string, params: Record<string, unknown>) {
    if (method === "browser.open") throw new Error("Synthetic image demo has no browser backend.");
    if (
      method.startsWith("review.") &&
      ![
        "review.targets",
        "review.files",
        "review.findings",
        "review.history",
        "review.threads",
        "review.fix.state",
      ].includes(method)
    )
      throw new Error("Synthetic image demo does not start reviews or modify files.");
    return super.request(method, params);
  }
}
const client = new BackendClient(
  new ImageSmokeTransport({
    repositories: [
      { provider: "github", workspace: "synthetic", repo: "image-smoke", localPath: null },
    ],
    files: [
      { path: "before-after.png", status: "modified", additions: 0, deletions: 0, oldPath: null },
    ],
    findings: [],
  }),
);
const renderer = await createCliRenderer({
  exitOnCtrlC: true,
  backgroundColor: theme.background,
  screenMode: "alternate-screen",
  useMouse: true,
});
renderer.setTerminalTitle("Norn synthetic image smoke");
let reportTimer: ReturnType<typeof setTimeout> | undefined;
if (reportPath) {
  reportTimer = setTimeout(
    () =>
      void writeFile(
        reportPath,
        JSON.stringify(
          {
            dimensions: { width: renderer.width, height: renderer.height },
            capabilities: renderer.capabilities,
            resolution: renderer.resolution,
          },
          null,
          2,
        ),
        { flag: "wx" },
      ).catch(() => {
        console.error(
          "Synthetic terminal diagnostic report could not be written (existing files are preserved).",
        );
      }),
    1000,
  );
}
const store = new ShellStore();
const ui = mountApp(renderer, store, client, () => {
  clearTimeout(reportTimer);
  ui.unmount();
  client.close();
  renderer.destroy();
  process.exit(0);
});
try {
  await store.connect(client);
} catch (error) {
  clearTimeout(reportTimer);
  ui.unmount();
  client.close();
  renderer.destroy();
  throw error;
}
