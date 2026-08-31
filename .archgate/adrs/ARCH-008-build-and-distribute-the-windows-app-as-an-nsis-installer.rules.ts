/// <reference path="../rules.d.ts" />

const TAURI_CONF = "src-tauri/tauri.conf.json";
const JUSTFILE = "justfile";

interface TauriConf {
  productName?: unknown;
  identifier?: unknown;
  bundle?: {
    active?: unknown;
    targets?: unknown;
    icon?: unknown;
  };
}

function targetsIncludeNsis(targets: unknown): boolean {
  if (targets === "all") return true;
  if (Array.isArray(targets)) return targets.includes("nsis");
  return false;
}

function hasIcoIcon(icon: unknown): boolean {
  return (
    Array.isArray(icon) &&
    icon.some((entry) => typeof entry === "string" && entry.toLowerCase().endsWith(".ico"))
  );
}

async function fileExists(ctx: RuleContext, path: string): Promise<boolean> {
  const matches = await ctx.glob(path);
  return matches.length > 0;
}

export default {
  rules: {
    "windows-nsis-bundle-config": {
      description:
        "tauri.conf.json must keep the NSIS Windows installer buildable (active bundle, NSIS target, .ico icon, product identity)",
      async check(ctx) {
        let conf: TauriConf;
        try {
          conf = (await ctx.readJSON(TAURI_CONF)) as TauriConf;
        } catch {
          ctx.report.violation({
            message: `Unable to read ${TAURI_CONF}. The Tauri bundle configuration is required to produce the Windows NSIS installer.`,
            file: TAURI_CONF,
          });
          return;
        }

        const bundle = conf.bundle ?? {};

        if (bundle.active !== true) {
          ctx.report.violation({
            message:
              "bundle.active must be true so Tauri produces installers (including the Windows NSIS setup .exe).",
            file: TAURI_CONF,
            fix: 'Set "bundle": { "active": true, ... } in tauri.conf.json.',
          });
        }

        if (!targetsIncludeNsis(bundle.targets)) {
          ctx.report.violation({
            message:
              'bundle.targets must be "all" or an array that includes "nsis" so the Windows NSIS installer can be built.',
            file: TAURI_CONF,
            fix: 'Set bundle.targets to "all" or include "nsis" in the targets array.',
          });
        }

        if (!hasIcoIcon(bundle.icon)) {
          ctx.report.violation({
            message:
              "bundle.icon must include a Windows .ico file; the NSIS installer and Windows executable require it.",
            file: TAURI_CONF,
            fix: 'Add an "icons/icon.ico" entry to bundle.icon.',
          });
        }

        if (typeof conf.productName !== "string" || conf.productName.length === 0) {
          ctx.report.violation({
            message:
              "productName must be set; the NSIS installer derives its product name from it.",
            file: TAURI_CONF,
            fix: 'Set a non-empty "productName" (e.g. "Lachesi").',
          });
        }

        if (typeof conf.identifier !== "string" || conf.identifier.length === 0) {
          ctx.report.violation({
            message:
              "identifier must be set; the NSIS installer derives its upgrade identity from it.",
            file: TAURI_CONF,
            fix: 'Set a non-empty reverse-DNS "identifier" (e.g. "app.lachesi.desktop").',
          });
        }
      },
    },

    "windows-nsis-build-recipe": {
      description:
        "When a justfile exists it should wire the Windows release build to an explicit NSIS bundle command",
      severity: "warning",
      async check(ctx) {
        // The task runner is governed by ARCH-007; only advise once it exists.
        if (!(await fileExists(ctx, JUSTFILE))) return;

        const matches = await ctx.grep(
          JUSTFILE,
          /--bundles\s+nsis[\s\S]*--features\s+custom-protocol,desktop-bundle/,
        );
        if (matches.length === 0) {
          ctx.report.warning({
            message:
              "justfile has no NSIS build with production protocol and desktop launch routing. The Windows release build should be wired to the task runner (ARCH-007).",
            file: JUSTFILE,
            fix: "Add a recipe such as `bundle-windows:` running `pnpm tauri build --bundles nsis --features custom-protocol,desktop-bundle`.",
          });
        }
      },
    },
  },
} satisfies RuleSet;
