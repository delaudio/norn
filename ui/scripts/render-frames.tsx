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
  build: () => Promise<{ store: ShellStore; client: BackendClient }>;
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

const scenes: Scene[] = [
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

function renderScene(store: ShellStore, client: BackendClient): ReactNode {
  return (
    <box width="100%" height="100%" backgroundColor={theme.background}>
      <Shell store={store} client={client} quit={() => {}} />
    </box>
  );
}

const frames: Record<string, TerminalFrame> = {};
for (const scene of scenes) {
  const { store, client } = await scene.build();
  const test = await createTestRenderer({
    width: scene.cols,
    height: scene.rows,
    backgroundColor: theme.background,
  });
  try {
    const root = createRoot(test.renderer);
    flushSync(() => root.render(renderScene(store, client)));
    await test.renderOnce();
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
