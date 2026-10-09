#!/usr/bin/env node

// Verifies that the Tauri/Ratatui-free engine build does not pull a GUI or
// terminal toolkit into its active dependency tree. Fails if the dependency
// inspection itself fails, so CI cannot pass on an unverified tree.

import { execFileSync } from "node:child_process";

const FORBIDDEN = /(^|[ /])(tauri|ratatui|ratatui-image|crossterm) v\d/;

let output;
try {
  output = execFileSync(
    "cargo",
    [
      "tree",
      "--locked",
      "--manifest-path",
      "src-tauri/Cargo.toml",
      "--no-default-features",
      "-e",
      "normal,build",
    ],
    { encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] },
  );
} catch (error) {
  console.error("core dependency inspection failed:");
  console.error(error.stderr?.toString() || error.message);
  process.exit(1);
}

const offenders = output.split("\n").filter((line) => FORBIDDEN.test(line));
if (offenders.length > 0) {
  console.error("core build pulled GUI dependencies:");
  for (const line of offenders) {
    console.error(line);
  }
  process.exit(1);
}

console.log("core dependency tree is free of Tauri and Ratatui.");
