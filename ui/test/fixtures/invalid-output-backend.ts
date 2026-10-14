#!/usr/bin/env bun
// Owned adversarial peer. Never reads configuration or starts another process.
import { writeSync } from "node:fs";
import { encodeFrame, MAX_FRAME_BYTES, PROTOCOL_VERSION } from "../../src/transport/protocol";

let sent = false;
for await (const line of console) {
  const request = JSON.parse(line);
  if (request.type === "hello") {
    if (process.env.NORN_OWNED_BAD_FRAME === "stalled-handshake") continue;
    writeSync(
      1,
      encodeFrame({
        type: "ready",
        protocolVersion: PROTOCOL_VERSION,
        serverVersion: "owned-invalid",
        capabilities: [],
      }),
    );
  } else if (request.method === "shutdown") {
    process.exit(0);
  } else {
    if (sent) continue;
    sent = true;
    switch (process.env.NORN_OWNED_BAD_FRAME) {
      case "oversized":
        writeSync(1, `${"a".repeat(MAX_FRAME_BYTES + 1)}\n`);
        break;
      case "unterminated":
        writeSync(1, "a".repeat(MAX_FRAME_BYTES + 1));
        break;
      case "utf8":
        writeSync(1, new Uint8Array([255, 10]));
        break;
      case "direction":
        writeSync(
          1,
          encodeFrame({
            type: "hello",
            protocolVersion: PROTOCOL_VERSION,
            client: { name: "owned", version: "1" },
          }),
        );
        break;
      default:
        writeSync(
          1,
          `${JSON.stringify({
            type: "response",
            id: request.id,
            ok: true,
            result: {},
            unauthorized: "OWNED_PAYLOAD_NEVER_IN_ERROR",
          })}\n`,
        );
    }
  }
}
