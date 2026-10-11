import { describe, expect, it } from "vitest";
import { collectDesktopData } from "./desktopDataExport";

function storage(items: Record<string, string>) {
  const keys = Object.keys(items);
  return {
    length: keys.length,
    key: (index: number) => keys[index] ?? null,
    getItem: (key: string) => items[key] ?? null,
  };
}
describe("desktop data export", () => {
  it("exports canonical and legacy user content without credentials or caches or source writes", () => {
    const source = {
      "norn.drafts.v1.github:owner/repo/7": "[]",
      "lachesi.reviewPrompt.owner/repo": "custom policy",
      "norn.viewedFiles.v1.owner/repo#7": '["a.ts"]',
      "norn.theme.v1": "dark",
      "credential.token": "excluded-sensitive-value",
      "norn.menuBarSnapshot.v1": "cache",
    };
    const before = JSON.stringify(source);
    const bundle = collectDesktopData(storage(source));
    expect(bundle.entries).toHaveLength(4);
    expect(JSON.stringify(bundle)).not.toContain("excluded-sensitive-value");
    expect(JSON.stringify(bundle)).not.toContain("cache");
    expect(JSON.stringify(source)).toBe(before);
  });
  it("refuses a changing or oversized source without returning partial data", () => {
    expect(() =>
      collectDesktopData({ length: 1, key: () => "norn.theme.v1", getItem: () => null }),
    ).toThrow("changed");
    expect(() =>
      collectDesktopData(
        storage({ "norn.reviewPrompt.v1.owner/repo": "x".repeat(256 * 1024 + 1) }),
      ),
    ).toThrow("limit");
  });
});
