// Entry point for the Norn OpenTUI workspace. Starts the persistent Rust
// backend over stdio and mounts the React shell. The renderer restores the
// terminal on normal quit, Ctrl+C, backend death and initialization failure.

import { createCliRenderer } from "@opentui/core";
import { ShellStore } from "./app/store";
import { theme } from "./app/theme";
import { mountApp } from "./app/ui";
import { BackendClient } from "./transport/backendClient";
import { ProcessBackendTransport } from "./transport/processTransport";

const HELP = [
  "Norn OpenTUI workspace",
  "",
  "j/k or ↑/↓: move selection · Tab: switch pane (detail scrolls with j/k)",
  "Enter: start review · x: cancel · q or Ctrl+C: quit",
].join("\n");

async function main(): Promise<void> {
  if (process.argv.includes("--help")) {
    console.log(HELP);
    return;
  }
  let client: BackendClient | null = null;
  let renderer: Awaited<ReturnType<typeof createCliRenderer>> | null = null;
  const quit = (code = 0) => {
    client?.close();
    renderer?.destroy();
    process.exit(code);
  };
  try {
    client = new BackendClient(new ProcessBackendTransport());
    renderer = await createCliRenderer({
      exitOnCtrlC: true,
      backgroundColor: theme.background,
      screenMode: "alternate-screen",
      useMouse: true,
    });
    const store = new ShellStore();
    mountApp(renderer, store, client, quit);
    client.onOperationEvent((event) => store.applyEvent(event));
    client.onExit((exit) => {
      store.markClosed();
      if (exit.code !== 0 && exit.code !== null) {
        console.error(`norn-backend exited with code ${exit.code}.`);
      }
      quit(exit.code ?? 0);
    });
    await store.connect(client);
  } catch (error) {
    client?.close();
    renderer?.destroy();
    console.error(error instanceof Error ? error.message : String(error));
    process.exit(1);
  }
}

await main();
