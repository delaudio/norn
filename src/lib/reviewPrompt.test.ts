import { describe, expect, it } from "vitest";
import { DEFAULT_REVIEW_PROMPT, getReviewPrompt, setReviewPrompt } from "@/lib/reviewPrompt";
import type { KeyValueStorage } from "@/lib/storageMigration";

function fakeStorage() {
  const values = new Map<string, string>();
  const storage: KeyValueStorage = {
    getItem: (key: string) => values.get(key) ?? null,
    setItem: (key: string, value: string) => values.set(key, value),
  };
  return { storage, values };
}

describe("DEFAULT_REVIEW_PROMPT", () => {
  it("guides Claude to inspect local references and handle documentation diffs", () => {
    expect(DEFAULT_REVIEW_PROMPT).toContain("inspect any manual reference with a local path");
    expect(DEFAULT_REVIEW_PROMPT).toContain("documentation or conventions only");
    expect(DEFAULT_REVIEW_PROMPT).toContain("Do not invent runtime bugs");
  });

  it("requires a stable machine-readable findings schema", () => {
    expect(DEFAULT_REVIEW_PROMPT).toContain('"schemaVersion": "norn.review.v1"');
    expect(DEFAULT_REVIEW_PROMPT).toContain('"severity": "critical|major|minor|nit"');
    expect(DEFAULT_REVIEW_PROMPT).toContain('"confidence": "low|medium|high"');
    expect(DEFAULT_REVIEW_PROMPT).toContain("Use an empty `findings` array");
  });
});

describe("review prompt identity migration", () => {
  it("is safe when browser storage is unavailable", () => {
    expect(getReviewPrompt("acme/widgets", null)).toBe("");
    expect(() => setReviewPrompt("acme/widgets", "Prompt", null)).not.toThrow();
  });

  it("clears a migrated prompt with a canonical tombstone and retains the legacy source", () => {
    const fixture = fakeStorage();
    fixture.storage.setItem("lachesi.reviewPrompt.acme/widgets", "Legacy prompt");

    expect(getReviewPrompt("acme/widgets", fixture.storage)).toBe("Legacy prompt");
    setReviewPrompt("acme/widgets", "", fixture.storage);

    expect(getReviewPrompt("acme/widgets", fixture.storage)).toBe("");
    expect(fixture.values.get("norn.reviewPrompt.v1.acme/widgets")).toBe("");
    expect(fixture.values.get("lachesi.reviewPrompt.acme/widgets")).toBe("Legacy prompt");
  });
});
