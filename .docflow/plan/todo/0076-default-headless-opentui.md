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
