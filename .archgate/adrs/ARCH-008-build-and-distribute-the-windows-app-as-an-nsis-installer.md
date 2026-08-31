---
id: ARCH-008
title: Build and distribute the Windows app as an NSIS installer
domain: architecture
rules: true
files:
  - "src-tauri/tauri.conf.json"
  - "justfile"
---

# Build and distribute the Windows app as an NSIS installer

## Context

Lachesi is a Tauri v2 desktop app (see [ARCH-001](./ARCH-001-tauri-react-rust-bitbucket-boundary.md)) that must ship a Windows distributable. Tauri's bundler can produce several Windows artifact types, and the project's `src-tauri/tauri.conf.json` currently sets `"bundle": { "active": true, "targets": "all", ... }` with an `icons/icon.ico` already present. Without a decision, "all" produces both an NSIS setup `.exe` and a WiX MSI on Windows, doubling the build surface and leaving it ambiguous which artifact contributors and users are expected to use.

Windows installer options:

- **NSIS (`.exe` setup)**: Tauri's Nullsoft-based installer. Small, flexible, supports per-user (no-admin) installation, and is the most widely used Tauri Windows target. It does not require the WiX toolchain.
- **WiX MSI (`.msi`)**: Preferred for enterprise group-policy/SCCM deployment, but requires the WiX toolset, is heavier to configure, and is unnecessary for Lachesi's individual-developer audience.
- **Portable `.exe`/raw executable**: No installer, but no shortcuts, uninstall entry, or update path — a poor default for end users.

Build-environment options:

- **Local Windows build via the task runner**: A contributor on Windows runs the bundle through `just` (see [ARCH-007](./ARCH-007-drive-repository-commands-through-platform-native-task-runners.md)). No shared CI infrastructure required.
- **CI (GitHub Actions `windows-latest`)**: Reproducible, unattended releases, but requires standing up and maintaining a release workflow before the app is even shipping.

Code-signing options: an Authenticode certificate removes the SmartScreen "unknown publisher" warning but requires purchasing and securely storing a certificate. For an early-stage internal tool this is premature.

For Lachesi today, the pragmatic decision is: **standardize on the NSIS setup `.exe`**, **build it locally on Windows through the `just` task runner**, and **defer code signing** as an explicit, documented future step. This keeps the toolchain minimal (no WiX, no CI, no certificate), matches the current single-maintainer workflow, and produces a real installer users can run.

## Decision

The Windows distributable for Lachesi MUST be the **Tauri NSIS setup `.exe`**:

- `src-tauri/tauri.conf.json` MUST keep `bundle.active: true`, and `bundle.targets` MUST either be `"all"` or an array that includes `"nsis"`, so the NSIS installer can be produced.
- `bundle.icon` MUST include a Windows `.ico` file (NSIS and the Windows executable require it).
- `productName` and `identifier` MUST remain set, as the installer derives its product name and upgrade identity from them.
- The Windows release build MUST be produced **locally on Windows** through the `just` task runner using an explicit NSIS bundle command (`pnpm tauri build --bundles nsis --features custom-protocol,desktop-bundle`), so the distributed artifact is unambiguously the NSIS installer, loads embedded frontend assets, and uses desktop launch routing even though `bundle.targets` stays `"all"` for other platforms.

Scope and explicit non-goals for this ADR:

- **MSI is not the standard.** WiX/MSI MAY be revisited later for enterprise distribution but is not produced or supported by default now.
- **No CI release pipeline is mandated.** Releases are built locally on Windows. A future ADR MAY introduce a GitHub Actions release workflow.
- **Code signing is deferred.** Windows builds ship unsigned for now; users will see a SmartScreen "unknown publisher" prompt. Adopting an Authenticode certificate and Tauri's `windows.signCommand`/`certificateThumbprint` configuration MUST be introduced as a separate ADR when a certificate is available.
- macOS/Linux bundling is out of scope for this ADR and remains governed by `bundle.targets: "all"`.

## Prerequisites

Producing the NSIS installer compiles the Rust backend (see [ARCH-001](./ARCH-001-tauri-react-rust-bitbucket-boundary.md)), so the Windows build host MUST have the following toolchain installed before running `just bundle-windows`:

- **Rust toolchain** — MUST be installed via `winget install Rustlang.Rustup`, which provides the `stable-x86_64-pc-windows-msvc` toolchain. The installer adds `~/.cargo/bin` to the user `PATH`; a **new terminal MUST be opened** afterward so `cargo` resolves.
- **MSVC C++ Build Tools** — MUST be installed via `winget install Microsoft.VisualStudio.2022.BuildTools` with the **"Desktop development with C++"** workload. This provides the MSVC `link.exe` linker and the Windows SDK that the `*-msvc` Rust toolchain requires. **Without it, `cargo build` fails at the link step** even though `cargo` itself is present.
- **WebView2 runtime** — is preinstalled on current Windows 10/11. When absent, it MUST be installed via `winget install Microsoft.EdgeWebView2Runtime`.

These are host-machine prerequisites and are not checkable from the repository, so they are enforced by documentation (this section and `CLAUDE.md`) rather than an automated rule.

## Do's and Don'ts

### Do

- **DO** produce the Windows installer with `pnpm tauri build --bundles nsis --features custom-protocol,desktop-bundle` (wired through the `just` `bundle-windows` recipe per [ARCH-007](./ARCH-007-drive-repository-commands-through-platform-native-task-runners.md)).
- **DO** keep `bundle.targets` as `"all"` or an array containing `"nsis"` in `tauri.conf.json`.
- **DO** keep a valid `.ico` in `bundle.icon` and ensure `productName` and `identifier` stay populated.
- **DO** distribute the generated NSIS setup `.exe` (found under `src-tauri/target/release/bundle/nsis/`) as the Windows artifact.
- **DO** document, in the release notes, that current Windows builds are unsigned and how to proceed past SmartScreen.
- **DO** raise a new ADR before adding MSI output, a CI release pipeline, or code signing.
- **DO** install the Rust toolchain, the MSVC C++ Build Tools ("Desktop development with C++" workload), and the WebView2 runtime before running `just bundle-windows` (see Prerequisites).

