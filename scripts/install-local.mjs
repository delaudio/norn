#!/usr/bin/env node

import { spawnSync } from "node:child_process";
import {
  chmodSync,
  copyFileSync,
  existsSync,
  lstatSync,
  mkdirSync,
  mkdtempSync,
  renameSync,
  rmSync,
} from "node:fs";
import { homedir } from "node:os";
import { dirname, isAbsolute, join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const commandGroups = {
  cli: ["norn"],
  tui: ["norn-tui"],
  desktop: ["norn-app"],
  "cli-compat": ["lachesi"],
  "tui-compat": ["lac"],
};
const defaultComponents = Object.keys(commandGroups);

function executableName(command, platform) {
  return platform === "win32" ? `${command}.exe` : command;
}

function verifyExecutable(path, command) {
  const result = spawnSync(path, ["--version"], {
    encoding: "utf8",
    timeout: 30_000,
    windowsHide: true,
  });
  if (result.error || result.status !== 0) {
    const detail = result.error?.code === "ETIMEDOUT" ? "timed out" : "returned a failure";
    throw new Error(`Staged command ${command} ${detail} during --version verification.`);
  }
}

function selectedCommands(components) {
  const unknown = components.filter((component) => !(component in commandGroups));
  if (unknown.length > 0) {
    throw new Error(`Unknown install component: ${unknown.join(", ")}.`);
  }
  return [...new Set(components.flatMap((component) => commandGroups[component]))];
}

export function installLocal({
  prefix,
  sourceDirectory,
  components = defaultComponents,
  platform = process.platform,
}) {
  if (!isAbsolute(prefix)) {
    throw new Error("Install prefix must be an absolute path.");
  }
  const commands = selectedCommands(components);
  if (commands.length === 0) {
    throw new Error("At least one install component is required.");
  }

  const binDirectory = join(prefix, "bin");
  mkdirSync(binDirectory, { recursive: true, mode: 0o755 });
  const stagingDirectory = mkdtempSync(join(binDirectory, ".norn-install-"));
  const staged = [];
  const replacements = [];

  try {
    for (const command of commands) {
      const name = executableName(command, platform);
      const source = join(sourceDirectory, name);
      if (!existsSync(source) || !lstatSync(source).isFile()) {
        throw new Error(`Required built command is missing or not a regular file: ${name}.`);
      }
      const stagedPath = join(stagingDirectory, name);
      copyFileSync(source, stagedPath);
      chmodSync(stagedPath, 0o755);
      verifyExecutable(stagedPath, command);
      staged.push({ command, name, path: stagedPath });
    }

    for (const entry of staged) {
      const target = join(binDirectory, entry.name);
      const backup = join(stagingDirectory, `${entry.name}.previous`);
      if (existsSync(target) && lstatSync(target).isDirectory()) {
        throw new Error(`Install target is a directory: ${target}.`);
      }
      const replacement = { target, backup, hadPrevious: existsSync(target), installed: false };
      replacements.push(replacement);
      if (replacement.hadPrevious) {
        renameSync(target, backup);
      }
      renameSync(entry.path, target);
      replacement.installed = true;
    }
  } catch (error) {
    for (const replacement of replacements.reverse()) {
      if (replacement.installed && existsSync(replacement.target)) {
        rmSync(replacement.target, { force: true });
      }
      if (replacement.hadPrevious && existsSync(replacement.backup)) {
        renameSync(replacement.backup, replacement.target);
      }
    }
    throw error;
  } finally {
    rmSync(stagingDirectory, { recursive: true, force: true });
  }

  return {
    binDirectory,
    commands: staged.map((entry) => entry.command),
  };
}

function parseArguments(argv) {
  const values = new Map();
  const normalized = argv[0] === "--" ? argv.slice(1) : argv;
  for (let index = 0; index < normalized.length; index += 2) {
    const flag = normalized[index];
    const value = normalized[index + 1];
    if (!flag?.startsWith("--") || value === undefined) {
      throw new Error(`Invalid installer argument near ${flag ?? "end of command"}.`);
    }
    values.set(flag.slice(2), value);
  }
  return values;
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  try {
    const args = parseArguments(process.argv.slice(2));
    const prefix = resolve(
      args.get("prefix") ?? process.env.NORN_INSTALL_PREFIX ?? join(homedir(), ".local"),
    );
    const sourceDirectory = resolve(
      args.get("source-dir") ??
        process.env.NORN_BUILD_DIR ??
        join(dirname(fileURLToPath(import.meta.url)), "../src-tauri/target/release"),
    );
    const components = args.has("components")
      ? args
          .get("components")
          .split(",")
          .map((component) => component.trim())
          .filter(Boolean)
      : defaultComponents;
    const result = installLocal({ prefix, sourceDirectory, components });
    console.log(`Installed ${result.commands.join(", ")} into ${result.binDirectory}.`);
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exit(1);
  }
}
