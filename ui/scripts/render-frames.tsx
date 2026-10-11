import { DataPanel } from "../src/app/components/DataPanel";
import { SettingsPanel } from "../src/app/components/SettingsPanel";
import { DataStore } from "../src/app/dataStore";
import { SettingsStore } from "../src/app/settingsStore";
// Renders every terminal component/scene with the real OpenTUI renderer and
// serializes bounded frame fixtures (cell spans, colors, attributes) for the
// Storybook preview and the golden-frame visual lane.
//
// Deterministic: fixed fixtures, no HOME/config/keychain/network access. Run
// with `bun run frames:generate`.

import { mkdir, writeFile } from "node:fs/promises";
import { dirname } from "node:path";
import { pathToFileURL } from "node:url";
import { type RGBA, TextAttributes } from "@opentui/core";
import { createTestRenderer } from "@opentui/core/testing";
import { createRoot, flushSync } from "@opentui/react";
import type { ReactNode } from "react";
import { Shell } from "../src/app/components/Shell";
import { ShellStore } from "../src/app/store";
import { theme } from "../src/app/theme";
import { BackendClient } from "../src/transport/backendClient";
import { FakeBackendTransport } from "../src/transport/fakeTransport";
import type { BackendTransport, ReadyInfo } from "../src/transport/types";
import type { TerminalFrame } from "../storybook/frame";

const repositories = [
  {
    provider: "github" as const,
    workspace: "acme",
    repo: "payments",
    localPath: "/home/dev/payments",
  },
  { provider: "bitbucket" as const, workspace: "acme", repo: "billing", localPath: null },
];

const files = [
  { path: "src/app.ts", status: "modified", additions: 12, deletions: 3, oldPath: null },
  { path: "src/new.ts", status: "added", additions: 40, deletions: 0, oldPath: null },
  { path: "README.md", status: "deleted", additions: 0, deletions: 8, oldPath: null },
];
const findings = [
  {
    id: "f1",
    title: "Missing null check",
    severity: "high",
    summary: "s",
    anchor: { path: "src/app.ts", startLine: 4, endLine: null, side: "new" },
  },
];
const staleFindings = [
  ...findings,
  {
    id: "f3",
    title: "Anchor elsewhere",
    severity: "medium",
    summary: "s",
    anchor: { path: "gone/file.ts", startLine: 10, endLine: null, side: "new" },
  },
];
const targets = [
  {
    provider: "github" as const,
    workspace: "acme",
    repo: "payments",
    prId: 128,
    runId: "run-128",
    title: "Add idempotency keys",
    status: "succeeded",
  },
  {
    provider: "bitbucket" as const,
    workspace: "acme",
    repo: "billing",
    prId: 64,
    runId: "run-64",
    title: "Retry hardening",
    status: "failed",
  },
];

class NeverReadyTransport implements BackendTransport {
  start(): Promise<ReadyInfo> {
    return new Promise<ReadyInfo>(() => {});
  }
  request(): Promise<Record<string, unknown>> {
    return new Promise<Record<string, unknown>>(() => {});
  }
  onOperationEvent(): () => void {
    return () => {};
  }
  onExit(): () => void {
    return () => {};
  }
  close(): void {}
}

class FailingTransport implements BackendTransport {
  async start(): Promise<ReadyInfo> {
    throw new Error("norn-backend is not installed");
  }
  async request(): Promise<Record<string, unknown>> {
    throw new Error("unavailable");
  }
  onOperationEvent(): () => void {
    return () => {};
  }
  onExit(): () => void {
    return () => {};
  }
  close(): void {}
}

interface Scene {
  id: string;
  cols: number;
  rows: number;
  build: () => Promise<{
    store: ShellStore;
    client: BackendClient;
    view?: ReactNode;
    afterMount?: () => void;
  }>;
}

function readyStore(transport: BackendTransport) {
  const client = new BackendClient(transport);
  const store = new ShellStore();
  return { client, store, ready: store.connect(client) };
}

async function emptyScene() {
  const { client, store, ready } = readyStore(new FakeBackendTransport({ repositories: [] }));
  await ready;
  return { store, client };
}

async function readyScene() {
  const { client, store, ready } = readyStore(
    new FakeBackendTransport({ repositories, files, findings }),
  );
  await ready;
  return { store, client };
}

async function connectingScene() {
  const client = new BackendClient(new NeverReadyTransport());
  const store = new ShellStore();
  void store.connect(client);
  return { store, client };
}

async function errorScene() {
  const { client, store, ready } = readyStore(new FailingTransport());
  await ready;
  return { store, client };
}

async function closedScene() {
  const { client, store, ready } = readyStore(
    new FakeBackendTransport({ repositories, files, findings }),
  );
  await ready;
  store.markClosed();
  return { store, client };
}

