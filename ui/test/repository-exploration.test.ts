import { describe, expect, test } from "bun:test";
import { validExploreParams, validExploreResult } from "../src/transport/explorationValidation";

const context = {
  repository: { provider: "github", workspace: "owned", repo: "same" },
  rootId: "a".repeat(64),
};
const params = { context, path: "--literal ü.txt", offset: 0, maxBytes: 4, contentSha256: null };
describe("qualified repository preview contract", () => {
  test("rejects arbitrary roots, traversal, unpinned continuations and byte limits", () => {
    expect(validExploreParams("RepositoryExploreContentParams", params)).toBe(true);
    for (const input of [
      { ...params, context: { ...context, absolutePath: "/owned" } },
      { ...params, path: "../outside" },
      { ...params, offset: 4 },
      { ...params, path: "🙂".repeat(1100) },
      { ...params, maxBytes: 32769 },
    ])
      expect(validExploreParams("RepositoryExploreContentParams", input)).toBe(false);
    expect(
      validExploreParams("RepositoryExploreContentParams", { ...params, path: "legitimate\nfile" }),
    ).toBe(true);
  });
  test("enforces exact context echo, UTF-8 page lengths and unavailable state", () => {
    const result = {
      request: params,
      state: "text",
      size: 6,
      contentSha256: "b".repeat(64),
      text: "é",
      nextOffset: 2,
    };
    expect(validExploreResult("RepositoryExploreContentPage", result, params)).toBe(true);
    for (const input of [
      { ...result, request: { ...params, context: { ...context, rootId: "c".repeat(64) } } },
      { ...result, nextOffset: 1 },
      { ...result, text: "🙂é" },
      { ...result, nextOffset: null },
      { ...result, state: "binary" },
    ])
      expect(validExploreResult("RepositoryExploreContentPage", input, params)).toBe(false);
    expect(
      validExploreResult(
        "RepositoryExploreContentPage",
        { ...result, state: "binary", text: "", nextOffset: null },
        params,
      ),
    ).toBe(true);
  });
  test("uses UTF-8 inventory ordering and rejects duplicate or wrong-filter rows", () => {
    const request = { context, offset: 0, limit: 32, query: "", changedOnly: false, version: null };
    const result = {
      request,
      version: "b".repeat(64),
      total: 2,
      files: ["\uffff", "🙂"].map((path) => ({ path, oldPath: null, status: "unchanged" })),
      nextOffset: null,
    };
    expect(validExploreResult("RepositoryExploreFilesPage", result, request)).toBe(true);
    expect(
      validExploreResult(
        "RepositoryExploreFilesPage",
        { ...result, files: [result.files[0], result.files[0]] },
        request,
      ),
    ).toBe(false);
    const changed = { ...request, changedOnly: true };
    expect(
      validExploreResult("RepositoryExploreFilesPage", { ...result, request: changed }, changed),
    ).toBe(false);
  });
  test("rejects blame from another line range and inconsistent find counts", () => {
    const request = { context, path: "file", offset: 3, limit: 2, contentSha256: "b".repeat(64) };
    const row = {
      line: 4,
      sha: "d".repeat(40),
      shortSha: "d".repeat(8),
      author: "Owned",
      authorEmail: null,
      authorTime: null,
      summary: null,
      message: "Full message",
    };
    const result = {
      request,
      baseSha: "d".repeat(40),
      available: true,
      totalLines: 5,
      lines: [row],
      nextOffset: 4,
    };
    expect(validExploreResult("RepositoryExploreBlamePage", result, request)).toBe(true);
    expect(
      validExploreResult(
        "RepositoryExploreBlamePage",
        { ...result, lines: [{ ...row, line: 3 }] },
        request,
      ),
    ).toBe(false);
    const find = { ...request, source: "content", query: "owned", offset: 0 };
    const found = {
      request: find,
      total: 1,
      matches: [{ startUtf16: 2, endUtf16: 7, startByte: 2, endByte: 7, line: 1, columnUtf16: 2 }],
      nextOffset: null,
    };
    expect(validExploreResult("RepositoryExploreFindPage", found, find)).toBe(true);
    expect(validExploreResult("RepositoryExploreFindPage", { ...found, total: 2 }, find)).toBe(
      false,
    );
  });
});
