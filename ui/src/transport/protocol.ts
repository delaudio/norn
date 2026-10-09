// Typed protocol surface for the OpenTUI client. The authoritative contract is
// the generated module at src/protocol/generated/protocol.ts (single source:
// protocol/norn-protocol.schema.json). This module re-exports it and adds a
// light defensive frame parser: the backend is our own trusted subprocess and
// Rust performs strict schema validation, so the client only needs a structural
// guard against drift.

import type { NornStdioProtocol } from "../../../src/protocol/generated/protocol";

export type {
  Event,
  Hello,
  NornStdioProtocol,
  ProtocolError,
  Ready,
  Request,
  ResponseError,
  ResponseOk,
  TargetIdentity,
} from "../../../src/protocol/generated/protocol";
export {
  MAX_FRAME_BYTES,
  PROTOCOL_VERSION,
} from "../../../src/protocol/generated/protocol";

const MESSAGE_TYPES = new Set(["hello", "ready", "request", "response", "event"]);

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

/// Parse one newline-delimited frame, returning null for anything malformed.
export function parseFrame(line: string): NornStdioProtocol | null {
  const trimmed = line.replace(/[\r\n]+$/, "");
  if (trimmed.length === 0) {
    return null;
  }
  let value: unknown;
  try {
    value = JSON.parse(trimmed);
  } catch {
    return null;
  }
  if (!isRecord(value) || typeof value.type !== "string" || !MESSAGE_TYPES.has(value.type)) {
    return null;
  }
  return value as NornStdioProtocol;
}

export function encodeFrame(message: NornStdioProtocol): string {
  return `${JSON.stringify(message)}\n`;
}
