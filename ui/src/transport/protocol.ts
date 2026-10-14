// Typed protocol surface for the OpenTUI client. The authoritative contract is
// the generated module at src/protocol/generated/protocol.ts (single source:
// protocol/norn-protocol.schema.json). This module re-exports it and adds a
// bounded schema validator. Subprocess output is validated before dispatch;
// TypeScript types and a trusted launch path do not replace runtime checks.

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

// Runtime validation uses the same generated schema as Rust/root fixtures.
// Resolve Ajv from the UI package, which CI installs independently of root.
import Ajv from "ajv";
import addFormats from "ajv-formats";
import { MAX_FRAME_BYTES, type NornStdioProtocol } from "../../../src/protocol/generated/protocol";
import schema from "../../../src/protocol/generated/protocol.schema.json";

const ajv = new Ajv({ allErrors: false, strict: false, allowUnionTypes: true });
addFormats(ajv);
const validateMessage = ajv.compile(schema);

export function parseFrame(line: string): NornStdioProtocol | null {
  if (Buffer.byteLength(line, "utf8") > MAX_FRAME_BYTES) return null;
  try {
    const value: unknown = JSON.parse(line);
    return validateMessage(value) ? (value as NornStdioProtocol) : null;
  } catch {
    return null;
  }
}
export function encodeFrame(message: NornStdioProtocol): string {
  const line = `${JSON.stringify(message)}\n`;
  if (Buffer.byteLength(line, "utf8") > MAX_FRAME_BYTES)
    throw new Error("encoded protocol frame exceeds the byte limit");
  return line;
}
