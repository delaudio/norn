# Current Focus

This file is the live snapshot of any in-flight session. It is short on
purpose; the durable record lives in git, `_agent/WORKLOG.md`, and
`plan/done/`. Queued work lives in `plan/todo/`.

If status files and git disagree, git is authoritative; correct this file.

## Active state

- **Branch:** `fix/readiness-legacy-scan-generated-outputs` (release prep
  pending commit).
- **Active item:** ship `v0.3.2`, a bug fix for the readiness legacy-name scan.
- **Change:** `LEGACY_SCAN_SKIP_DIRS` in `src-tauri/src/readiness.rs` now skips
  generated, gitignored `storybook-static/` and `.astro/` output, which had
  tripped `repository.legacyNameNotAllowed` and blocked the terminal UI
  preflight when running `norn` from the Norn source tree. A regression test
  covers both directories, and `.storybook/manager-head.html` now uses Norn
  naming and `https://design-system.norn.dev`.
- **Version:** sources aligned at `0.3.2`
  (`package.json`, `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml`, locked
  `norn` entry).
- **Verification:** 668 Rust library tests pass with 2 ignored, including the
  new readiness regression; frontend tests, build, typecheck, lint, formatting,
  all-feature Clippy, `pnpm run version:verify`, Archgate 17/17, and a clean
  pre-push Norn review all pass. `norn doctor --repo-path .` reports zero
  `legacyNameNotAllowed` issues on the fixed tree.

## Next item

- Commit the release prep, open and merge the PR, tag `v0.3.2`, and confirm the
  release workflow end to end.
- Implement the desktop Local review target without changing the shipped TUI
  snapshot contract.
