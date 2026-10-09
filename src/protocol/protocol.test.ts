import { readdirSync, readFileSync } from "node:fs";

import { describe, expect, it } from "vitest";

import {
  decodeLine,
  encodeLine,
  isValidMessage,
  MAX_FRAME_BYTES,
  PROTOCOL_VERSION,
} from "./validate";

function readFixture(relative: string): unknown {
  return JSON.parse(readFileSync(`protocol/fixtures/${relative}`, "utf8"));
}

function fixtureNames(kind: "valid" | "invalid"): string[] {
  return readdirSync(`protocol/fixtures/${kind}`)
    .filter((name) => name.endsWith(".json"))
    .sort();
}

describe("norn stdio protocol", () => {
  it("exposes the version and frame limit from the schema", () => {
    expect(PROTOCOL_VERSION).toBe(1);
    expect(MAX_FRAME_BYTES).toBe(1024 * 1024);
  });

  it("accepts every valid fixture and rejects every invalid fixture", () => {
    for (const name of fixtureNames("valid")) {
      const fixture = readFixture(`valid/${name}`);
      expect(isValidMessage(fixture), `${name} should be valid`).toBe(true);
      expect(decodeLine(`${JSON.stringify(fixture)}\n`), `${name} should decode`).not.toBeNull();
    }
    for (const name of fixtureNames("invalid")) {
      const fixture = readFixture(`invalid/${name}`);
      expect(isValidMessage(fixture), `${name} should be invalid`).toBe(false);
      expect(decodeLine(`${JSON.stringify(fixture)}\n`), `${name} should not decode`).toBeNull();
    }
  });

  it("rejects malformed, empty, and oversized frames", () => {
    expect(decodeLine("{not json}")).toBeNull();
    expect(decodeLine("\n")).toBeNull();
    expect(decodeLine("")).toBeNull();
    expect(decodeLine("a".repeat(MAX_FRAME_BYTES + 1))).toBeNull();
  });

  it("rejects unknown methods, enums, extra fields, and version mismatches", () => {
    expect(isValidMessage({ type: "request", id: "r", method: "diff.unknown", params: {} })).toBe(
      false,
    );
    expect(
      isValidMessage({
        type: "response",
        id: "r",
        ok: false,
        error: { code: "boom", message: "nope" },
      }),
    ).toBe(false);
    expect(
      isValidMessage({ type: "hello", protocolVersion: 2, client: { name: "n", version: "1" } }),
    ).toBe(false);
    expect(
      isValidMessage({
        type: "hello",
        protocolVersion: 1,
        client: { name: "n", version: "1" },
        extra: true,
      }),
    ).toBe(false);
  });

  it("encodes a message as a single frame even with embedded newlines", () => {
    const frame = encodeLine({
      type: "event",
      operationId: "op-1",
      sequence: 1,
      target: { kind: "local", localSnapshotSha256: "a".repeat(64) },
      event: { kind: "progress", message: "line one\nline two ✓", completed: 1, total: 3 },
    });
    expect(frame.endsWith("\n")).toBe(true);
    expect(frame.match(/\n/g)?.length).toBe(1);
    expect(decodeLine(frame)).not.toBeNull();
  });
});
