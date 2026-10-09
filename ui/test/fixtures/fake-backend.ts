#!/usr/bin/env bun
// Minimal fixture backend used to exercise the production subprocess transport
// deterministically. Speaks the versioned stdio protocol on stdin/stdout.

const decoder = new TextDecoder();

function write(message: unknown): void {
  process.stdout.write(`${JSON.stringify(message)}\n`);
}

function handle(message: {
  type?: string;
  protocolVersion?: number;
  id?: string;
  method?: string;
  params?: Record<string, unknown>;
}): void {
  if (message.type === "hello") {
    if (message.protocolVersion !== 1) {
      write({
        type: "ready",
        protocolVersion: message.protocolVersion,
        serverVersion: "fixture",
        capabilities: [],
      });
      return;
    }
    write({
      type: "ready",
      protocolVersion: 1,
      serverVersion: "fixture-0.1.0",
      capabilities: ["repository", "diff", "review"],
    });
    return;
  }
  if (message.type !== "request") {
    return;
  }
  const params = message.params ?? {};
  switch (message.method) {
    case "repository.status":
      write({
        type: "response",
        id: message.id,
        ok: true,
        result: {
          repos: [{ provider: "github", workspace: "acme", repo: "payments", localPath: null }],
        },
      });
      return;
    case "diff.file":
      write({
        type: "response",
        id: message.id,
        ok: true,
        result: {
          target: params.target,
          path: params.path,
          diff: "@@ -1 +1 @@\n-old\n+new\n",
          truncated: false,
        },
      });
      return;
    case "review.start": {
      const operationId = "fixture-op-1";
      write({
        type: "response",
        id: message.id,
        ok: true,
        result: { target: params.target, operationId, state: "accepted" },
      });
      write({
        type: "event",
        operationId,
        sequence: 1,
        target: params.target,
        event: { kind: "state", state: "running" },
      });
      write({
        type: "event",
        operationId,
        sequence: 2,
        target: params.target,
        event: { kind: "progress", message: "collected file", completed: 1, total: 1 },
      });
      write({
        type: "event",
        operationId,
        sequence: 3,
        target: params.target,
        event: { kind: "state", state: "succeeded" },
      });
      return;
    }
    case "operation.status":
      write({
        type: "response",
        id: message.id,
        ok: true,
        result: { operationId: params.operationId, state: "succeeded", sequence: 3, error: null },
      });
      return;
    case "operation.cancel":
      write({
        type: "response",
        id: message.id,
        ok: true,
        result: { operationId: params.operationId, state: "cancelled", sequence: 4, error: null },
      });
      return;
    case "shutdown":
      write({ type: "response", id: message.id, ok: true, result: {} });
      process.exit(0);
      return;
    default:
      write({
        type: "response",
        id: message.id,
        ok: false,
        error: { code: "unknownMethod", message: `unknown method ${message.method}` },
      });
  }
}

let buffer = "";
const reader = Bun.stdin.stream().getReader();
while (true) {
  const { done, value } = await reader.read();
  if (done) {
    break;
  }
  buffer += decoder.decode(value, { stream: true });
  let index = buffer.indexOf("\n");
  while (index >= 0) {
    const line = buffer.slice(0, index);
    buffer = buffer.slice(index + 1);
    if (line.trim().length > 0) {
      handle(JSON.parse(line));
    }
    index = buffer.indexOf("\n");
  }
}

export {};
