import { mkdir } from "node:fs/promises";
import { resolve } from "node:path";

const output = resolve(import.meta.dir, "../../dist/opentui");
await mkdir(output, { recursive: true });
const child = Bun.spawn(
  [
    process.execPath,
    "build",
    "--compile",
    "src/compiled.ts",
    "--outfile",
    resolve(output, process.platform === "win32" ? "norn-ui.exe" : "norn-ui"),
  ],
  { cwd: resolve(import.meta.dir, ".."), stdin: "inherit", stdout: "inherit", stderr: "inherit" },
);
process.exit(await child.exited);
