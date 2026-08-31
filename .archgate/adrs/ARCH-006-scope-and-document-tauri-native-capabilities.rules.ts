/// <reference path="../rules.d.ts" />

const CAPABILITY_FILE = "src-tauri/capabilities/default.json";

// Approved permissions for the main window. Adding a permission here is an
// intentional governance step tied to ARCH-006 (owning feature + rationale).
const APPROVED_PERMISSIONS = new Set<string>([
  "core:default",
  "opener:default",
  "notification:default",
]);

function permissionId(entry: unknown): string | null {
  if (typeof entry === "string") return entry;
  if (entry && typeof entry === "object" && "identifier" in entry) {
    const id = (entry as { identifier?: unknown }).identifier;
    return typeof id === "string" ? id : null;
  }
  return null;
}

export default {
  rules: {
    "capabilities-stay-in-approved-allowlist": {
      description:
        "Every Tauri permission in capabilities/default.json must be in the ARCH-006 approved allowlist",
      async check(ctx) {
        const capability = (await ctx.readJSON(CAPABILITY_FILE)) as {
          permissions?: unknown[];
        };
        const permissions = Array.isArray(capability.permissions) ? capability.permissions : [];

        for (const entry of permissions) {
          const id = permissionId(entry);
          if (id === null) {
            ctx.report.violation({
              message:
                "Unrecognized permission entry shape in capabilities/default.json; declare permissions as an identifier string or an object with an `identifier` field so they can be governed (ARCH-006).",
              file: CAPABILITY_FILE,
            });
            continue;
          }
          if (!APPROVED_PERMISSIONS.has(id)) {
            ctx.report.violation({
              message: `Permission "${id}" is not in the ARCH-006 approved allowlist. Add it intentionally to this rule and document its owning feature, scope, failure behavior, and test coverage in ARCH-006 before enabling it.`,
              file: CAPABILITY_FILE,
            });
          }
        }
      },
    },
  },
} satisfies RuleSet;
