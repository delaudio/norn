import { expect, test } from "bun:test";
import { SettingsStore } from "../src/app/settingsStore";
import { ShellStore } from "../src/app/store";
import { BackendClient } from "../src/transport/backendClient";
import { FakeBackendTransport } from "../src/transport/fakeTransport";

test("first run and existing credentials are status-only and never start an operation", async () => {
  const calls: string[] = [];
  class Transport extends FakeBackendTransport {
    async request(method: string, params: Record<string, unknown>) {
      calls.push(method);
      return super.request(method, params);
    }
  }
  const client = new BackendClient(
    new Transport({
      repositories: [],
      availableTools: ["git"],
      credentialStore: "unsupported",
      credentials: [{ provider: "github", available: true, source: "environment" }],
    }),
  );
  const shell = new ShellStore();
  await shell.connect(client);
  expect(shell.getSnapshot().readiness?.configuredRepos).toBe(0);
  expect(
    shell.getSnapshot().readiness?.tools.find((tool) => tool.name === "claude")?.available,
  ).toBe(false);
  const settings = new SettingsStore();
  await settings.load(client);
  expect(settings.getSnapshot().credentials[0]?.source).toBe("environment");
  expect(settings.getSnapshot().token).toBe("");
  expect(settings.getSnapshot().readiness?.credentialStore).toBe("unsupported");
  expect(
    calls.some((method) => ["review.start", "auth.set", "settings.save"].includes(method)),
  ).toBe(false);
  settings.dispose();
  shell.markClosed();
});

test("status refresh coalesces input, preserves unsaved edits and fences disposed forms", async () => {
  let requests = 0;
  class Transport extends FakeBackendTransport {
    async request(method: string, params: Record<string, unknown>) {
      if (method === "workspace.readiness") {
        requests++;
        await Bun.sleep(10);
      }
      return super.request(method, params);
    }
  }
  const client = new BackendClient(new Transport());
  const settings = new SettingsStore();
  await settings.load(client);
  settings.select("claudeModel");
  settings.beginEdit();
  settings.clearEdit();
  settings.append("custom/model");
  settings.finishEdit();
  requests = 0;
  await Promise.all(Array.from({ length: 20 }, () => settings.refreshReadiness(client)));
  expect(requests).toBe(1);
  expect(settings.getSnapshot().draft?.claudeModel).toBe("custom/model");
  const pending = settings.refreshReadiness(client);
  settings.dispose();
  await pending;
  expect(settings.getSnapshot().readiness).toBeNull();
  expect(settings.getSnapshot().readinessLoading).toBe(false);
  expect(settings.getSnapshot().draft).toBeNull();
});

test("malformed and duplicate tool readiness cannot enter UI state", async () => {
  class BadTransport extends FakeBackendTransport {
    async request(method: string, params: Record<string, unknown>) {
      const result = await super.request(method, params);
      if (method === "workspace.readiness")
        result.tools = Array.from({ length: 4 }, () => ({ name: "git", available: true }));
      return result;
    }
  }
  await expect(new BackendClient(new BadTransport()).readiness()).rejects.toThrow("invalid");
});