async function reviewScene() {
  const { client, store, ready } = readyStore(
    new FakeBackendTransport({ repositories, files, findings }),
  );
  await ready;
  await store.startReview(client);
  await Bun.sleep(5);
  return { store, client };
}

async function staleAnchorScene() {
  const { client, store, ready } = readyStore(
    new FakeBackendTransport({ repositories, files, findings: staleFindings }),
  );
  await ready;
  await Bun.sleep(5);
  store.cycleFocus();
  store.cycleFocus();
  store.move(1);
  return { store, client };
}

async function targetScene() {
  const { client, store, ready } = readyStore(
    new FakeBackendTransport({ repositories, targets, files, findings }),
  );
  await ready;
  await Bun.sleep(5);
  return { store, client };
}

async function splitScene() {
  const { client, store, ready } = readyStore(
    new FakeBackendTransport({ repositories, files, findings }),
  );
  await ready;
  await Bun.sleep(5);
  store.toggleDiffMode();
  return { store, client };
}

function longDiff(lineCount: number): string {
  const body = Array.from({ length: lineCount }, (_, index) =>
    index % 2 === 0 ? `+line ${index}` : `-line ${index}`,
  ).join("\n");
  return `@@ -1,${lineCount} +1,${lineCount} @@\n${body}\n`;
}

async function largeDiffScene() {
  const { client, store, ready } = readyStore(
    new FakeBackendTransport({ repositories, files, findings, diffText: longDiff(2500) }),
  );
  await ready;
  await Bun.sleep(5);
  return { store, client };
}

async function groupedFindingsScene() {
  const scene = await readyScene();
  scene.store.cycleFindingGroup();
  return scene;
}

async function conversationScene() {
  const { client, store, ready } = readyStore(
    new FakeBackendTransport({
      repositories,
      targets,
      files,
      findings,
      threads: [
        {
          id: "thread-128",
          title: "Idempotency review",
          latestRunId: "run-128",
          truncated: false,
          messages: [
            {
              id: "user-1",
              role: "user",
              content: "Explain the missing null check.",
              truncated: false,
            },
            {
              id: "assistant-1",
              role: "assistant",
              content: "The optional key must be checked before lookup. ✓",
              truncated: false,
            },
          ],
        },
      ],
    }),
  );
  await ready;
  await store.selectPicker(2);
  store.toggleThreads();
  return { client, store };
}

async function fixPreviewScene() {
  const { client, store, ready } = readyStore(
    new FakeBackendTransport({ repositories, targets, files, findings }),
  );
  await ready;
  await store.selectPicker(2);
  await store.previewFix(client);
  return { client, store };
}

async function commitComposerScene() {
  const { client, store, ready } = readyStore(
    new FakeBackendTransport({ repositories, targets, files, findings }),
  );
  await ready;
  await store.selectPicker(2);
  await store.beginCommit(client);
  return { client, store };
}
async function commitPreviewScene() {
  const scene = await commitComposerScene();
  await scene.store.previewCommit(scene.client);
  return scene;
}
async function pushPreviewScene() {
  const { client, store, ready } = readyStore(
    new FakeBackendTransport({ repositories, targets, files, findings, fixPhase: "readyToPush" }),
  );
  await ready;
  await store.selectPicker(2);
  await store.previewPush(client);
  return { client, store };
}

async function branchScene(action: "sync" | "conflicts") {
  const { client, store, ready } = readyStore(
    new FakeBackendTransport({ repositories, targets, files, findings }),
  );
  await ready;
  await store.selectPicker(2);
  await store.previewBranch(client, action);
  return { client, store };
}

async function backgroundOperationScene() {
  class BackgroundTransport extends FakeBackendTransport {
    async request(method: string, params: Record<string, unknown>) {
      if (method === "review.start")
        return { operationId: "background-review", state: "accepted", sequence: 0 };
      if (method === "operation.status")
        return { operationId: "background-review", state: "running", sequence: 1, error: null };
      return super.request(method, params);
    }
  }
  const { client, store, ready } = readyStore(
    new BackgroundTransport({ repositories, targets, files, findings }),
  );
  await ready;
  await store.startReview(client);
  await store.selectPicker(1);
  return { client, store };
}

async function settingsScene(mode: "normal" | "secret" | "error" | "light") {
  class SettingsTransport extends FakeBackendTransport {
    async request(method: string, params: Record<string, unknown>) {
      if (mode === "error" && method === "settings.get")
        throw new Error(
          "Stored preferences could not be read. Preserve the file before repairing it.",
        );
      return super.request(method, params);
    }
  }
  const transport = new SettingsTransport({
    repositories,
    credentials: [{ provider: "github", available: true, source: "keychain" }],
  });
  const { client, store, ready } = readyStore(transport);
  await ready;
  const settings = new SettingsStore();
  await settings.load(client);
  if (mode === "light") {
    const snapshot = await client.settings();
    snapshot.settings.theme = "light";
    await store.applySettings(client, snapshot.settings);
  }
  return {
    client,
    store,
    view: <SettingsPanel settings={settings} shell={store} client={client} />,
    afterMount:
      mode === "secret"
        ? () => {
            settings.select("token");
            settings.beginEdit();
            settings.append("visual-fixture-only");
          }
        : undefined,
  };
}

