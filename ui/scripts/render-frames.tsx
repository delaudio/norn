import { DataPanel } from "../src/app/components/DataPanel";
import { DraftPanel } from "../src/app/components/DraftPanel";
import { DraftStagingPanel } from "../src/app/components/DraftStagingPanel";
import { ExplorerPanel } from "../src/app/components/ExplorerPanel";
import { ProviderPanel } from "../src/app/components/ProviderPanel";
import { PublicationPanel } from "../src/app/components/PublicationPanel";
import { ReferencePanel } from "../src/app/components/ReferencePanel";
import { RepositoryOperationsPanel } from "../src/app/components/RepositoryOperationsPanel";
import { SettingsPanel } from "../src/app/components/SettingsPanel";
import { WorkspacePanel } from "../src/app/components/WorkspacePanel";
import { DataStore } from "../src/app/dataStore";
import { DraftStagingStore } from "../src/app/draftStagingStore";
import { DraftStore } from "../src/app/draftStore";
import { ExplorerStore, treeRows } from "../src/app/explorerStore";
import { ProviderStore } from "../src/app/providerStore";
import { PublicationStore } from "../src/app/publicationStore";
import { ReferenceStore } from "../src/app/referenceStore";
import { RepositoryOperationsStore } from "../src/app/repositoryOperationsStore";
import { SettingsStore } from "../src/app/settingsStore";
import { WorkspaceStore } from "../src/app/workspaceStore";
import { FakeProfileTransport } from "../src/transport/fakeProfileTransport";
import {
  FakeProviderTransport,
  providerFixtureRepositories,
} from "../src/transport/fakeProviderTransport";
import {
  FakePublicationTransport,
  publicationFixtureScope,
} from "../src/transport/fakePublicationTransport";
import { EditorExplorerFixture } from "../test/fixtures/editor-backend";
import { ExplorerFixture } from "../test/fixtures/explorer-backend";
import { LocalTransport } from "../test/fixtures/local-backend";
import { LocalRasterTransport } from "../test/fixtures/local-raster-backend";
import { RepositoryOperationsFixture } from "../test/fixtures/repository-operations-backend";
import { SavedTransport } from "../test/fixtures/saved-local-backend";
// Renders every terminal component/scene with the real OpenTUI renderer and
// serializes bounded frame fixtures (cell spans, colors, attributes) for the
// Storybook preview and the golden-frame visual lane.
//
// Deterministic: fixed fixtures, no HOME/config/keychain/network access. Run
// with `bun run frames:generate`.

import { mkdir, writeFile } from "node:fs/promises";
import { dirname } from "node:path";
import { pathToFileURL } from "node:url";
import { getTreeSitterClient, type RGBA, TextAttributes } from "@opentui/core";
import { createTestRenderer } from "@opentui/core/testing";
import { createRoot, flushSync } from "@opentui/react";
import type { ReactNode } from "react";
import { Shell } from "../src/app/components/Shell";
import { ShellStore } from "../src/app/store";
import { parseDiffSyntax } from "../src/app/syntaxTokens";
import { theme } from "../src/app/theme";
import { BackendClient } from "../src/transport/backendClient";
import { FakeBackendTransport } from "../src/transport/fakeTransport";
import { importedScopeRecords } from "../src/transport/scopeFixtures";
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
    afterMount?: (test: Awaited<ReturnType<typeof createTestRenderer>>) => void | Promise<void>;
  }>;
}

function readyStore(transport: BackendTransport) {
  const client = new BackendClient(transport);
  const store = new ShellStore({
    highlight: async (path, text) => parseDiffSyntax(path, text),
    cancel() {},
  });
  return { client, store, ready: store.connect(client) };
}

