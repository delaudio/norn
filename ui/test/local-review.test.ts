import { expect, test } from "bun:test";
import findingsFixture from "../../protocol/fixtures/valid/response-local-review-findings.json";
import historyFixture from "../../protocol/fixtures/valid/response-local-review-history.json";
import outputFixture from "../../protocol/fixtures/valid/response-local-review-output.json";
import startFixture from "../../protocol/fixtures/valid/response-local-review-start.json";
import threadsFixture from "../../protocol/fixtures/valid/response-local-review-threads.json";
import type {
  LocalReviewFindingsParams,
  LocalReviewHistoryParams,
  LocalReviewQueryParams,
  LocalReviewStartParams,
} from "../../src/protocol/generated/protocol";
import { BackendClient } from "../src/transport/backendClient";
import { FakeBackendTransport } from "../src/transport/fakeTransport";

class Transport extends FakeBackendTransport {
  calls: string[] = [];
  replies = new Map<string, Record<string, unknown>>();
  override async request(
    method: string,
    _params: Record<string, unknown>,
  ): Promise<Record<string, unknown>> {
    this.calls.push(method);
    return structuredClone(this.replies.get(method) ?? {});
  }
}
function setup() {
  const transport = new Transport();
  for (const [method, fixture] of [
    ["start", startFixture],
    ["history", historyFixture],
    ["findings", findingsFixture],
    ["output", outputFixture],
    ["threads", threadsFixture],
  ] as const)
    transport.replies.set(`local.review.${method}`, structuredClone(fixture.result));
  return { transport, client: new BackendClient(transport) };
}
test("qualified native local facade echoes saved identity and selected run", async () => {
  const { client, transport } = setup();
  expect(
    (await client.localReviewStart(startFixture.result.request as LocalReviewStartParams)).target
      .localSnapshotSha256,
  ).toBe(startFixture.result.request.snapshot.snapshotId);
  expect(
    (await client.localReviewHistory(historyFixture.result.request as LocalReviewHistoryParams))
      .runs[0]!.runId,
  ).toBe("run-owned");
  expect(
    (await client.localReviewFindings(findingsFixture.result.request as LocalReviewFindingsParams))
      .total,
  ).toBe(0);
  expect(
    (await client.localReviewOutput(outputFixture.result.request as LocalReviewQueryParams)).text,
  ).toBe("Owned output.");
  expect(
    (await client.localReviewThreads(threadsFixture.result.request as LocalReviewQueryParams))
      .threads,
  ).toHaveLength(0);
  expect(transport.calls).toHaveLength(5);
});
test("local start rejects caller paths and invalid profiles before transport", async () => {
  const { client, transport } = setup();
  for (const params of [
    { ...startFixture.result.request, path: "/foreign" },
    { ...startFixture.result.request, profile: " security " },
    { ...startFixture.result.request, profile: "a\nb" },
    { ...startFixture.result.request, profile: "界".repeat(129) },
    {
      ...startFixture.result.request,
      snapshot: { ...startFixture.result.request.snapshot, rootId: "bad" },
    },
  ])
    await expect(client.localReviewStart(params as LocalReviewStartParams)).rejects.toThrow();
  expect(transport.calls).toHaveLength(0);
});
test("local responses reject changed root, operation snapshot, run and pagination", async () => {
  const { client, transport } = setup();
  const start = structuredClone(startFixture.result);
  start.target.localSnapshotSha256 = "d".repeat(64);
  transport.replies.set("local.review.start", start);
  await expect(
    client.localReviewStart(startFixture.result.request as LocalReviewStartParams),
  ).rejects.toThrow();
  const output = structuredClone(outputFixture.result);
  output.runId = "foreign-run";
  transport.replies.set("local.review.output", output);
  await expect(
    client.localReviewOutput(outputFixture.result.request as LocalReviewQueryParams),
  ).rejects.toThrow();
  output.runId = "run-owned";
  output.request.snapshot.rootId = "d".repeat(64);
  transport.replies.set("local.review.output", output);
  await expect(
    client.localReviewOutput(outputFixture.result.request as LocalReviewQueryParams),
  ).rejects.toThrow();
  const history = structuredClone(historyFixture.result);
  history.nextOffset = 0 as unknown as null;
  transport.replies.set("local.review.history", history);
  await expect(
    client.localReviewHistory(historyFixture.result.request as LocalReviewHistoryParams),
  ).rejects.toThrow();
});
test("local finding layers remain explicit and byte limits reject oversized Unicode output", async () => {
  const { client, transport } = setup();
  const finding = {
    id: "finding",
    title: "Layer ambiguity",
    severity: "high",
    summary: "Owned",
    anchor: { path: "same.txt", startLine: 1, endLine: null, side: "new" },
    candidateLayers: ["staged", "unstaged"],
  };
  const result = { ...structuredClone(findingsFixture.result), findings: [finding], total: 1 };
  transport.replies.set("local.review.findings", result);
  expect(
    (await client.localReviewFindings(findingsFixture.result.request as LocalReviewFindingsParams))
      .findings[0]!.candidateLayers,
  ).toEqual(["staged", "unstaged"]);
  for (const layers of [
    ["unstaged", "staged"],
    ["staged", "staged"],
  ]) {
    finding.candidateLayers = layers;
    transport.replies.set("local.review.findings", result);
    await expect(
      client.localReviewFindings(findingsFixture.result.request as LocalReviewFindingsParams),
    ).rejects.toThrow();
  }
  finding.candidateLayers = ["staged"];
  finding.anchor.path = "../foreign";
  transport.replies.set("local.review.findings", result);
  await expect(
    client.localReviewFindings(findingsFixture.result.request as LocalReviewFindingsParams),
  ).rejects.toThrow();
  transport.replies.set("local.review.output", {
    ...outputFixture.result,
    text: "界".repeat(22000),
  });
  await expect(
    client.localReviewOutput(outputFixture.result.request as LocalReviewQueryParams),
  ).rejects.toThrow();
});
test("local conversations reject private fields and contradictory selection", async () => {
  const { client, transport } = setup();
  const thread = {
    id: "thread",
    title: "Owned",
    latestRunId: "run-owned",
    selected: true,
    messages: [],
    truncated: false,
  };
  transport.replies.set("local.review.threads", {
    ...threadsFixture.result,
    threads: [{ ...thread, claudeSessionId: "private" }],
  });
  await expect(
    client.localReviewThreads(threadsFixture.result.request as LocalReviewQueryParams),
  ).rejects.toThrow();
  transport.replies.set("local.review.threads", {
    ...threadsFixture.result,
    threads: [thread, { ...thread, id: "second" }],
  });
  await expect(
    client.localReviewThreads(threadsFixture.result.request as LocalReviewQueryParams),
  ).rejects.toThrow();
});
