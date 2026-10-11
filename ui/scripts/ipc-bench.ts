// Read-only IPC latency against an explicit real backend and isolated empty
// configuration. No auth/status/review/agent/provider request is issued.
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { BackendClient } from "../src/transport/backendClient";
import { ProcessBackendTransport } from "../src/transport/processTransport";

const [binaryArgument, outputPath] = Bun.argv.slice(2);
if (!binaryArgument || !outputPath)
  throw new Error("Usage: bun run scripts/ipc-bench.ts BACKEND OUTPUT.json");
const binary = resolve(binaryArgument);
const sandbox = await mkdtemp(join(tmpdir(), "norn-ipc-bench-"));
const config = join(sandbox, "config");
await mkdir(config);
await writeFile(
  join(config, "settings.json"),
  JSON.stringify({ repos: [], defaultDiffView: "unified", theme: "dark" }),
);
const environment = {
  ...process.env,
  NORN_CONFIG_DIR: config,
  NORN_DATA_DIR: join(sandbox, "data"),
  NORN_REVIEW_DATA_DIR: join(sandbox, "review"),
};
let client: BackendClient | undefined;
let unsubscribe: (() => void) | undefined;
let exitTimer: ReturnType<typeof setTimeout> | undefined;
try {
  const started = performance.now();
  const transport = new ProcessBackendTransport(binary, environment);
  client = new BackendClient(transport);
  const ready = await client.start();
  const processHandshakeMs = performance.now() - started;
  const samples: number[] = [];
  for (let index = 0; index < 100; index++) {
    const started = performance.now();
    if ((await client.repositories()).length !== 0)
      throw new Error("Isolated configuration was not used");
    samples.push(performance.now() - started);
  }
  samples.sort((a, b) => a - b);
  if (processHandshakeMs > 100 || (samples[94] ?? Infinity) > 50)
    throw new Error("Real IPC latency budget exceeded");
  const exit = new Promise<number | null>((resolve, reject) => {
    unsubscribe = transport.onExit((status) => resolve(status.code));
    exitTimer = setTimeout(() => reject(new Error("Backend did not exit after shutdown")), 5000);
  });
  await client.shutdown();
  const observedExitCode = await exit;
  if (observedExitCode !== null && observedExitCode !== 0)
    throw new Error("Backend exited unsuccessfully");
  await writeFile(
    outputPath,
    JSON.stringify(
      {
        backendVersion: ready.serverVersion,
        runtime: Bun.version,
        platform: process.platform,
        arch: process.arch,
        request: "repository.status",
        samples: 100,
        processHandshakeMs,
        roundTripMedianMs: samples[49],
        roundTripP95Ms: samples[94],
        providerCalls: 0,
        agentRuns: 0,
        config: "isolated empty",
        shutdownAcknowledged: true,
        observedExitCode,
        shutdown:
          observedExitCode === 0
            ? "clean exit 0"
            : "ACK then EOF; transport did not report exit code",
      },
      null,
      2,
    ),
  );
  console.log(`Wrote real Rust IPC measurements to ${outputPath}`);
} finally {
  if (exitTimer) clearTimeout(exitTimer);
  unsubscribe?.();
  client?.close();
  await rm(sandbox, { recursive: true, force: true });
}
