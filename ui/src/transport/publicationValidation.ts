// Validate publication DTOs against the same generated schema used by Rust/CI.

import type { PublicationScope } from "../../../src/protocol/generated/protocol";
import contract from "../../../src/protocol/generated/protocol.schema.json";

type Schema = {
  $ref?: string;
  type?: string;
  const?: unknown;
  enum?: unknown[];
  oneOf?: Schema[];
  anyOf?: Schema[];
  properties?: Record<string, Schema>;
  required?: string[];
  additionalProperties?: boolean;
  items?: Schema;
  minItems?: number;
  maxItems?: number;
  uniqueItems?: boolean;
  minimum?: number;
  maximum?: number;
  minLength?: number;
  maxLength?: number;
  pattern?: string;
};
const definitions = contract.definitions as unknown as Record<string, Schema>;
export function validPublicationDTO(name: string, value: unknown): boolean {
  let budget = 100_000;
  function matches(schema: Schema, value: unknown, depth: number): boolean {
    if (--budget < 0 || depth > 64) return false;
    if (schema.$ref) {
      const key = schema.$ref.replace("#/definitions/", "");
      return !!definitions[key] && matches(definitions[key], value, depth + 1);
    }
    if (
      schema.oneOf &&
      schema.oneOf.filter((candidate) => matches(candidate, value, depth + 1)).length !== 1
    )
      return false;
    if (schema.anyOf && !schema.anyOf.some((candidate) => matches(candidate, value, depth + 1)))
      return false;
    if (Object.hasOwn(schema, "const") && value !== schema.const) return false;
    if (schema.enum && !schema.enum.includes(value)) return false;
    if (schema.type === "null") return value === null;
    if (schema.type === "boolean") return typeof value === "boolean";
    if (schema.type === "integer")
      return (
        typeof value === "number" &&
        Number.isSafeInteger(value) &&
        value >= (schema.minimum ?? -Infinity) &&
        value <= (schema.maximum ?? Infinity)
      );
    if (schema.type === "string") {
      if (typeof value !== "string") return false;
      const length = [...value].length;
      return (
        length >= (schema.minLength ?? 0) &&
        length <= (schema.maxLength ?? Infinity) &&
        (!schema.pattern || new RegExp(schema.pattern).test(value))
      );
    }
    if (schema.type === "array")
      return (
        Array.isArray(value) &&
        value.length >= (schema.minItems ?? 0) &&
        value.length <= (schema.maxItems ?? Infinity) &&
        (!schema.uniqueItems ||
          new Set(value.map((item) => JSON.stringify(item))).size === value.length) &&
        !!schema.items &&
        value.every((item) => matches(schema.items!, item, depth + 1))
      );
    if (schema.type === "object") {
      if (!value || typeof value !== "object" || Array.isArray(value)) return false;
      const object = value as Record<string, unknown>;
      if (schema.required?.some((key) => !Object.hasOwn(object, key))) return false;
      return Object.entries(object).every(([key, item]) =>
        schema.properties?.[key]
          ? matches(schema.properties[key], item, depth + 1)
          : schema.additionalProperties !== false,
      );
    }
    return true;
  }
  return !!definitions[name] && matches(definitions[name], value, 0);
}
export function samePublicationScope(actual: PublicationScope, expected: PublicationScope) {
  return (
    actual.provider === expected.provider &&
    actual.workspace === expected.workspace &&
    actual.repo === expected.repo &&
    actual.prId === expected.prId
  );
}
