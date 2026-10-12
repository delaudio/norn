import { expect, test } from "bun:test";
import { ProviderStore, providerPrKey } from "../src/app/providerStore";
import { ShellStore } from "../src/app/store";
import { BackendClient } from "../src/transport/backendClient";
import {
  FakeProviderTransport,
  providerFixtureRepositories,
} from "../src/transport/fakeProviderTransport";

const repositories = providerFixtureRepositories.map((repository) => ({
  ...repository,
  localPath: null,
}));
const settle = async () => {
  for (let i = 0; i < 12; i++) await Bun.sleep(0);
};
function fixture() {
  const transport = new FakeProviderTransport();
  const client = new BackendClient(transport);
  const store = new ProviderStore();
  return { transport, client, store };
}

test("browser keeps provider groups, actual author identities and explicit page continuation", async () => {
  const { transport, client, store } = fixture();
  store.load(client, repositories, []);
  await settle();
  expect(store.getSnapshot().groups.map((group) => group.repository.provider)).toEqual([
    "github",
    "bitbucket",
  ]);
  expect(store.authors()).toHaveLength(4);
  expect(store.authors().filter((author) => author.me)).toHaveLength(2);
  expect(transport.maxReads).toBeLessThanOrEqual(2);
  expect(store.getSnapshot().groups.every((group) => group.page === 1)).toBe(true);
  store.cycleFilter(client, 2);
  await settle();
  expect(store.getSnapshot().filter).toBe("MERGED");
  expect(
    store.getSnapshot().groups.every((group) => !group.values.length && group.nextPage === 2),
  ).toBe(true);
  store.loadMore(client);
  await settle();
  expect(store.getSnapshot().groups[0]?.values[0]?.id).toBe(9);
  expect(store.getSnapshot().groups[1]?.page).toBe(1);
  const selected = store.getSnapshot().selected;
  store.refresh(client);
  await settle();
  expect(store.getSnapshot().selected).toBe(selected);
  expect(store.getSnapshot().groups[0]?.page).toBe(2);
  expect(store.authors()).toHaveLength(4);
  expect(transport.calls.every((call) => call.method.startsWith("provider."))).toBe(true);
  store.dispose();
  client.close();
});
test("rapid filter changes retain two in-flight reads and discard old generations", async () => {
  const { transport, client, store } = fixture();
  let release!: () => void;
  transport.gate = new Promise<void>((resolve) => {
    release = resolve;
  });
  store.load(client, repositories, []);
  for (let i = 0; i < 19; i++) store.cycleFilter(client, 1);
  expect(transport.activeReads).toBe(2);
  expect(transport.calls).toHaveLength(2);
  transport.gate = null;
  release();
  await settle();
  expect(transport.maxReads).toBe(2);
  expect(store.getSnapshot().filter).toBe("ALL");
  const lists = transport.calls.filter((call) => call.method === "provider.prs");
  expect(lists.slice(1).every((call) => call.params.filter === "ALL")).toBe(true);
  expect(
    store.getSnapshot().groups.every((group) => group.values.length === 2 && !group.loading),
  ).toBe(true);
  store.dispose();
  client.close();
});
test("author filtering uses provider and ID even for identical display names", async () => {
  const { transport, client, store } = fixture();
  store.load(client, repositories, []);
  await settle();
  store.cycleAuthor(client, 1);
  await settle();
  const author = store.getSnapshot().author!;
  expect(author.provider).toBe("bitbucket");
  expect(author.id).toBe("actual-me");
  expect(store.getSnapshot().groups).toHaveLength(1);
  expect(store.getSnapshot().groups[0]?.values.map((pr) => pr.id)).toEqual([7]);
  store.cycleAuthor(client, 1);
  await settle();
  expect(store.getSnapshot().author?.id).toBe("actual-other");
  expect(store.getSnapshot().groups[0]?.values.map((pr) => pr.id)).toEqual([8]);
  const requests = transport.calls.filter((call) => call.method === "provider.prs");
  expect(requests.at(-1)?.params.authorId).toBe("actual-other");
  store.dispose();
  client.close();
});
test("detail is full-scope fenced and closing disposes late responses", async () => {
  const { transport, client, store } = fixture();
  store.load(client, repositories, []);
  await settle();
  expect(store.getSnapshot().selected).toBe(providerPrKey(providerFixtureRepositories[0]!, 7));
  store.showDetail(client);
  await settle();
  expect(store.getSnapshot().detail?.browserUrl).toBe("https://github.com/synthetic/review/pull/7");
  expect(store.getSnapshot().detail?.detail.reviewers[0]?.approved).toBe(true);
  let release!: () => void;
  transport.gate = new Promise<void>((resolve) => {
    release = resolve;
  });
  store.showDetail(client);
  store.closeDetail();
  store.dispose();
  transport.gate = null;
  release();
  await settle();
  expect(store.getSnapshot().detail).toBeNull();
  expect(store.getSnapshot().groups).toEqual([]);
  expect(transport.calls.every((call) => call.method.startsWith("provider."))).toBe(true);
  client.close();
});
test("per-repository failures and unavailable current user recover explicitly", async () => {
  const { transport, client, store } = fixture();
  transport.fault = "user";
  store.load(client, repositories, []);
  await settle();
  expect(store.getSnapshot().userErrors).toHaveLength(2);
  expect(store.authors().some((author) => author.me)).toBe(false);
  transport.fault = "list";
  store.refresh(client);
  await settle();
  expect(store.getSnapshot().groups.every((group) => group.error && !group.loading)).toBe(true);
  transport.fault = null;
  store.refresh(client);
  await settle();
  expect(store.getSnapshot().userErrors).toEqual([]);
  expect(
    store.getSnapshot().groups.every((group) => !group.error && group.values.length === 2),
  ).toBe(true);
  store.dispose();
  client.close();
});
test("query editing bounds UTF-8 and graphemes without applying rejected paste", async () => {
  const { client, store } = fixture();
  store.beginQuery();
  store.append("世界👩‍💻");
  store.backspace();
  expect(store.getSnapshot().text).toBe("世界");
  store.append("\u001b[31m");
  store.append("é".repeat(200));
  expect(store.getSnapshot().text).toBe("世界");
  store.cancelQuery();
  expect(store.getSnapshot().query).toBe("");
  client.close();
});
test("large all-repository inventory is explicit and a chosen repository remains browsable", async () => {
  const { transport, client, store } = fixture();
  store.load(
    client,
    Array.from({ length: 101 }, (_, i) => ({ ...repositories[0]!, repo: `repo-${i}` })),
    [],
  );
  expect(store.getSnapshot().error).toContain("100 repositories");
  expect(transport.calls).toHaveLength(0);
  store.cycleRepository(client, 1);
  await settle();
  expect(store.getSnapshot().groups).toHaveLength(1);
  expect(store.getSnapshot().error).toBeNull();
  store.dispose();
  client.close();
});
test("unsupported backend and malformed scopes send no reads", () => {
  const { transport, client, store } = fixture();
  store.load(client, repositories, [], false);
  expect(store.getSnapshot().error).toContain("matching UI/backend");
  expect(transport.calls).toHaveLength(0);
  store.load(client, [{ ...repositories[0]!, repo: "../other" }], []);
  expect(transport.calls).toHaveLength(0);
  expect(store.getSnapshot().repositories).toEqual([]);
  client.close();
});
test("shell preserves real saved runs and admits new PRs without fabricated run IDs", async () => {
  const { transport, client } = fixture();
  const shell = new ShellStore();
  await shell.connect(client);
  await shell.selectProviderPr(providerFixtureRepositories[0]!, 7, "Remote metadata", "OPEN");
  expect(shell.getSnapshot().picker[shell.getSnapshot().selectedPicker]).toMatchObject({
    kind: "review",
    ...providerFixtureRepositories[0],
    prId: 7,
    runId: "real-saved-run",
  });
  await shell.selectProviderPr(providerFixtureRepositories[1]!, 7, "Other provider", "OPEN");
  expect(shell.getSnapshot().picker[shell.getSnapshot().selectedPicker]).toMatchObject({
    kind: "review",
    ...providerFixtureRepositories[1],
    prId: 7,
    runId: null,
  });
  const settings = await client.settings();
  await shell.applySettings(client, settings.settings);
  expect(shell.getSnapshot().picker[shell.getSnapshot().selectedPicker]).toMatchObject({
    kind: "review",
    ...providerFixtureRepositories[1],
    prId: 7,
    runId: null,
  });
  expect(transport.calls.some((call) => call.method === "review.start")).toBe(false);
  await expect(
    shell.selectProviderPr(providerFixtureRepositories[0]!, 0x80000000, "local", "OPEN"),
  ).rejects.toThrow("identity");
  shell.cancelSyntax();
  client.close();
});
test("opening PR browser does not clear a pending reply", async () => {
  const { client } = fixture();
  const shell = new ShellStore();
  await shell.connect(client);
  await shell.selectProviderPr(providerFixtureRepositories[0]!, 7, "Saved", "OPEN");
  shell.beginReply();
  shell.appendReplyText("Unsaved 世界");
  shell.toggleProvider();
  expect(shell.getSnapshot().providerOpen).toBe(false);
  expect(shell.getSnapshot().composerText).toBe("Unsaved 世界");
  await expect(
    shell.selectProviderPr(providerFixtureRepositories[1]!, 8, "New", "OPEN"),
  ).rejects.toThrow("editor");
  expect(shell.getSnapshot().picker[shell.getSnapshot().selectedPicker]?.kind).toBe("review");
  shell.cancelSyntax();
  client.close();
});

