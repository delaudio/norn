#!/usr/bin/env bun
// Owned subprocess: close protocol handles while remaining alive briefly.
import { closeSync, writeSync } from "node:fs";
import { encodeFrame, PROTOCOL_VERSION } from "../../src/transport/protocol";

for await (const line of console) {
  if (JSON.parse(line).type !== "hello") continue;
  writeSync(
    1,
    encodeFrame({
      type: "ready",
      protocolVersion: PROTOCOL_VERSION,
      serverVersion: "closed-pipes-fixture",
      capabilities: [],
    }),
  );
  closeSync(0);
  closeSync(1);
  setTimeout(() => process.exit(7), 100);
  break;
}
