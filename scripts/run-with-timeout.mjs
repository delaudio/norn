#!/usr/bin/env node

import { spawn } from "node:child_process";
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";

export function runWithTimeout(command, args, timeoutMs) {
  return new Promise((resolvePromise, reject) => {
    const useProcessGroup = process.platform !== "win32";
    const child = spawn(command, args, {
      detached: useProcessGroup,
      stdio: "inherit",
      windowsHide: true,
    });
    let timedOut = false;
    let forceTimer;

    const signalChild = (signal) => {
      try {
        if (useProcessGroup && child.pid) {
          process.kill(-child.pid, signal);
        } else {
          child.kill(signal);
        }
      } catch (error) {
        if (error?.code !== "ESRCH") {
          reject(error);
        }
      }
    };

    const timeout = setTimeout(() => {
      timedOut = true;
      signalChild("SIGTERM");
      forceTimer = setTimeout(() => signalChild("SIGKILL"), 2_000);
    }, timeoutMs);

    child.once("error", (error) => {
      clearTimeout(timeout);
      clearTimeout(forceTimer);
      reject(error);
    });
    child.once("exit", (code, signal) => {
      clearTimeout(timeout);
      clearTimeout(forceTimer);
      if (timedOut) {
        resolvePromise(124);
      } else if (code !== null) {
        resolvePromise(code);
      } else {
        resolvePromise(signal ? 128 : 1);
      }
    });
  });
}

function parseArguments(argv) {
  const separator = argv.indexOf("--");
  const timeoutIndex = argv.indexOf("--timeout-ms");
  if (separator < 0 || timeoutIndex < 0 || timeoutIndex + 1 >= separator) {
    throw new Error("Usage: run-with-timeout --timeout-ms <milliseconds> -- <command> [args...]");
  }
  const timeoutMs = Number(argv[timeoutIndex + 1]);
  const command = argv[separator + 1];
  if (!Number.isSafeInteger(timeoutMs) || timeoutMs <= 0 || !command) {
    throw new Error("Timeout must be a positive integer and a command is required.");
  }
  return { timeoutMs, command, args: argv.slice(separator + 2) };
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  try {
    const parsed = parseArguments(process.argv.slice(2));
    const exitCode = await runWithTimeout(parsed.command, parsed.args, parsed.timeoutMs);
    if (exitCode === 124) {
      console.error(`Bounded command timed out after ${parsed.timeoutMs}ms.`);
    }
    process.exit(exitCode);
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exit(1);
  }
}
