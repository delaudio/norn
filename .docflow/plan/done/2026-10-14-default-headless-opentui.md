# Default headless Rust builds and OpenTUI entry points

Owner: default-agent (single writer). Priority: maintainer override, 2026-10-14.
Owning decision: [0020](../../adr/0020-opentui-rust-stdio-workspace.md), criteria 1, 2, 7, 8, 10.
Dependency: merged #394. Hardening #396/plan 0075 is paused as a draft; no concurrent implementation.

## Scope

Make ordinary Cargo, development, build and installation use core Rust, CLI,
stdio backend and OpenTUI only. Preserve browser diff assets and noninteractive
CLI behavior. Retain explicit Tauri/Ratatui feature lanes and separated legacy
CI. Update equivalent Make/Just tasks and contributor instructions. Do not
remove source components, rename the Rust crate, publish releases or modify user data.

## Exit criteria

- Default Cargo dependency tree includes no Tauri, Wry, GTK or Ratatui stack.
- Ordinary scripts never enable desktop, tui or custom-protocol features.
- Installed norn starts its adjacent OpenTUI client and backend on a terminal;
  CLI help/version work without a JS runtime. Installation includes browser assets.
- Native IPC, OpenTUI terminal startup and browser diff checks pass, alongside
  typecheck, tests and Archgate; explicit legacy checks run separately in CI.
- PR is reviewed without Norn, exact-head checks pass and squash merge is recorded.

## Remaining migration

This is the default-build transition, not removal of the optional adapters or
completion of packaging/upgrade proof in #308 and final deletion in #309.

## Shipped — 2026-10-14

Merged PR [#397](https://github.com/delaudio/norn/pull/397) at
2026-10-14T10:21:52Z. Shipped commit:
`416610fe42c7b4b7c0453bbcb04b126f9c528c78`.
Exact verified source: `b312fe9562bd8f4bdf63c32507e6e3c155908d57`.

- CI `verify`, `ui`, all three `native-workflow` jobs, `legacy-adapters`
  and the release `contract`: `SUCCESS`. Scheduled/release-only lifecycle:
  intentionally `SKIPPED`; no release was published.
- `pnpm run build`: `Finished release profile`, exit 0; includes browser assets,
  standalone client and default-feature Rust command/backend binaries.
- `pnpm run test:rust:core`: `776 passed; 0 failed; 2 ignored`, exit 0;
  default graph: `core dependency tree is free of Tauri and Ratatui.`, exit 0.
- UI: `318 pass`, `0 fail`; `Rendered 223 native OpenTUI frames.` and
  `Native frames match the committed baseline.`, exit 0.
- Root typecheck/tests/tooling, explicit UI project typecheck, CLI/onboarding,
  default-target Clippy and Archgate (`17 passed`) completed with exit 0.
  Early CI fixture type errors were fixed before the verified source above.
- Installed release smoke: `PASS success: real production UI/editor input,
  literal argv, foreground handoff, UI resume and terminal restoration`;
  `Installed CLI, OpenTUI, syntax worker, IPC and browser assets pass; CLI/UI
  need no external Bun/Node.`, exit 0. Native CI also runs this installed smoke
  on Linux, macOS and Windows, including ConPTY on Windows.
- Owned headless Docker build/runtime pass: `norn 0.3.4`, exit 0;
  runtime links libc/libm/libgcc only, retains Git and has no GUI packages/Bun.

No Norn review, live provider/agent calls, releases or user-store writes were
performed. ADR0020 remains Accepted because final migration cutover is broader
than this delivered default-build transition.

Audit: new numbering/INDEX/links and paused-branch overlap checks pass.
Pre-existing missing plan coverage for ADR0011 and the legacy coordination
scaffold remain outside this delivery. No coordination migration was applied.
