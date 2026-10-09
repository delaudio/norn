// Runtime validation for the Norn stdio protocol (epic #294, issue #297).
//
// The TypeScript types are generated into ./generated/protocol.ts from the
// single contract source; this module validates untrusted subprocess input
// against the exact same schema with ajv, so TS types alone are not trusted.

import Ajv, { type ValidateFunction } from "ajv";
import addFormats from "ajv-formats";
import { MAX_FRAME_BYTES, type NornStdioProtocol, PROTOCOL_VERSION } from "./generated/protocol";
import schema from "./generated/protocol.schema.json";

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
} from "./generated/protocol";
export { MAX_FRAME_BYTES, PROTOCOL_VERSION };

import type { Event, Hello, Ready, Request, ResponseError, ResponseOk } from "./generated/protocol";

export type ClientMessage = Hello | Request;
export type ServerMessage = Ready | ResponseOk | ResponseError | Event;
export type Response = ResponseOk | ResponseError;

const ajv = new Ajv({ allErrors: true, strict: false, allowUnionTypes: true });
addFormats(ajv);
const validateMessage: ValidateFunction = ajv.compile(schema);

/** Validate an untrusted decoded message against the protocol schema. */
export function isValidMessage(value: unknown): value is NornStdioProtocol {
  return validateMessage(value) === true;
}

/** Human-readable schema violations for the most recent invalid message. */
export function messageValidationErrors(): string[] {
  return (validateMessage.errors ?? []).map(
    (error) => `${error.instancePath || "/"} ${error.message ?? "is invalid"}`,
  );
}

/**
 * Decode one newline-delimited frame. Returns null for oversized, empty,
 * malformed, or schema-invalid input instead of throwing.
 */
export function decodeLine(line: string): NornStdioProtocol | null {
  if (line.length > MAX_FRAME_BYTES) {
    return null;
  }
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
  return isValidMessage(value) ? value : null;
}

/** Encode a message as a single frame (one trailing newline, no embedded one). */
export function encodeLine(message: NornStdioProtocol): string {
  return `${JSON.stringify(message)}\n`;
}

/** Reject an incompatible handshake before any review action is enabled. */
export function isCompatibleHandshake(message: NornStdioProtocol): boolean {
  return message.type !== "hello" || message.protocolVersion === PROTOCOL_VERSION;
}
