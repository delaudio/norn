import { spawnSync } from "node:child_process";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const backend = resolve(
  root,
  "src-tauri/target/debug",
  process.platform === "win32" ? "norn-backend.exe" : "norn-backend",
);
for (const [binary, args] of [
  ["pnpm", ["run", "browser-diff:build"]],
  [
    "cargo",
    [
      "build",
      "--locked",
      "--manifest-path",
      "src-tauri/Cargo.toml",
      "--bin",
      "norn-backend",
      "--bin",
      "norn",
    ],
  ],
]) {
  const result = spawnSync(binary, args, {
    cwd: root,
    stdio: "inherit",
    shell: process.platform === "win32",
  });
  if (result.error || result.status !== 0) process.exit(result.status || 1);
}
const result = spawnSync(
  "bun",
  ["run", resolve(root, "ui/src/main.ts"), ...process.argv.slice(2)],
  {
    // Preserve caller cwd: default home must resolve the caller's Git repository.
    stdio: "inherit",
    env: { ...process.env, NORN_BACKEND_BIN: backend },
  },
);
if (result.error) console.error(result.error.message);
process.exit(result.status ?? 1);
