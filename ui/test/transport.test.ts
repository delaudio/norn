import { expect, test } from "bun:test";
import { chmodSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { BackendClient } from "../src/transport/backendClient";
import { FakeBackendTransport } from "../src/transport/fakeTransport";
import { ProcessBackendTransport } from "../src/transport/processTransport";
import type { TargetIdentity } from "../src/transport/protocol";

const target = {
  kind: "provider",
  provider: "github",
  workspace: "acme",
  repo: "payments",
} as TargetIdentity;

test("fake transport handshakes, lists repositories and streams review events", async () => {
  const client = new BackendClient(
    new FakeBackendTransport({
      repositories: [{ provider: "github", workspace: "acme", repo: "payments", localPath: null }],
    }),
  );
  const ready = await client.start();
  expect(ready.capabilities).toContain("review");
  const repos = await client.repositories();
  expect(repos[0]?.repo).toBe("payments");

  const sequences: number[] = [];
  const unsubscribe = client.onOperationEvent((event) => sequences.push(event.sequence));
  const started = await client.startReview(target);
  expect(started.operationId.startsWith("fake-op-")).toBe(true);
  await new Promise((resolve) => setTimeout(resolve, 5));
  expect(sequences.length).toBeGreaterThan(0);
  unsubscribe();
  client.close();
});

test("process transport handshakes against a fixture subprocess", async () => {
  const script = fileURLToPath(new URL("./fixtures/fake-backend.ts", import.meta.url));
  chmodSync(script, 0o755);
  const client = new BackendClient(new ProcessBackendTransport(script));
  const ready = await client.start();
  expect(ready.serverVersion).toBe("fixture-0.1.0");

  const repos = await client.repositories();
  expect(repos[0]?.workspace).toBe("acme");

  const kinds: string[] = [];
  const unsubscribe = client.onOperationEvent((event) => {
    const value = event.event as { kind?: string };
    kinds.push(value.kind ?? "unknown");
  });
  const started = await client.startReview(target);
  expect(started.operationId).toBe("fixture-op-1");
  await new Promise((resolve) => setTimeout(resolve, 30));
  expect(kinds).toContain("state");
  expect(kinds).toContain("progress");
  unsubscribe();

  await client.shutdown();
  client.close();
});

test("fake transport lists review files and findings", async () => {
  const client = new BackendClient(
    new FakeBackendTransport({
      files: [{ path: "src/a.ts", status: "modified", additions: 2, deletions: 1, oldPath: null }],
      findings: [],
    }),
  );
  await client.start();
  const files = await client.reviewFiles(target);
  expect(files[0]?.path).toBe("src/a.ts");
  const findings = await client.reviewFindings(target);
  expect(findings).toHaveLength(0);
});

test("fake transport lists stored review targets", async () => {
  const client = new BackendClient(
    new FakeBackendTransport({
      targets: [
        {
          provider: "github",
          workspace: "acme",
          repo: "payments",
          prId: 1,
          runId: "run-1",
          title: "T",
          status: "succeeded",
        },
      ],
    }),
  );
  await client.start();
  const targets = await client.reviewTargets();
  expect(targets[0]?.prId).toBe(1);
});