async function dataScene(mode: "preview" | "error" | "done") {
  class DataTransport extends FakeBackendTransport {
    async request(method: string, params: Record<string, unknown>) {
      if (method === "data.preview") {
        if (mode === "error")
          throw new Error("Export file could not be read. Preserve the source and retry.");
        return {
          inputVersion: "a".repeat(64),
          destinationVersion: "b".repeat(64),
          additionsCount: 2,
          unchangedCount: 1,
          conflictsCount: 1,
          rejectedCount: 1,
          detailsTruncated: false,
          additions: ["prompt.github:demo/workspace", "viewed.github:demo/workspace/7"],
          unchanged: ["theme"],
          conflicts: ["drafts.github:demo/workspace/7"],
          rejected: [{ entry: 4, reason: "Historical scope requires an explicit provider." }],
        };
      }
      if (method === "data.import")
        return {
          imported: 2,
          unchanged: 1,
          conflicts: 1,
          rejectedCount: 1,
          detailsTruncated: false,
          rejected: [],
          backupPath: "/synthetic/user-data-backup.json",
          destinationVersion: "c".repeat(64),
        };
      return super.request(method, params);
    }
  }
  const { client, store, ready } = readyStore(
    new DataTransport({ repositories, targets, files, findings }),
  );
  await ready;
  const data = new DataStore();
  data.beginEdit();
  data.append("/synthetic/desktop-export.json");
  data.finishEdit();
  await data.preview(client);
  if (mode === "done") await data.confirm(client);
  return { client, store, view: <DataPanel data={data} client={client} onClose={() => {}} /> };
}

async function imageScene(status: "modified" | "added" | "deleted" | "renamed" | "unsupported") {
  class ImageTransport extends FakeBackendTransport {
    async request(method: string, params: Record<string, unknown>) {
      if (status === "unsupported" && method === "file.preview")
        throw new Error("Unsupported terminal image format; use the browser viewer.");
      return super.request(method, params);
    }
  }
  const imageFiles = [
    {
      path: "assets/diagram.png",
      status: status === "unsupported" ? "modified" : status,
      additions: 0,
      deletions: 0,
      oldPath: status === "renamed" ? "assets/previous.png" : null,
    },
  ];
  const { client, store, ready } = readyStore(
    new ImageTransport({ repositories, files: imageFiles, findings: [] }),
  );
  await ready;
  await Bun.sleep(5);
  return { client, store };
}

