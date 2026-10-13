// Entry point for the Norn OpenTUI workspace. Starts the persistent Rust
// backend over stdio and mounts the React shell. The renderer restores the
// terminal on normal quit, Ctrl+C, backend death and initialization failure.

import { createCliRenderer, RendererControlState } from "@opentui/core";
import { restoreConsoleInterrupts } from "./app/consoleControl";
import { launchMode } from "./app/launchMode";
import { ShellStore } from "./app/store";
import { theme } from "./app/theme";
import { mountApp } from "./app/ui";
import { BackendClient } from "./transport/backendClient";
import { ProcessBackendTransport } from "./transport/processTransport";

const HELP = [
  "Norn OpenTUI workspace",
  "",
  "Default: pull requests for the Git repository in the current directory.",
  "--current-repo: default mode · --workspace: all configured repositories",
  "",
  "j/k or ↑/↓: move selection · Tab: switch pane (detail scrolls with j/k)",
  "Home: f Open/Draft/Merged/Local · Enter load · a AI review · s settings",
  "g: choose diff → g terminal / b browser · Tab pane · j/k move · F8 files",
  "q or Ctrl+C: quit",
].join("\n");

async function main(): Promise<void> {
  if (process.argv.includes("--help")) {
    console.log(HELP);
    return;
  }
  let client: BackendClient | null = null;
  let renderer: Awaited<ReturnType<typeof createCliRenderer>> | null = null;
  let store: ShellStore | null = null;
  let quitting = false;
  const quit = (code = 0, message?: string) => {
    if (quitting) return;
    quitting = true;
    store?.cancelSyntax();
    void (async () => {
      await client?.close();
      if (renderer?.controlState === RendererControlState.EXPLICIT_SUSPENDED)
        await new Promise<void>((resolve) => setTimeout(resolve, 150));
      renderer?.destroy();
      try {
        restoreConsoleInterrupts();
      } catch (error) {
        console.error(error instanceof Error ? error.message : "Console cleanup failed.");
        code ||= 1;
      }
      if (message) console.error(message);
      process.exit(code);
    })();
  };
  process.on("SIGINT", () => {
    if (renderer?.controlState !== RendererControlState.EXPLICIT_SUSPENDED) quit(130);
  });
  process.on("SIGTERM", () => quit(143));
  try {
    const mode = launchMode(process.argv.slice(2));
    client = new BackendClient(new ProcessBackendTransport());
    renderer = await createCliRenderer({
      exitOnCtrlC: false,
      backgroundColor: theme.background,
      screenMode: "alternate-screen",
      useMouse: true,
    });
    store = new ShellStore();
    mountApp(renderer, store, client, quit);
    client.onOperationEvent((event) => store?.applyEvent(event));
    client.onExit((exit) => {
      store?.markClosed();
      quit(
        exit.code ?? 0,
        exit.code !== 0 && exit.code !== null
          ? `norn-backend exited with code ${exit.code}.`
          : undefined,
      );
    });
    await store.connect(client, mode);
  } catch (error) {
    store?.cancelSyntax();
    await client?.close();
    renderer?.destroy();
    try {
      restoreConsoleInterrupts();
    } catch (cleanupError) {
      console.error(
        cleanupError instanceof Error ? cleanupError.message : "Console cleanup failed.",
      );
    }
    console.error(error instanceof Error ? error.message : String(error));
    process.exit(1);
  }
}

await main();
