#!/usr/bin/env node

// Generates the TypeScript contract for the Norn stdio protocol from the single
// contract source, protocol/norn-protocol.schema.json, using
// json-schema-to-typescript. The schema is copied next to the generated types so
// the runtime validator (ajv) validates the exact same contract.
//
// Deterministic. Run `pnpm run protocol:generate`; CI runs
// `pnpm run protocol:check`, which regenerates and fails on drift.

import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname } from "node:path";

import { compile } from "json-schema-to-typescript";

const SCHEMA_PATH = "protocol/norn-protocol.schema.json";
const OUT_TS = "src/protocol/generated/protocol.ts";
const OUT_SCHEMA = "src/protocol/generated/protocol.schema.json";

const schema = JSON.parse(readFileSync(SCHEMA_PATH, "utf8"));
const version = schema.definitions.ProtocolVersion.const;
const maxFrameBytes = schema["x-maxFrameBytes"];

const banner = [
  "// AUTO-GENERATED FILE. DO NOT EDIT.",
  `// Source: ${SCHEMA_PATH}`,
  "// Regenerate with `pnpm run protocol:generate`.",
].join("\n");

const types = await compile(schema, "NornProtocol", {
  bannerComment: banner,
  additionalProperties: false,
  cwd: SCHEMA_PATH,
  unreachableDefinitions: true,
  enableConstEnums: false,
});

const constants = [
  "",
  `export const PROTOCOL_VERSION = ${version};`,
  `export const MAX_FRAME_BYTES = ${maxFrameBytes};`,
  "",
].join("\n");

mkdirSync(dirname(OUT_TS), { recursive: true });
writeFileSync(OUT_TS, `${types.trimEnd()}\n${constants}`);
writeFileSync(OUT_SCHEMA, `${JSON.stringify(schema, null, 2)}\n`);
console.log(`generated ${OUT_TS} and ${OUT_SCHEMA} from ${SCHEMA_PATH}`);
