import { expect, test } from "bun:test";
import { createTestRenderer } from "@opentui/core/testing";
import { createRoot } from "@opentui/react";
import { PublicationPanel } from "../src/app/components/PublicationPanel";
import { PublicationStore } from "../src/app/publicationStore";
import { BackendClient } from "../src/transport/backendClient";
import {
  FakePublicationTransport,
  publicationFixtureScope as scope,
} from "../src/transport/fakePublicationTransport";

test("native publication input fences paste, previews before approval and stages only actual root replies", async () => {
  const transport = new FakePublicationTransport();
  const client = new BackendClient(transport);
  const publication = new PublicationStore();
  await publication.load(client, scope);
  const renderer = await createTestRenderer({ width: 120, height: 40 });
  const root = createRoot(renderer.renderer);
  const replies: string[] = [];
  root.render(
    <PublicationPanel
      publication={publication}
      onClose={() => {}}
      onReply={(comment) => {
        replies.push(comment.id);
      }}
    />,
  );
  const settle = async () => {
    for (let i = 0; i < 5; i++) {
      await Bun.sleep(0);
      await renderer.renderOnce();
    }
  };
  try {
    await settle();
    await renderer.mockInput.pasteBracketedText(" y\r\n");
    await settle();
    expect(transport.calls).not.toContain("publication.prepare");
    expect(transport.calls).not.toContain("publication.apply");
    renderer.mockInput.pressKey(" ");
    renderer.mockInput.pressEnter();
    await settle();
    expect(renderer.captureCharFrame()).toContain("Exact preview");
    expect(renderer.captureCharFrame()).toContain("old 12");
    expect(renderer.captureCharFrame()).toContain("norn-draft:");
    await renderer.mockInput.pasteBracketedText("y");
    await settle();
    expect(transport.calls).not.toContain("publication.apply");
    renderer.mockInput.pressKey("y");
    await settle();
    expect(transport.calls.filter((method) => method === "publication.apply")).toHaveLength(1);
    renderer.mockInput.pressKey("t");
    await settle();
    expect(renderer.captureCharFrame()).toContain("Synthetic reviewer");
    renderer.mockInput.pressKey("r");
    await settle();
    expect(replies).toEqual(["123"]);
  } finally {
    publication.dispose();
    root.unmount();
    renderer.renderer.destroy();
  }
});