### Don't

- **DON'T** distribute the WiX `.msi` as the Windows artifact, or add MSI-specific configuration as the default path.
- **DON'T** remove `"nsis"` from `bundle.targets` or set `bundle.active` to `false` — that disables the required installer.
- **DON'T** remove the `.ico` entry from `bundle.icon` or blank out `productName`/`identifier`.
- **DON'T** ship a portable/raw `.exe` without an installer as the primary distributable.
- **DON'T** add code-signing credentials, certificates, or `signCommand` config under this ADR — that requires its own ADR.
- **DON'T** rely on an ad-hoc `tauri build` (which under `"all"` also emits an MSI) as the release step; use the explicit `--bundles nsis` command.
- **DON'T** assume `cargo` alone is sufficient on Windows — without the MSVC C++ Build Tools the link step fails with a missing-linker error.

## Consequences

### Positive

- **Minimal toolchain:** NSIS needs no WiX toolset, no CI, and no certificate to produce a working installer.
- **Real install experience:** Users get shortcuts, an uninstall entry, and per-user install without admin rights.
- **Deterministic artifact:** The `--bundles nsis` release command yields exactly one, well-defined Windows artifact.
- **Low maintenance now:** Local builds match the current single-maintainer workflow without standing up release infrastructure.
- **Consistent with existing config:** The repo already ships `icons/icon.ico` and `bundle.targets: "all"`, so NSIS output works with a minimal change.

### Negative

- **Unsigned installer:** Users encounter a SmartScreen "unknown publisher" warning until signing is adopted.
- **No enterprise MSI:** Group-policy/SCCM deployment is unsupported until an MSI ADR is written.
- **Manual releases:** Without CI, each Windows build is produced by hand on a Windows machine.

### Risks

- **SmartScreen erodes user trust:** Unsigned installers look suspicious. **Mitigation:** release notes explicitly document the unsigned status and next steps, and a follow-up signing ADR is called out as the remediation.
- **Accidental MSI drift:** A contributor could reintroduce MSI as the default. **Mitigation:** an automated `archgate` rule verifies `bundle.targets` still enables NSIS and code review rejects MSI-as-default changes.
- **Config regression breaks the installer:** Removing the `.ico` or clearing `identifier` silently breaks bundling. **Mitigation:** the companion rule fails the check when `bundle.active`, `bundle.targets` (NSIS), the `.ico` icon, `productName`, or `identifier` are missing or misconfigured.
- **Missing host toolchain blocks fresh contributors:** On a clean Windows machine the build fails with an opaque `cargo not found` or a linker error. **Mitigation:** the Prerequisites section and `CLAUDE.md` document the exact `winget` commands for the Rust toolchain, MSVC C++ Build Tools, and WebView2 runtime.

## Implementation Pattern

Relevant `src-tauri/tauri.conf.json` bundle section (NSIS enabled via `"all"`, Windows `.ico` present):

```json
{
  "productName": "Lachesi",
  "identifier": "app.lachesi.desktop",
  "bundle": {
    "active": true,
    "targets": "all",
    "icon": [
      "icons/32x32.png",
      "icons/128x128.png",
      "icons/128x128@2x.png",
      "icons/icon.icns",
      "icons/icon.ico"
    ]
  }
}
```

Windows release build, wired through the `just` task runner (ARCH-007):

```just
# justfile (Windows) — produces the NSIS setup .exe only
bundle-windows:
    pnpm tauri build --bundles nsis --features custom-protocol,desktop-bundle
```

The signed installer path after a successful build:

```
src-tauri/target/release/bundle/nsis/Lachesi_<version>_x64-setup.exe
```

## Compliance and Enforcement

Automated enforcement (companion `ARCH-008-...rules.ts`):

- **`windows-nsis-bundle-config`** parses `src-tauri/tauri.conf.json` and reports a violation when `bundle.active` is not `true`, when `bundle.targets` neither equals `"all"` nor includes `"nsis"`, when `bundle.icon` omits a `.ico` file, or when `productName`/`identifier` are missing.
- **`windows-nsis-build-recipe`** warns (non-blocking) when a root `justfile` exists but contains no `--bundles nsis` build command, so the NSIS release path stays wired to the task runner.

Manual enforcement (code review):

- Reviewers MUST reject changes that make MSI the default Windows artifact or that ship an unsigned build without documenting it in release notes.
- Reviewers MUST confirm any move to CI-based releases or code signing is introduced as its own ADR, not folded into unrelated changes.

Exceptions MUST be approved by the lead architect and documented as a separate ADR.

## References

- [Use a Tauri desktop shell with a React webview and a Rust Bitbucket client](./ARCH-001-tauri-react-rust-bitbucket-boundary.md)
- [Scope and document Tauri native capabilities before expanding them](./ARCH-006-scope-and-document-tauri-native-capabilities.md)
- [Drive repository commands through platform-native task runners](./ARCH-007-drive-repository-commands-through-platform-native-task-runners.md)
- `src-tauri/tauri.conf.json` (bundle configuration)
- [Tauri v2 — Windows Installer (NSIS)](https://tauri.app/distribute/windows-installer/)
- [Tauri v2 — Prerequisites (Rust, MSVC Build Tools, WebView2)](https://tauri.app/start/prerequisites/)
- [Tauri v2 — Code Signing (Windows)](https://tauri.app/distribute/sign/windows/)
