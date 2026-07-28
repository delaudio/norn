import { describe, expect, it } from "vitest";
import { sampleRawDiff } from "@/storybook/bitbucket.fixtures";
import type { PrComment } from "@/types";
import { groupComments } from "./comments";
import { changeNewLine, fileKey, parseUnifiedDiff } from "./diff";

const files = parseUnifiedDiff(sampleRawDiff);
const file = files[0];

function firstInsertLine(): number {
  for (const hunk of file.hunks) {
    for (const change of hunk.changes) {
      const line = changeNewLine(change);
      if (change.type === "insert" && line) return line;
    }
  }
  throw new Error("no insert change in fixture diff");
}

function comment(overrides: Partial<PrComment>): PrComment {
  return {
    id: "1",
    parentId: null,
    contentRaw: "body",
    userDisplayName: "Reviewer",
    createdOn: "2026-01-01T00:00:00.000Z",
    deleted: false,
    inline: null,
    ...overrides,
  };
}

describe("groupComments", () => {
  it("anchors an inline comment to its file + change key", () => {
    const c = comment({
      id: "10",
      inline: { path: file.newPath, to: firstInsertLine(), from: null },
    });
    const { inlineByFile, unanchored } = groupComments(files, [c]);
    expect(unanchored).toHaveLength(0);
    expect(Object.keys(inlineByFile)).toContain(fileKey(file));
  });

  it("routes general (non-inline) comments to unanchored", () => {
    const { inlineByFile, unanchored } = groupComments(files, [comment({ id: "11" })]);
    expect(unanchored).toHaveLength(1);
    expect(Object.keys(inlineByFile)).toHaveLength(0);
  });

  it("routes inline comments on unknown files to unanchored", () => {
    const c = comment({ id: "12", inline: { path: "does/not/exist.ts", to: 1, from: null } });
    expect(groupComments(files, [c]).unanchored).toHaveLength(1);
  });

  it("routes file comments with no resolvable line to fileLevelByFile", () => {
    const fileLevel = comment({ id: "14", inline: { path: file.newPath, to: null, from: null } });
    const outOfDiff = comment({
      id: "15",
      inline: { path: file.newPath, to: 99999, from: null },
    });
    const { fileLevelByFile, unanchored } = groupComments(files, [fileLevel, outOfDiff]);
    expect(fileLevelByFile[fileKey(file)]).toHaveLength(2);
    expect(unanchored).toHaveLength(0);
  });

  it("ignores deleted comments", () => {
    const { unanchored } = groupComments(files, [comment({ id: "13", deleted: true })]);
    expect(unanchored).toHaveLength(0);
  });
});