test("provider browser requests coalesce and late results cannot overwrite another selection or disposed view", async () => {
  for (const changed of ["selection", "disposed"]) {
    let release!: () => void;
    const gate = new Promise<void>((resolve) => {
      release = resolve;
    });
    class BrowserTransport extends FakeProviderTransport {
      async request(method: string, params: Record<string, unknown>) {
        if (method === "provider.browser") {
          this.calls.push({ method, params });
          await gate;
          const repository = params.repository as (typeof providerFixtureRepositories)[number];
          return {
            request: params,
            repository,
            url: `https://github.com/${repository.workspace}/${repository.repo}/pull/${params.prId}`,
            launchRequested: true,
            launchError: null,
          };
        }
        return super.request(method, params);
      }
    }
    const transport = new BrowserTransport();
    const client = new BackendClient(transport);
    const store = new ProviderStore();
    store.load(client, repositories, []);
    await settle();
    const pending = store.openBrowser(client);
    await store.openBrowser(client);
    expect(transport.calls.filter((call) => call.method === "provider.browser")).toHaveLength(1);
    if (changed === "selection") {
      store.move(1);
      store.notice("New selection owns this message.");
    } else store.dispose();
    release();
    await pending;
    expect(store.getSnapshot().notice).toBe(
      changed === "selection" ? "New selection owns this message." : null,
    );
    expect(store.getSnapshot().browserOpening).toBe(false);
    store.dispose();
    client.close();
  }
});
