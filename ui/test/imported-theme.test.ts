import { expect, test } from "bun:test";
import { SettingsStore } from "../src/app/settingsStore";
import { ShellStore } from "../src/app/store";
import { BackendClient } from "../src/transport/backendClient";
import { FakeBackendTransport } from "../src/transport/fakeTransport";

class ThemeTransport extends FakeBackendTransport {
  methods: string[] = [];
  imported: "light" | "dark" = "light";
  revision = "a".repeat(64);
  available = true;
  hold: Promise<void> | null = null;
  async request(method: string, params: Record<string, unknown>) {
    this.methods.push(method);
    if (method === "workspace.read" && params.key === "theme") {
      if (this.hold) await this.hold;
      return {
        key: "theme",
        available: this.available,
        recordVersion: this.revision,
        payload: { kind: "theme", value: this.imported },
        offset: 0,
        nextOffset: null,
        totalItems: 1,
      };
    }
    return super.request(method, params);
  }
}
async function fixture() {
  const transport = new ThemeTransport();
  const client = new BackendClient(transport);
  const store = new SettingsStore();
  await store.load(client);
  transport.methods = [];
  return { transport, client, store };
}
function customModel(store: SettingsStore) {
  store.select("codexModel");
  store.beginEdit();
  store.clearEdit();
  store.append("vendor/custom:preserved");
  store.finishEdit();
}

test("imported theme needs preview, staging and explicit settings save, preserving other drafts/source", async () => {
  const { transport, client, store } = await fixture();
  customModel(store);
  await store.stageImportedTheme(client);
  expect(transport.methods).toEqual([]);
  await store.previewImportedTheme(client);
  expect(store.getSnapshot().themePreview).toMatchObject({ value: "light", draftTheme: "dark" });
  expect(store.getSnapshot().draft?.theme).toBe("dark");
  await store.stageImportedTheme(client);
  expect(store.getSnapshot().draft?.theme).toBe("light");
  expect(store.getSnapshot().draft?.codexModel).toBe("vendor/custom:preserved");
  expect(transport.methods).toEqual(["workspace.read", "workspace.read"]);
  expect((await client.settings()).settings.theme).toBe("dark");
  const saved = await store.save(client);
  expect(saved?.theme).toBe("light");
  expect(saved?.codexModel).toBe("vendor/custom:preserved");
  expect(transport.imported).toBe("light");
  expect(
    transport.methods.some((method) => method === "workspace.mutate" || method.startsWith("auth.")),
  ).toBe(false);
});

test("missing/changed imported theme and stale settings fail without replacing preferences", async () => {
  const { transport, client, store } = await fixture();
  transport.available = false;
  await store.previewImportedTheme(client);
  expect(store.getSnapshot().error).toContain("No imported theme");
  expect(store.getSnapshot().themePreview).toBeNull();
  transport.available = true;
  await store.previewImportedTheme(client);
  transport.revision = "b".repeat(64);
  await store.stageImportedTheme(client);
  expect(store.getSnapshot().error).toContain("changed");
  expect(store.getSnapshot().draft?.theme).toBe("dark");
  expect(store.getSnapshot().themePreview).toBeNull();
  await store.previewImportedTheme(client);
  await store.stageImportedTheme(client);
  const newer = await client.settings();
  await client.saveSettings(newer.version, { ...newer.settings, claudeModel: "newer/model" });
  expect(await store.save(client)).toBeNull();
  expect(store.getSnapshot().draft?.theme).toBe("light");
  expect((await client.settings()).settings.claudeModel).toBe("newer/model");
  expect((await client.settings()).settings.theme).toBe("dark");
});

test("preview dismissal, duplicate input and disposal fence pending theme reads", async () => {
  const { transport, client, store } = await fixture();
  await store.previewImportedTheme(client);
  store.dismissImportedTheme();
  await store.stageImportedTheme(client);
  expect(transport.methods).toEqual(["workspace.read"]);
  let release!: () => void;
  transport.hold = new Promise((resolve) => {
    release = resolve;
  });
  const pending = store.previewImportedTheme(client);
  await store.previewImportedTheme(client);
  expect(transport.methods.filter((method) => method === "workspace.read")).toHaveLength(2);
  expect(await store.save(client)).toBeNull();
  store.dispose();
  release();
  await pending;
  expect(store.getSnapshot().themePreview).toBeNull();
  expect(store.getSnapshot().draft).toBeNull();
  expect(store.getSnapshot().error).toBeNull();
  expect(transport.methods.some((method) => method === "settings.save")).toBe(false);
});

test("user-data overlay retains an open settings session instead of disposing its draft", () => {
  const shell = new ShellStore();
  shell.toggleSettings();
  shell.toggleData();
  expect(shell.getSnapshot()).toMatchObject({ settingsOpen: true, dataOpen: true });
  shell.toggleData();
  expect(shell.getSnapshot()).toMatchObject({ settingsOpen: true, dataOpen: false });
});
