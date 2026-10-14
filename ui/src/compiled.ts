// One self-contained executable also hosts the owned syntax subprocess.
// Keep the worker entry separate from renderer initialization and CLI parsing.
import { version } from "../../package.json";

if (process.argv.includes("--version")) {
  console.log(`norn-ui ${version}`);
} else if (process.argv[2] === "--syntax-worker") {
  await import("./workers/syntax");
} else {
  process.env.NORN_COMPILED_UI = "1";
  await import("./main");
}
