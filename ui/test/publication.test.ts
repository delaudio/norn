import { expect, test } from "bun:test";
import { PublicationStore } from "../src/app/publicationStore";
import { BackendClient } from "../src/transport/backendClient";
import {
  FakePublicationTransport,
  publicationFixtureScope as scope,
} from "../src/transport/fakePublicationTransport";
import { validPublicationDTO } from "../src/transport/publicationValidation";

async function fixture() {
  const transport = new FakePublicationTransport();
  const client = new BackendClient(transport);
  const store = new PublicationStore();
  await store.load(client, scope);
  return { transport, client, store };
}
test("preview never publishes; one explicit approval sends selected items sequentially", async () => {
  const { transport, store } = await fixture();
  store.toggle();
  store.move(1);
  store.toggle();
  await store.prepare();
  expect(store.getSnapshot().preview?.items).toHaveLength(2);
  expect(transport.calls.filter((method) => method === "publication.apply")).toHaveLength(0);
  await store.approve();
  await store.approve();
  expect(transport.calls.filter((method) => method === "publication.apply")).toHaveLength(2);
  expect(store.getSnapshot().results.every((result) => result.draftRemoved)).toBe(true);
  expect(store.getSnapshot().drafts).toHaveLength(0);
  store.dispose();
});
test("uncertain first item stops the batch, preserves remaining drafts and reconciles read-only", async () => {
  const { transport, store } = await fixture();
  transport.uncertainAt = 1;
  store.toggle();
  store.move(1);
  store.toggle();
  await store.prepare();
  await store.approve();
  expect(transport.calls.filter((method) => method === "publication.apply")).toHaveLength(1);
  expect(store.getSnapshot().drafts).toHaveLength(2);
  expect(store.getSnapshot().results[0]?.entry.state).toBe("uncertain");
  await store.show("status");
  await store.reconcile();
  expect(transport.calls.filter((method) => method === "publication.apply")).toHaveLength(1);
  expect(store.getSnapshot().results[0]?.entry.state).toBe("published");
  store.dispose();
});
test("late preview after scope disposal is canceled and cannot be approved", async () => {
  const { transport, store } = await fixture();
  let release!: () => void;
  transport.holdPrepare = new Promise<void>((resolve) => {
    release = resolve;
  });
  store.toggle();
  const preparing = store.prepare();
  store.dispose();
  release();
  await preparing;
  await store.approve();
  expect(store.getSnapshot().preview).toBeNull();
  expect(transport.calls).toContain("publication.cancel");
  expect(transport.calls).not.toContain("publication.apply");
});
test("schema DTO validation rejects extra fields, invalid pins and spoofed approval scope", async () => {
  const { transport, client, store } = await fixture();
  store.toggle();
  await store.prepare();
  const preview = store.getSnapshot().preview!;
  expect(validPublicationDTO("PublicationPrepareResult", preview)).toBe(true);
  expect(validPublicationDTO("PublicationPrepareResult", { ...preview, forged: true })).toBe(false);
  expect(
    validPublicationDTO("PublicationPrepareResult", {
      ...preview,
      items: [
        { ...preview.items[0], comment: { ...preview.items[0]!.comment, headSha: "not-a-hash" } },
      ],
    }),
  ).toBe(false);
  const request = transport.request.bind(transport);
  transport.request = async (method, params) => {
    const result = await request(method, params);
    return method === "publication.prepare"
      ? { ...result, scope: { ...scope, provider: "bitbucket" } }
      : result;
  };
  await expect(client.preparePublication(scope, ["draft-1"])).rejects.toThrow(
    "Invalid publication response",
  );
  store.dispose();
});

test("canceling an in-flight item stops all remaining writes and retains draft content", async () => {
  const { transport, store } = await fixture();
  let release!: () => void;
  transport.holdApply = new Promise<void>((resolve) => {
    release = resolve;
  });
  store.toggle();
  store.move(1);
  store.toggle();
  await store.prepare();
  const applying = store.approve();
  await Bun.sleep(0);
  await store.cancel();
  release();
  await applying;
  expect(transport.calls.filter((method) => method === "publication.apply")).toHaveLength(1);
  expect(store.getSnapshot().drafts).toHaveLength(2);
  store.dispose();
});

test("actual root reply staging preserves parent, side, path and commit without sending", async () => {
  const { transport, client, store } = await fixture();
  const { DraftStore } = await import("../src/app/draftStore");
  const drafts = new DraftStore(
    () => 4,
    () => "reply-local",
  );
  await drafts.load(client, scope);
  await store.show("threads");
  drafts.reply(store.getSnapshot().comments[0]!);
  drafts.append("Reply body 世界\nSecond line");
  drafts.finishField();
  await drafts.save(client);
  expect(drafts.getSnapshot().items.find((draft) => draft.localId === "reply-local")).toMatchObject(
    {
      parentId: "123",
      from: 12,
      to: null,
      path: "src/example.ts",
      reviewHeadSha: "b".repeat(40),
      raw: "Reply body 世界\nSecond line",
      publicationMode: "inline",
    },
  );
  expect(transport.calls).not.toContain("publication.apply");
  drafts.dispose();
  store.dispose();
});
