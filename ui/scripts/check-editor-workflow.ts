// Real controlling terminal/console smoke: production UI and Rust backend.
// Each case creates and removes its own repo/config/editor/PTY or ConPTY.
import assert from "node:assert/strict";
import { resolve } from "node:path";

const root = resolve(process.cwd());
const windows = process.platform === "win32";
const driver = Bun.which(windows ? "pwsh" : "python3");
assert(
  driver,
  windows
    ? "PowerShell 7 is required for owned ConPTY tests"
    : "Python 3 is required for owned PTY tests",
);
const failures: unknown[] = [];
for (const scenario of [
  "success",
  "failure",
  "interrupt",
  "backend-death",
  "resize",
  "ui-interrupt",
]) {
  const command: string[] = windows
    ? [
        driver,
        "-NoProfile",
        "-File",
        resolve(root, "ui/scripts/support/check-editor-conpty.ps1"),
        "-ProjectRoot",
        root,
        "-Case",
        scenario,
      ]
    : [
        driver,
        resolve(root, "ui/scripts/support/check-editor-pty.py"),
        "--root",
        root,
        "--case",
        scenario,
      ];
  const child: Bun.Subprocess = Bun.spawn(command, { stdout: "inherit", stderr: "inherit" });
  const deadline = setTimeout(() => child.kill(), 120_000);
  try {
    assert.equal(
      await child.exited,
      0,
      `Owned ${windows ? "ConPTY" : "PTY"} case failed: ${scenario}`,
    );
  } catch (error) {
    failures.push(error);
  } finally {
    clearTimeout(deadline);
  }
}
if (failures.length) throw new AggregateError(failures, "Owned terminal lifecycle cases failed");
