# Current Focus

This file is the live snapshot of any in-flight session. It is short on
purpose; the durable record lives in git, `_agent/WORKLOG.md`, and
`plan/done/`. Queued work lives in `plan/todo/`.

If status files and git disagree, git is authoritative; correct this file.

## Active state

- **Branch:** `docs/closeout-cli-release-channel`.
- **Active item:** close out the shipped command-only release channel, then
  publish the first governed Norn release, `v0.2.0`.
- **Plan item:** `.docflow/plan/done/2026-08-31-cli-only-release-channel.md`,
  owned by implemented ADR 0014.
- **Verification:** PR #205 shipped at `643ff6f` and its Docflow closeout at
  `d78e108`. Version sources were aligned to `0.2.0` through PR #207. The
  Homebrew tap credential is configured. PR #208 shipped the command-only
  release path at `24d9ce1`; typecheck, 104 frontend tests plus tooling suites,
  Tauri IPC smoke, build, Archgate 17/17, and bounded Norn review passed. Apple
  signing and notarization remain intentionally unavailable.

## Last shipped

`24d9ce1` - ship the command-only release channel through PR #208.

## Next item

- Merge this closeout, set `NORN_HOMEBREW_BOOTSTRAP_TAG=v0.2.0`, create the
  signed tag after a final preflight, monitor every release job, and remove the
  bootstrap variable after successful tap publication.
