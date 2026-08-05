import { describe, expect, it } from "vitest";
import { type KeyValueStorage, readMigratedStorageValue } from "./storageMigration";

function fakeStorage(entries: Record<string, string>, failWrites = false) {
  const values = new Map(Object.entries(entries));
  let writes = 0;
  const storage: KeyValueStorage = {
    getItem: (key) => values.get(key) ?? null,
    setItem: (key, value) => {
      writes += 1;
      if (failWrites) throw new Error("unavailable");
      values.set(key, value);
    },
  };
  return { storage, values, writes: () => writes };
}

describe("browser storage identity migration", () => {
  it("prefers the canonical value without rewriting it", () => {
    const fixture = fakeStorage({ canonical: "new", legacy: "old" });

    expect(readMigratedStorageValue(fixture.storage, "canonical", "legacy")).toBe("new");
    expect(fixture.writes()).toBe(0);
  });

  it("copies a legacy value once and retains the source", () => {
    const fixture = fakeStorage({ legacy: "preserved" });

    expect(readMigratedStorageValue(fixture.storage, "canonical", "legacy")).toBe("preserved");
    expect(readMigratedStorageValue(fixture.storage, "canonical", "legacy")).toBe("preserved");
    expect(fixture.writes()).toBe(1);
    expect(fixture.values.get("legacy")).toBe("preserved");
  });

  it("keeps the legacy value usable when a canonical write fails", () => {
    const fixture = fakeStorage({ legacy: "recoverable" }, true);

    expect(readMigratedStorageValue(fixture.storage, "canonical", "legacy")).toBe("recoverable");
    expect(fixture.values.has("canonical")).toBe(false);
  });

  it("rejects a legacy value that fails validation without canonicalizing it", () => {
    const fixture = fakeStorage({ legacy: "not-json" });

    expect(
      readMigratedStorageValue(fixture.storage, "canonical", "legacy", (value) => {
        try {
          JSON.parse(value);
          return value;
        } catch {
          return null;
        }
      }),
    ).toBeNull();
    expect(fixture.writes()).toBe(0);
    expect(fixture.values.has("canonical")).toBe(false);
    expect(fixture.values.get("legacy")).toBe("not-json");
  });

  it("rejects a legacy value when its normalizer throws", () => {
    const fixture = fakeStorage({ legacy: "wrong-shape" });

    expect(
      readMigratedStorageValue(fixture.storage, "canonical", "legacy", () => {
        throw new Error("invalid shape");
      }),
    ).toBeNull();
    expect(fixture.writes()).toBe(0);
    expect(fixture.values.get("legacy")).toBe("wrong-shape");
  });
});
