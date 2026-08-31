# Current Focus

This file is the live snapshot of any in-flight session. It is short on
purpose; the durable record lives in git, `_agent/WORKLOG.md`, and
`plan/done/`. Queued work lives in `plan/todo/`.

If status files and git disagree, git is authoritative; correct this file.

## Active state

- **Branch:** `codex/release-install-upgrade-hardening`.
- **Active item:** GitHub epic #204 — release, Homebrew lifecycle, and durable
  local installation hardening across issues #199-#203.
- **Plan item:** `0006-release-installation-upgrade-hardening` under ADR 0014.
- **Worktree note:** pre-existing user changes affect the CLI/GUI launch and
  documentation files. Release-gate work also required mechanical Clippy fixes
  across existing Rust modules so the new deny-warnings gate passes.
- **Verification:** issues #199-#203 are implemented locally. The canonical
  installer placed real executables in `~/.local/bin`; lifecycle timeouts now
  terminate process groups; formula and signed/notarized per-architecture cask
  publication are one release-blocking chain. A one-time, exact-tag bootstrap is
  available because the historical release is not a complete governed baseline;
  all later releases require a real prior-version upgrade. Frontend lint,
  typecheck, Vitest, tooling tests, production build, Rust fmt/Clippy/tests, and
  Archgate pass after the release additions. Desktop launch routing is now
  compile-time and cross-platform: desktop mode is the safe Cargo default,
  while CLI/TUI/service distributions disable it explicitly and are guarded by
  tooling tests. Default and no-default Rust routing tests, the real Tauri
  builder, Clippy, frontend gates, release validation, tooling tests, and
  Archgate pass. Desktop bundles are GUI-only, preventing Windows GUI-subsystem
  builds from swallowing CLI output; dedicated command builds retain explicit
  TUI/help/CLI routing. The pre-push Norn rerun found no remaining Windows
  routing bug. Its terminal-heuristic observation is covered by the explicit
  `norn-tui` command and the documented rule that both streams must be terminals;
  remaining lifecycle and release-window observations are non-blocking. Commit
  and branch publication are in progress.
- **Predecessor status:** issue #198 implementation and focused verification are
  present in the worktree; release delivery is now prioritized so that review
  runs use a governed installed binary.

## Last shipped

`fcfc508` - harden `norn doctor` compatibility checks for legacy-name migration
artifacts.

## Next item

- Inspect the published branch and determine whether the repository is ready
  for the next governed release or needs a release-candidate dry run first.
