import { expect, test } from "bun:test";
import { SettingsStore } from "../src/app/settingsStore";
import { BackendClient } from "../src/transport/backendClient";
import { FakeBackendTransport } from "../src/transport/fakeTransport";

async function fixture(transport = new FakeBackendTransport()) {
  const client = new BackendClient(transport);
  const store = new SettingsStore();
  await store.load(client);
  return { client, store, transport };
}
function edit(store: SettingsStore, id: string, value: string) {
  store.select(id);
  store.beginEdit();
  store.clearEdit();
  store.append(value);
  store.finishEdit();
}

test("settings preserve arbitrary custom models and discard cancelled field edits", async () => {
  const { client, store } = await fixture();
  edit(store, "claudeModel", "vendor/custom:thought-v4 ✓");
  const saved = await store.save(client);
  expect(saved?.claudeModel).toBe("vendor/custom:thought-v4 ✓");
  expect((await client.settings()).settings.claudeModel).toBe(saved?.claudeModel);
  store.select("claudeModel");
  store.beginEdit();
  store.clearEdit();
  store.append("discard this");
  store.cancelEdit();
  expect(store.getSnapshot().draft?.claudeModel).toBe(saved?.claudeModel);
  store.dispose();
  expect(store.getSnapshot().draft).toBeNull();
  expect((await client.settings()).settings.claudeModel).toBe(saved?.claudeModel);
});

test("stale saves retain the draft and cannot overwrite newer preferences", async () => {
  const { client, store } = await fixture();
  edit(store, "codexModel", "my-unsubmitted-model");
  const before = await client.settings();
  await client.saveSettings(before.version, { ...before.settings, codexModel: "newer-model" });
  expect(await store.save(client)).toBeNull();
  expect(store.getSnapshot().error).toContain("changed");
  expect(store.getSnapshot().draft?.codexModel).toBe("my-unsubmitted-model");
  expect((await client.settings()).settings.codexModel).toBe("newer-model");
});

test("credential edits are masked, separately saved, and never returned by status", async () => {
  const { client, store } = await fixture(
    new FakeBackendTransport({
      credentials: [{ provider: "github", available: true, source: "keychain" }],
    }),
  );
  expect(store.getSnapshot().token).toBe("");
  expect(store.getSnapshot().credentials[0]?.available).toBe(true);
  store.select("token");
  store.beginEdit();
  store.append("fixture-sensitive-value");
  expect(store.editDisplay()).not.toContain("fixture-sensitive-value");
  store.finishEdit();
  expect(JSON.stringify(store.fields())).not.toContain("fixture-sensitive-value");
  const before = await client.settings();
  await store.saveCredential(client);
  expect(store.getSnapshot().token).toBe("");
  expect((await client.settings()).version).toBe(before.version);
  expect(JSON.stringify(await client.credentialStatus())).not.toContain("fixture-sensitive-value");
  store.requestClear();
  store.dismissClear();
  await store.clearCredential(client);
  expect((await client.credentialStatus())[0]?.available).toBe(true);
  store.requestClear();
  await store.clearCredential(client);
  expect((await client.credentialStatus())[0]?.available).toBe(false);
});

test("credential failures redact input and oversized paste is rejected without truncating", async () => {
  class FailingCredentials extends FakeBackendTransport {
    async request(method: string, params: Record<string, unknown>) {
      if (method === "auth.set") throw new Error(`Cannot save ${params.token}`);
      return super.request(method, params);
    }
  }
  const { client, store } = await fixture(new FailingCredentials());
  edit(store, "token", "fixture-sensitive-value");
  await store.saveCredential(client);
  expect(store.getSnapshot().error).toContain("[redacted]");
  expect(store.getSnapshot().error).not.toContain("fixture-sensitive-value");
  store.select("token");
  store.beginEdit();
  store.append("x".repeat(8193));
  expect(store.getSnapshot().editText).toBe("fixture-sensitive-value");
  expect(store.getSnapshot().error).toContain("not applied");
  store.dispose();
  expect(store.getSnapshot().token).toBe("");
  expect(store.getSnapshot().editText).toBe("");
});

test("repository removal is a draft change and cancelled forms leave configuration intact", async () => {
  const { client, store } = await fixture(
    new FakeBackendTransport({
      repositories: [
        { provider: "github", workspace: "fixture", repo: "demo", localPath: "/tmp/clone" },
      ],
    }),
  );
  store.select("repo.0.workspace");
  store.removeRepository();
  expect(store.getSnapshot().draft?.repos).toHaveLength(0);
  expect(await client.repositories()).toHaveLength(1);
  store.dispose();
  await store.load(client);
  expect(store.getSnapshot().draft?.repos).toHaveLength(1);
  store.addRepository();
  edit(store, "repo.1.workspace", "other");
  edit(store, "repo.1.repo", "new");
  await store.save(client);
  expect(await client.repositories()).toHaveLength(2);
});