async function emptyScene() {
  const { client, store, ready } = readyStore(new FakeBackendTransport({ repositories: [] }));
  await ready;
  return { store, client };
}
async function savedScopeScene() {
  const scene = readyStore(
    new FakeBackendTransport({ repositories: [], workspaceRecords: importedScopeRecords }),
  );
  await scene.ready;
  return { store: scene.store, client: scene.client };
}
async function importedDraftScene() {
  const scene = await savedScopeScene();
  scene.store.toggleDrafts();
  return scene;
}
async function onboardingScene(unsupported: boolean) {
  const scene = readyStore(
    new FakeBackendTransport({
      repositories: [],
      availableTools: ["git"],
      credentialStore: unsupported ? "unsupported" : "native",
    }),
  );
  await scene.ready;
  return { store: scene.store, client: scene.client };
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

async function reviewProfileScene() {
  const { client, store, ready } = readyStore(
    new FakeBackendTransport({ repositories, targets, files, findings }),
  );
  await ready;
  await store.selectPicker(2);
  store.beginReviewProfile();
  store.appendReplyText("security");
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

async function settingsScene(mode: "normal" | "secret" | "error" | "light" | "imported-theme") {
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
    workspaceRecords: { theme: { kind: "theme", value: "light" } },
    repositories,
    credentials: [{ provider: "github", available: true, source: "keychain" }],
  });
  const { client, store, ready } = readyStore(transport);
  await ready;
  const settings = new SettingsStore();
  await settings.load(client);
  if (mode === "imported-theme") await settings.previewImportedTheme(client);
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

async function wrappedAnchorScene() {
  const anchored = [
    {
      id: "wrapped",
      title: "Wrapped anchor",
      severity: "high",
      summary: "s",
      anchor: { path: files[0]!.path, startLine: 550, endLine: null, side: "old" },
    },
  ];
  const text = `@@ -1,600 +1,600 @@\n${Array.from({ length: 600 }, (_, index) => ` ${index === 549 ? "ANCHOR HERE" : `row ${index + 1} 界 é\t wrapped words `.repeat(3)}`).join("\n")}\n`;
  const scene = readyStore(
    new FakeBackendTransport({ repositories, files, findings: anchored, diffText: text }),
  );
  await scene.ready;
  await Bun.sleep(5);
  scene.store.toggleDiffMode();
  scene.store.toggleDiffExpanded();
  scene.store.cycleFocus();
  scene.store.cycleFocus();
  scene.store.move(1);
  return { store: scene.store, client: scene.client };
}

async function workspaceScene(mode: "saved" | "dirty" | "conflict" | "disposed") {
  const transport = new FakeBackendTransport({
    repositories,
    workspaceRecords: {
      "prompt.github:acme/payments": {
        kind: "prompt",
        value: "Imported prompt: focus on correctness.\nKeep findings actionable.",
      },
      "viewed.github:acme/payments/7": { kind: "viewed", value: ["src/app.ts"] },
    },
  });
  const { client, store, ready } = readyStore(transport);
  await ready;
  const workspace = new WorkspaceStore();
  await workspace.load(client, {
    kind: "review",
    provider: "github",
    workspace: "acme",
    repo: "payments",
    prId: 7,
    runId: "synthetic",
    title: "PR",
    status: "completed",
  });
  if (mode === "dirty" || mode === "conflict") workspace.append("\nMy unsaved change");
  if (mode === "conflict") {
    const page = await client.readWorkspace("prompt.github:acme/payments");
    await client.mutateWorkspace(page.key, page.recordVersion, {
      kind: "setPrompt",
      value: "Another writer",
    });
    await workspace.save(client);
  }
  if (mode === "disposed") workspace.dispose();
  return {
    client,
    store,
    view: (
      <WorkspacePanel workspace={workspace} client={client} path="src/app.ts" onClose={() => {}} />
    ),
  };
}

async function referencesScene(mode: "saved" | "editing" | "conflict" | "remove" | "local") {
  class ReferenceTransport extends FakeBackendTransport {
    async request(method: string, params: Record<string, unknown>) {
      if (mode === "conflict" && method === "workspace.mutate")
        throw new Error("User data changed. Edits retained; refresh before saving.");
      return super.request(method, params);
    }
  }
  const transport = new ReferenceTransport({
    repositories,
    targets,
    workspaceRecords: {
      "references.github:acme/payments/7": {
        kind: "references",
        value: [
          {
            id: "saved-note",
            type: "note",
            source: "manual",
            title: "Architecture context",
            body: "Imported reviewer guidance 世界.\nKeep compatibility with the related interface.",
            createdAt: 1,
            updatedAt: 2,
          },
          {
            id: "saved-jira",
            type: "jira",
            source: "detected",
            title: "Related issue",
            key: "DEMO-42",
            url: "https://example.invalid/DEMO-42",
            createdAt: 1,
            updatedAt: 2,
          },
          {
            id: "saved-repo",
            type: "repository",
            source: "manual",
            workspace: "synthetic",
            repo: "contracts",
            localPath: "/synthetic/read-only",
            createdAt: 1,
            updatedAt: 2,
          },
        ],
      },
    },
  });
  const { client, store, ready } = readyStore(transport);
  await ready;
  const references = new ReferenceStore(
    () => 123,
    () => "synthetic-new-reference",
  );
  await references.load(client, {
    provider: "github",
    workspace: "acme",
    repo: "payments",
    prId: mode === "local" ? null : 7,
  });
  if (mode === "editing" || mode === "conflict") {
    references.edit();
    for (let i = 0; i < 8; i++) references.move(1);
    references.beginField();
    references.append("\nUnsaved reviewer note.");
    references.finishField();
    if (mode === "conflict") await references.save(client);
  }
  if (mode === "remove") references.requestRemoval();
  return {
    client,
    store,
    view: <ReferencePanel references={references} client={client} onClose={() => {}} />,
  };
}

async function draftsScene(mode: "saved" | "editing" | "conflict" | "remove" | "local") {
  class Transport extends FakeBackendTransport {
    async request(method: string, params: Record<string, unknown>) {
      if (mode === "conflict" && method === "workspace.mutate")
        throw new Error("User data changed. Edits retained; refresh before saving.");
      return super.request(method, params);
    }
  }
  const transport = new Transport({
    workspaceRecords: {
      "drafts.github:acme/payments/7": {
        kind: "drafts",
        value: [
          {
            localId: "draft-1",
            prId: 7,
            path: "src/payment.ts",
            to: 12,
            raw: "Imported finding comment 世界.\nHandle the missing key before processing.",
            createdAt: 1,
            source: "aiFinding",
            publicationMode: "inline",
            findingRef: {
              reviewRunId: "run-7",
              findingId: "finding-1",
              findingFingerprint: "synthetic-fingerprint",
            },
            reviewBaseSha: "a".repeat(40),
            reviewHeadSha: "b".repeat(40),
          },
          {
            localId: "draft-2",
            prId: 7,
            path: "",
            raw: "General reviewer note. Saved locally.",
            parentId: "19",
            createdAt: 2,
            source: "manual",
            publicationMode: "general",
          },
          {
            localId: "draft-3",
            prId: 7,
            path: "src/legacy.ts",
            from: 3,
            raw: "Local-only observation.",
            createdAt: 3,
            source: null,
            publicationMode: "localOnly",
          },
        ],
      },
    },
  });
  const { client, store, ready } = readyStore(transport);
  await ready;
  const drafts = new DraftStore(
    () => 123,
    () => "synthetic-new-draft",
  );
  await drafts.load(client, {
    provider: "github",
    workspace: "acme",
    repo: "payments",
    prId: mode === "local" ? null : 7,
  });
  if (mode === "editing" || mode === "conflict") {
    drafts.edit();
    for (let i = 0; i < 5; i++) drafts.move(1);
    drafts.beginField();
    drafts.append("\nUnsaved reviewer edit.");
    drafts.finishField();
    if (mode === "conflict") await drafts.save(client);
  }
  if (mode === "remove") drafts.requestRemoval();
  return { client, store, view: <DraftPanel drafts={drafts} client={client} onClose={() => {}} /> };
}

async function stagingScene(
  mode: "choose" | "preview" | "general" | "unanchored" | "staged" | "duplicate" | "conflict",
) {
  class Transport extends FakeBackendTransport {
    conflict = false;
    async request(method: string, params: Record<string, unknown>) {
      if (this.conflict && method === "draft.stage")
        throw new Error(
          "Drafts changed. Inspect saved drafts and refresh this preview before retrying.",
        );
      return super.request(method, params);
    }
  }
  const transport = new Transport({
    findings: [
      {
        id: "finding-1",
        title: "Validate payment key",
        severity: "high",
        summary: "Saved finding 世界.\nCheck the missing key before sending a payment.",
        anchor:
          mode === "unanchored" ? null : { path: "src/payment.ts", startLine: 12, side: "new" },
      },
    ],
  });
  const { client, store, ready } = readyStore(transport);
  await ready;
  const staging = new DraftStagingStore();
  const target = {
    kind: "provider" as const,
    provider: "github" as const,
    workspace: "acme",
    repo: "payments",
    prId: 7,
    runId: "run-7",
  };
  staging.open(target, "finding-1");
  if (mode === "general") {
    staging.moveMode(1);
    staging.moveMode(1);
  }
  if (mode !== "choose") await staging.preview(client);
  if (mode === "conflict") {
    transport.conflict = true;
    await staging.stage(client);
  }
  if (mode === "staged" || mode === "duplicate") await staging.stage(client);
  if (mode === "duplicate") {
    const prepared = staging.getSnapshot().prepared!;
    const record = await client.readWorkspace(prepared.key);
    await client.mutateWorkspace(prepared.key, record.recordVersion, {
      kind: "putDraft",
      draft: {
        ...prepared.draft,
        raw: "Existing reviewer edit. Preserve it during repeated staging.",
      },
    });
    staging.open(target, "finding-1");
    await staging.preview(client);
  }
  return {
    client,
    store,
    view: (
      <DraftStagingPanel staging={staging} client={client} onClose={() => {}} onDrafts={() => {}} />
    ),
  };
}

async function searchScene(mode: "editing" | "none" | "split") {
  const transport = new FakeBackendTransport({
    repositories,
    files,
    diffText: [
      "diff --git a/a.ts b/a.ts",
      "--- a/a.ts",
      "+++ b/a.ts",
      "@@ -1,1000 +1,1000 @@",
      ...Array.from(
        { length: 1000 },
        (_, index) => ` ${index === 500 ? "MATCH_NEEDLE 界✓ " : ""}ROW-${index}`,
      ),
    ].join("\n"),
  });
  const { client, store, ready } = readyStore(transport);
  await ready;
  if (mode === "split") store.toggleDiffMode();
  store.beginSearch();
  store.appendSearch(mode === "none" ? "absent-query" : "MATCH_NEEDLE");
  if (mode !== "editing") store.applySearch();
  return { client, store };
}

async function syntaxScene(mode: "unified" | "split" | "plain") {
  const path = mode === "plain" ? "src/custom.unknown" : "src/example.tsx";
  const scene = readyStore(
    new FakeBackendTransport({
      repositories,
      files: [{ path, status: "modified", additions: 2, deletions: 2, oldPath: null }],
      findings: [],
      diffText:
        "@@ -1,3 +1,3 @@\n // Keep both revisions and Unicode 世界\n-const old = <div value={1}>Before</div>;\n+const next = <div value={2}>After</div>;\n export default next;",
    }),
  );
  await scene.ready;
  if (mode === "split") scene.store.toggleDiffMode();
  return { store: scene.store, client: scene.client };
}
async function publicationScene(mode: "drafts" | "preview" | "partial" | "status" | "threads") {
  const transport = new FakePublicationTransport();
  const { client, store, ready } = readyStore(transport);
  await ready;
  const publication = new PublicationStore();
  await publication.load(client, publicationFixtureScope);
  publication.toggle();
  publication.move(1);
  publication.toggle();
  if (["preview", "partial", "status"].includes(mode)) await publication.prepare();
  if (mode === "partial" || mode === "status") {
    transport.uncertainAt = 2;
    await publication.approve();
  }
  if (mode === "status") await publication.show("status");
  if (mode === "threads") await publication.show("threads");
  return {
    client,
    store,
    view: <PublicationPanel publication={publication} onClose={() => {}} onReply={() => {}} />,
  };
}

async function providerScene(
  kind:
    | "list"
    | "detail"
    | "empty"
    | "error"
    | "loading"
    | "identity"
    | "browser"
    | "approval"
    | "approval-uncertain"
    | "approval-bitbucket"
    | "approval-bitbucket-uncertain",
) {
  const transport = new FakeProviderTransport();
  if (kind === "empty") transport.empty = true;
  if (kind === "error") transport.fault = "list";
  if (kind === "identity") transport.fault = "user";
  if (kind === "loading") transport.gate = new Promise<void>(() => {});
  const { client, store, ready } = readyStore(transport);
  await ready;
  const providers = new ProviderStore();
  providers.load(
    client,
    providerFixtureRepositories.map((repo) => ({ ...repo, localPath: null })),
    [],
  );
  for (let i = 0; i < 10; i++) await Bun.sleep(0);
  if (kind === "browser") await providers.openBrowser(client);
  if (kind.startsWith("approval")) {
    if (kind.includes("bitbucket")) {
      providers.move(4);
    }
    await providers.previewApproval(client);
    if (kind.includes("uncertain")) {
      transport.approvalFault = "uncertain";
      await providers.confirmApproval(client);
      transport.approvalHeadSha = "c".repeat(40);
      await providers.approvalStatus(client, false);
    }
  }
  if (kind === "detail") {
    providers.showDetail(client);
    for (let i = 0; i < 6; i++) await Bun.sleep(0);
  }
  return {
    client,
    store,
    view: (
      <ProviderPanel
        store={providers}
        browserAvailable
        approvalAvailable
        client={client}
        onClose={() => {}}
        onWorkspace={async () => {}}
        onSettings={() => {}}
        quit={() => {}}
      />
    ),
  };
}

async function providerPreviewScene(kind: "github" | "bitbucket" | "saved" | "error" | "image") {
  const transport = new FakeProviderTransport();
  transport.images = kind === "image";
  const { client, store, ready } = readyStore(transport);
  await ready;
  if (kind === "error") transport.fault = "snapshot";
  await store.selectProviderPr(
    providerFixtureRepositories[kind === "bitbucket" ? 1 : 0]!,
    kind === "saved" ? 7 : 8,
    "Owned native preview",
    "OPEN",
  );
  if (kind === "image") {
    store.cycleFocus();
    store.move(1);
    for (let i = 0; i < 10; i++) await Bun.sleep(0);
  }
  return { client, store };
}

async function profileScene(kind: "catalog" | "base" | "error" | "loading") {
  const transport = new FakeProfileTransport({ repositories });
  if (kind === "base") {
    transport.profiles = [];
    transport.defaultProfile = null;
    transport.source = "base";
    transport.notice = "No local checkout is configured. The native base profile is available.";
  }
  if (kind === "error") transport.error = "The repository review policy is invalid.";
  if (kind === "loading") transport.gate = new Promise<void>(() => {});
  const { client, store, ready } = readyStore(transport);
  await ready;
  if (kind === "loading") void store.openReviewProfiles(client);
  else await store.openReviewProfiles(client);
  return { client, store };
}

async function localComparisonScene(
  kind: "source" | "base" | "layers" | "loading" | "error" | "profile",
) {
  const transport = new LocalTransport();
  const { client, store, ready } = readyStore(transport);
  await ready;
  if (kind === "profile") {
    await store.openReviewProfiles(client);
    return { client, store };
  }
  const request = transport.request.bind(transport);
  if (kind === "loading" || kind === "error")
    transport.request = async (method, params) => {
      if (method === "local.options") {
        if (kind === "error")
          throw new Error(
            "The local source could not be verified. Refresh or select another source.",
          );
        return new Promise(() => {});
      }
      return request(method, params);
    };
  if (kind === "loading") {
    void store.openLocalComparison(client);
    return { client, store };
  }
  await store.openLocalComparison(client);
  if (kind !== "source") store.cycleLocalField();
  if (kind === "layers") {
    store.cycleLocalField();
    store.toggleLocalLayer();
  }
  return { client, store };
}

async function savedLocalScene(kind: "list" | "next" | "empty" | "loading" | "error") {
  const transport = new SavedTransport();
  const { client, store, ready } = readyStore(transport);
  await ready;
  const request = transport.request.bind(transport);
  if (kind === "empty" || kind === "loading" || kind === "error")
    transport.request = async (method, params) => {
      if (method === "local.snapshot.list") {
        if (kind === "loading") return new Promise(() => {});
        if (kind === "error")
          throw new Error(
            "Saved local evidence could not be verified. Refresh the list explicitly.",
          );
        return {
          request: params,
          catalogVersion: "e".repeat(64),
          snapshots: [],
          total: 0,
          nextOffset: null,
        };
      }
      return request(method, params);
    };
  if (kind === "loading") void store.openSavedLocal(client);
  else await store.openSavedLocal(client);
  if (kind === "next") await store.pageSavedLocal(client, 1);
  return { client, store };
}
const scenes: Scene[] = [
  ...(
    [
      "tree",
      "content",
      "diff",
      "blame",
      "markdown",
      "find",
      "binary",
      "deleted",
      "empty",
      "error",
      "loading",
    ] as const
  ).flatMap((kind) =>
    [120, 50].map((cols) => ({
      id: `explorer-${kind}-${cols}x${cols === 120 ? 30 : 20}`,
      cols,
      rows: cols === 120 ? 30 : 20,
      build: async () => {
        const fixture = new ExplorerFixture();
        const { client, store, ready } = readyStore(fixture);
        await ready;
        const explorer = new ExplorerStore();
        if (kind === "empty") fixture.files = [];
        if (kind === "loading")
          fixture.nextGate = {
            method: "repository.explore.info",
            wait: new Promise<void>(() => {}),
          };
        const loaded = explorer.load(
          client,
          { provider: "github", workspace: "owned", repo: "same" },
          kind !== "error",
        );
        if (kind !== "loading") await loaded;
        if (!["tree", "empty", "error", "loading"].includes(kind)) {
          const path =
            kind === "binary"
              ? "images/binary.png"
              : kind === "deleted"
                ? "deleted.txt"
                : kind === "find"
                  ? "src/--modified ü.ts"
                  : "README.md";
          explorer.move(treeRows(explorer.getSnapshot()).findIndex((row) => row.path === path));
          await explorer.choose();
          if (kind === "diff" || kind === "blame") await explorer.setMode(kind);
          if (kind === "markdown") {
            await getTreeSitterClient().preloadParser("markdown");
            await getTreeSitterClient().preloadParser("markdown_inline");
            explorer.toggleMarkdown();
          }
          if (kind === "find") {
            explorer.openInput("find");
            explorer.appendInput("target_after_first_window");
            await explorer.applyInput();
          }
        }
        return {
          store,
          client,
          view: (
            <ExplorerPanel
              store={explorer}
              client={client}
              editorAvailable={false}
              width={cols}
              height={cols === 120 ? 30 : 20}
              onClose={() => explorer.dispose()}
              onSettings={() => {}}
              quit={() => {}}
            />
          ),
        };
      },
    })),
  ),
  ...(["launched", "failed", "unavailable", "busy"] as const).flatMap((kind) =>
    [120, 50].map((cols) => ({
      id: `explorer-editor-${kind}-${cols}x${cols === 120 ? 30 : 20}`,
      cols,
      rows: cols === 120 ? 30 : 20,
      build: async () => {
        const fixture = new EditorExplorerFixture();
        fixture.state = kind === "failed" ? "failed" : "launched";
        const { client, store, ready } = readyStore(fixture);
        await ready;
        const explorer = new ExplorerStore();
        await explorer.load(client, { provider: "github", workspace: "owned", repo: "same" }, true);
        await explorer.choose();
        return {
          store,
          client,
          view: (
            <ExplorerPanel
              store={explorer}
              client={client}
              editorAvailable={kind !== "unavailable"}
              editorBusy={kind === "busy"}
              width={cols}
              height={cols === 120 ? 30 : 20}
              onClose={() => explorer.dispose()}
              onSettings={() => {}}
              quit={() => {}}
            />
          ),
          afterMount: async (test: Awaited<ReturnType<typeof createTestRenderer>>) => {
            test.mockInput.pressKey("o");
            for (let pass = 0; pass < 10; pass++) {
              await Bun.sleep(0);
              await test.renderOnce();
            }
          },
        };
      },
    })),
  ),
  ...(
    ["branches", "preview", "busy", "failed", "uncertain", "canceled", "unavailable"] as const
  ).flatMap((kind) =>
    [120, 50].map((cols) => ({
      id: `repository-operations-${kind}-${cols}x${cols === 120 ? 30 : 20}`,
      cols,
      rows: cols === 120 ? 30 : 20,
      build: async () => {
        const fixture = new RepositoryOperationsFixture();
        const { client, store, ready } = readyStore(fixture);
        await ready;
        const operations = new RepositoryOperationsStore(client);
        await operations.load(
          { provider: "github", workspace: "owned", repo: "same" },
          kind !== "unavailable",
        );
        if (kind !== "branches" && kind !== "unavailable")
          await operations.preview({ kind: "fetch" });
        if (["busy", "failed", "uncertain", "canceled"].includes(kind)) {
          fixture.hold = kind === "busy";
          fixture.finalState =
            kind === "failed" || kind === "uncertain" || kind === "canceled" ? kind : "completed";
          await operations.confirm();
        }
        return {
          store,
          client,
          node: (
            <RepositoryOperationsPanel
              store={operations}
              width={cols}
              height={cols === 120 ? 30 : 20}
              onClose={() => {}}
              quit={() => {}}
            />
          ),
          afterMount: async () => {
            operations.dispose();
          },
        };
      },
    })),
  ),
  ...(["comparison", "added", "legacy", "unsupported"] as const).flatMap((kind) => [
    {
      id: `shell-local-image-${kind}-120x30`,
      cols: 120,
      rows: 30,
      build: async () => {
        const { client, store, ready } = readyStore(new LocalRasterTransport(kind));
        await ready;
        for (let i = 0; i < 12; i++) await Bun.sleep(0);
        return { client, store };
      },
    },
    {
      id: `shell-local-image-${kind}-50x18`,
      cols: 50,
      rows: 18,
      build: async () => {
        const { client, store, ready } = readyStore(new LocalRasterTransport(kind));
        await ready;
        for (let i = 0; i < 12; i++) await Bun.sleep(0);
        return { client, store };
      },
    },
  ]),
  ...(["list", "next", "empty", "loading", "error"] as const).flatMap((kind) => [
    {
      id: `shell-saved-local-${kind}-120x30`,
      cols: 120,
      rows: 30,
      build: () => savedLocalScene(kind),
    },
    {
      id: `shell-saved-local-${kind}-50x18`,
      cols: 50,
      rows: 18,
      build: () => savedLocalScene(kind),
    },
  ]),
  ...(["source", "base", "layers", "loading", "error", "profile"] as const).flatMap((kind) => [
    {
      id: `shell-local-${kind}-120x30`,
      cols: 120,
      rows: 30,
      build: () => localComparisonScene(kind),
    },
    {
      id: `shell-local-${kind}-50x18`,
      cols: 50,
      rows: 18,
      build: () => localComparisonScene(kind),
    },
  ]),
  ...(["catalog", "base", "error", "loading"] as const).flatMap((kind) => [
    { id: `shell-profiles-${kind}-120x30`, cols: 120, rows: 30, build: () => profileScene(kind) },
    { id: `shell-profiles-${kind}-50x18`, cols: 50, rows: 18, build: () => profileScene(kind) },
  ]),
  ...(
    [
      "approval",
      "approval-uncertain",
      "approval-bitbucket",
      "approval-bitbucket-uncertain",
    ] as const
  ).flatMap((kind) => [
    { id: `shell-provider-${kind}-120x30`, cols: 120, rows: 30, build: () => providerScene(kind) },
    { id: `shell-provider-${kind}-50x22`, cols: 50, rows: 22, build: () => providerScene(kind) },
  ]),
  {
    id: "shell-provider-browser-disabled-120x30",
    cols: 120,
    rows: 30,
    build: () => providerScene("browser"),
  },
  {
    id: "shell-provider-browser-disabled-50x15",
    cols: 50,
    rows: 15,
    build: () => providerScene("browser"),
  },
  ...(["github", "bitbucket", "saved", "error", "image"] as const).flatMap((kind) => [
    {
      id: `shell-provider-preview-${kind}-120x30`,
      cols: 120,
      rows: 30,
      build: () => providerPreviewScene(kind),
    },
    {
      id: `shell-provider-preview-${kind}-50x15`,
      cols: 50,
      rows: 15,
      build: () => providerPreviewScene(kind),
    },
  ]),
  ...(["list", "detail", "empty", "error", "loading", "identity"] as const).flatMap((kind) => [
    { id: `shell-provider-${kind}-120x30`, cols: 120, rows: 30, build: () => providerScene(kind) },
    { id: `shell-provider-${kind}-50x15`, cols: 50, rows: 15, build: () => providerScene(kind) },
  ]),
  { id: "shell-imported-scopes-120x30", cols: 120, rows: 30, build: savedScopeScene },
  { id: "shell-imported-scopes-50x15", cols: 50, rows: 15, build: savedScopeScene },
  { id: "shell-unconfigured-drafts-120x30", cols: 120, rows: 30, build: importedDraftScene },
  { id: "shell-unconfigured-drafts-50x15", cols: 50, rows: 15, build: importedDraftScene },
  {
    id: "shell-publication-drafts-120x40",
    cols: 120,
    rows: 40,
    build: () => publicationScene("drafts"),
  },
  {
    id: "shell-publication-drafts-50x20",
    cols: 50,
    rows: 20,
    build: () => publicationScene("drafts"),
  },
  {
    id: "shell-publication-preview-120x40",
    cols: 120,
    rows: 40,
    build: () => publicationScene("preview"),
  },
  {
    id: "shell-publication-preview-50x20",
    cols: 50,
    rows: 20,
    build: () => publicationScene("preview"),
  },
  {
    id: "shell-publication-partial-120x40",
    cols: 120,
    rows: 40,
    build: () => publicationScene("partial"),
  },
  {
    id: "shell-publication-status-80x30",
    cols: 80,
    rows: 30,
    build: () => publicationScene("status"),
  },
  {
    id: "shell-publication-threads-120x30",
    cols: 120,
    rows: 30,
    build: () => publicationScene("threads"),
  },
  {
    id: "shell-publication-threads-50x20",
    cols: 50,
    rows: 20,
    build: () => publicationScene("threads"),
  },
  { id: "shell-onboarding-120x30", cols: 120, rows: 30, build: () => onboardingScene(false) },
  { id: "shell-onboarding-50x15", cols: 50, rows: 15, build: () => onboardingScene(false) },
  {
    id: "shell-onboarding-unsupported-80x24",
    cols: 80,
    rows: 24,
    build: () => onboardingScene(true),
  },
  { id: "shell-syntax-unified-120x30", cols: 120, rows: 30, build: () => syntaxScene("unified") },
  { id: "shell-syntax-split-120x30", cols: 120, rows: 30, build: () => syntaxScene("split") },
  { id: "shell-syntax-split-50x15", cols: 50, rows: 15, build: () => syntaxScene("split") },
  { id: "shell-syntax-plain-80x24", cols: 80, rows: 24, build: () => syntaxScene("plain") },
  { id: "shell-search-editing-120x30", cols: 120, rows: 30, build: () => searchScene("editing") },
  { id: "shell-search-none-80x24", cols: 80, rows: 24, build: () => searchScene("none") },
  { id: "shell-search-split-80x24", cols: 80, rows: 24, build: () => searchScene("split") },
  { id: "shell-search-split-50x15", cols: 50, rows: 15, build: () => searchScene("split") },
  { id: "shell-staging-choose-120x30", cols: 120, rows: 30, build: () => stagingScene("choose") },
  { id: "shell-staging-preview-120x40", cols: 120, rows: 40, build: () => stagingScene("preview") },
  { id: "shell-staging-preview-50x20", cols: 50, rows: 20, build: () => stagingScene("preview") },
  { id: "shell-staging-general-80x24", cols: 80, rows: 24, build: () => stagingScene("general") },
  {
    id: "shell-staging-unanchored-50x20",
    cols: 50,
    rows: 20,
    build: () => stagingScene("unanchored"),
  },
  { id: "shell-staging-staged-80x30", cols: 80, rows: 30, build: () => stagingScene("staged") },
  {
    id: "shell-staging-duplicate-80x30",
    cols: 80,
    rows: 30,
    build: () => stagingScene("duplicate"),
  },
  { id: "shell-staging-conflict-80x30", cols: 80, rows: 30, build: () => stagingScene("conflict") },
  {
    id: "shell-references-saved-120x40",
    cols: 120,
    rows: 40,
    build: () => referencesScene("saved"),
  },
  { id: "shell-drafts-saved-120x40", cols: 120, rows: 40, build: () => draftsScene("saved") },
  { id: "shell-drafts-saved-50x20", cols: 50, rows: 20, build: () => draftsScene("saved") },
  { id: "shell-drafts-editing-120x40", cols: 120, rows: 40, build: () => draftsScene("editing") },
  { id: "shell-drafts-conflict-80x30", cols: 80, rows: 30, build: () => draftsScene("conflict") },
  { id: "shell-drafts-remove-80x30", cols: 80, rows: 30, build: () => draftsScene("remove") },
  { id: "shell-drafts-local-50x15", cols: 50, rows: 15, build: () => draftsScene("local") },
  { id: "shell-references-saved-50x20", cols: 50, rows: 20, build: () => referencesScene("saved") },
  {
    id: "shell-references-editing-120x40",
    cols: 120,
    rows: 40,
    build: () => referencesScene("editing"),
  },
  {
    id: "shell-references-conflict-80x30",
    cols: 80,
    rows: 30,
    build: () => referencesScene("conflict"),
  },
  {
    id: "shell-references-remove-80x30",
    cols: 80,
    rows: 30,
    build: () => referencesScene("remove"),
  },
  { id: "shell-references-local-50x15", cols: 50, rows: 15, build: () => referencesScene("local") },
  { id: "shell-workspace-saved-120x30", cols: 120, rows: 30, build: () => workspaceScene("saved") },
  { id: "shell-workspace-dirty-80x24", cols: 80, rows: 24, build: () => workspaceScene("dirty") },
  {
    id: "shell-workspace-conflict-50x15",
    cols: 50,
    rows: 15,
    build: () => workspaceScene("conflict"),
  },
  {
    id: "shell-workspace-disposed-80x24",
    cols: 80,
    rows: 24,
    build: () => workspaceScene("disposed"),
  },
  { id: "shell-wrapped-anchor-120x30", cols: 120, rows: 30, build: wrappedAnchorScene },
  { id: "shell-wrapped-anchor-80x24", cols: 80, rows: 24, build: wrappedAnchorScene },
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
  {
    id: "shell-settings-imported-theme-120x40",
    cols: 120,
    rows: 40,
    build: () => settingsScene("imported-theme"),
  },
  {
    id: "shell-settings-imported-theme-50x20",
    cols: 50,
    rows: 20,
    build: () => settingsScene("imported-theme"),
  },
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
  { id: "shell-review-profile-120x30", cols: 120, rows: 30, build: reviewProfileScene },
  { id: "shell-review-profile-50x15", cols: 50, rows: 15, build: reviewProfileScene },
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
      await afterMount(test);
      await test.renderOnce();
    }
    for (let pass = 0; pass < 3; pass += 1) {
      await Bun.sleep(0);
      await test.renderOnce();
    }
    if (scene.id.startsWith("explorer-markdown-")) {
      const ready = () =>
        test.captureCharFrame().includes("Owned explorer") &&
        test.captureCharFrame().includes("Readable");
      const deadline = Date.now() + 3000;
      while (!ready() && Date.now() < deadline) {
        await Bun.sleep(10);
        await test.renderOnce();
      }
      if (!ready()) throw new Error(`Native Markdown did not render: ${scene.id}`);
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
