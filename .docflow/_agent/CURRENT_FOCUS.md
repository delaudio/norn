# Current Focus

This file is the live snapshot of any in-flight session. It is short on
purpose; the durable record lives in git, `_agent/WORKLOG.md`, and
`plan/done/`. Queued work lives in `plan/todo/`.

If status files and git disagree, git is authoritative; correct this file.

## Active state

- **Branch:** `fix/release-linux-dependencies`.
- **Active item:** recover the first governed Norn release as `v0.2.1` after
  the `v0.2.0` verification job exposed missing Tauri system dependencies on
  the Ubuntu runner.
- **Plan item:** `.docflow/plan/done/2026-08-31-cli-only-release-channel.md`,
  owned by implemented ADR 0014.
- **Verification:** PR #205 shipped at `643ff6f` and its Docflow closeout at
  `d78e108`. Version sources were aligned to `0.2.0` through PR #207. The
  Homebrew tap credential is configured. PR #208 shipped the command-only
  release path at `24d9ce1`; typecheck, 104 frontend tests plus tooling suites,
  Tauri IPC smoke, build, Archgate 17/17, and bounded Norn review passed. Apple
  signing and notarization remain intentionally unavailable. The signed
  `v0.2.0` tag was retained as immutable after run `33432277406` failed before
  creating a GitHub release or modifying the public tap.

## Last shipped

`1554ced` - close out the command-only release channel through PR #209.

## Next item

- Install the documented Tauri Linux prerequisites before the all-feature Rust
  gate, add a workflow regression test, align version sources to `0.2.1`, and
  publish the signed recovery tag after review and integration.