const scenes: Scene[] = [
  { id: "shell-images-modified-120x30", cols: 120, rows: 30, build: () => imageScene("modified") },
  { id: "shell-images-modified-50x20", cols: 50, rows: 20, build: () => imageScene("modified") },
  { id: "shell-images-added-80x24", cols: 80, rows: 24, build: () => imageScene("added") },
  { id: "shell-images-deleted-80x24", cols: 80, rows: 24, build: () => imageScene("deleted") },
  { id: "shell-images-renamed-100x24", cols: 100, rows: 24, build: () => imageScene("renamed") },
  {
    id: "shell-images-unsupported-80x24",
    cols: 80,
    rows: 24,
    build: () => imageScene("unsupported"),
  },
  { id: "shell-data-preview-120x30", cols: 120, rows: 30, build: () => dataScene("preview") },
  { id: "shell-data-preview-50x15", cols: 50, rows: 15, build: () => dataScene("preview") },
  { id: "shell-data-error-80x24", cols: 80, rows: 24, build: () => dataScene("error") },
  { id: "shell-data-imported-80x24", cols: 80, rows: 24, build: () => dataScene("done") },
  { id: "shell-settings-120x40", cols: 120, rows: 40, build: () => settingsScene("normal") },
  { id: "shell-settings-50x15", cols: 50, rows: 15, build: () => settingsScene("normal") },
  { id: "shell-settings-secret-120x40", cols: 120, rows: 40, build: () => settingsScene("secret") },
  { id: "shell-settings-secret-50x15", cols: 50, rows: 15, build: () => settingsScene("secret") },
  { id: "shell-settings-error-80x24", cols: 80, rows: 24, build: () => settingsScene("error") },
  { id: "shell-settings-light-120x40", cols: 120, rows: 40, build: () => settingsScene("light") },
  { id: "shell-background-operation-120x30", cols: 120, rows: 30, build: backgroundOperationScene },
  { id: "shell-background-operation-50x15", cols: 50, rows: 15, build: backgroundOperationScene },
  { id: "shell-sync-preview-120x30", cols: 120, rows: 30, build: () => branchScene("sync") },
  {
    id: "shell-conflicts-preview-120x30",
    cols: 120,
    rows: 30,
    build: () => branchScene("conflicts"),
  },
  {
    id: "shell-conflicts-preview-50x15",
    cols: 50,
    rows: 15,
    build: () => branchScene("conflicts"),
  },
  { id: "shell-commit-composer-120x30", cols: 120, rows: 30, build: commitComposerScene },
  { id: "shell-commit-preview-120x30", cols: 120, rows: 30, build: commitPreviewScene },
  { id: "shell-push-preview-120x30", cols: 120, rows: 30, build: pushPreviewScene },
  { id: "shell-push-preview-50x15", cols: 50, rows: 15, build: pushPreviewScene },
  { id: "shell-fix-preview-120x30", cols: 120, rows: 30, build: fixPreviewScene },
  { id: "shell-fix-preview-50x15", cols: 50, rows: 15, build: fixPreviewScene },
  { id: "shell-grouped-findings-120x30", cols: 120, rows: 30, build: groupedFindingsScene },
  { id: "shell-conversation-120x30", cols: 120, rows: 30, build: conversationScene },
  { id: "shell-conversation-50x15", cols: 50, rows: 15, build: conversationScene },
  { id: "shell-ready-80x24", cols: 80, rows: 24, build: readyScene },
  { id: "shell-ready-50x15", cols: 50, rows: 15, build: readyScene },
  { id: "shell-ready-120x30", cols: 120, rows: 30, build: readyScene },
  { id: "shell-empty-80x24", cols: 80, rows: 24, build: emptyScene },
  { id: "shell-connecting-80x24", cols: 80, rows: 24, build: connectingScene },
  { id: "shell-error-80x24", cols: 80, rows: 24, build: errorScene },
  { id: "shell-closed-80x24", cols: 80, rows: 24, build: closedScene },
  { id: "shell-review-80x24", cols: 80, rows: 24, build: reviewScene },
  { id: "shell-stale-anchor-80x24", cols: 80, rows: 24, build: staleAnchorScene },
  { id: "shell-review-targets-120x30", cols: 120, rows: 30, build: targetScene },
  { id: "shell-split-120x30", cols: 120, rows: 30, build: splitScene },
  { id: "shell-large-diff-120x30", cols: 120, rows: 30, build: largeDiffScene },
];

function color(value: RGBA): string {
  const [r, g, b, a] = value.toInts();
  return `rgba(${r},${g},${b},${a / 255})`;
}

function renderScene(store: ShellStore, client: BackendClient, view?: ReactNode): ReactNode {
  return (
    <box width="100%" height="100%" backgroundColor={theme.background}>
      {view ?? <Shell store={store} client={client} quit={() => {}} />}
    </box>
  );
}

const frames: Record<string, TerminalFrame> = {};
for (const scene of scenes) {
  const { store, client, view, afterMount } = await scene.build();
  const test = await createTestRenderer({
    width: scene.cols,
    height: scene.rows,
    backgroundColor: theme.background,
  });
  const root = createRoot(test.renderer);
  try {
    flushSync(() => root.render(renderScene(store, client, view)));
    await test.renderOnce();
    if (afterMount) {
      afterMount();
      await test.renderOnce();
    }
    const frame = test.captureSpans();
    frames[scene.id] = {
      cols: frame.cols,
      rows: frame.rows,
      lines: frame.lines.map((line) => ({
        spans: line.spans.map((span) => ({
          text: span.text,
          width: span.width,
          fg: color(span.fg),
          bg: color(span.bg),
          attributes: span.attributes,
          bold: !!(span.attributes & TextAttributes.BOLD),
          italic: !!(span.attributes & TextAttributes.ITALIC),
          underline: !!(span.attributes & TextAttributes.UNDERLINE),
        })),
      })),
    };
  } finally {
    flushSync(() => root.unmount());
    test.renderer.destroy();
    client.close();
  }
}

await mkdir(new URL("../storybook/generated/", import.meta.url), { recursive: true });
const outputArg = process.argv[2];
const outputUrl = outputArg
  ? pathToFileURL(outputArg)
  : new URL("../storybook/generated/frames.json", import.meta.url);
const outputDir = outputArg
  ? pathToFileURL(`${dirname(outputArg)}/`)
  : new URL("../storybook/generated/", import.meta.url);
await mkdir(outputDir, { recursive: true });
await writeFile(outputUrl, `${JSON.stringify(frames, null, 2)}\n`);
console.log(`Rendered ${scenes.length} native OpenTUI frames.`);
