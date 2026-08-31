# Current Focus

This file is the live snapshot of any in-flight session. It is short on
purpose; the durable record lives in git, `_agent/WORKLOG.md`, and
`plan/done/`. Queued work lives in `plan/todo/`.

If status files and git disagree, git is authoritative; correct this file.

## Active state

- **Branch:** `release/v0.2.0-cli-channel`.
- **Active item:** enable the first governed Norn release, `v0.2.0`, without an
  Apple Developer Program dependency.
- **Plan item:** `.docflow/plan/todo/0005-cli-only-release-channel.md`, owned by
  implemented ADR 0014; no new capability decision is required.
- **Verification:** PR #205 shipped at `643ff6f` and its Docflow closeout at
  `d78e108`. Version sources were aligned to `0.2.0` through PR #207. The
  Homebrew tap credential is configured. Publication now requires a
  command-only release path plus the exact-tag bootstrap variable; Apple
  signing and notarization remain intentionally unavailable.

## Last shipped

`643ff6f` - ship release, Homebrew lifecycle, durable installation, and sandbox
permission diagnostics through PR #205.

## Next item

- Implement and merge the command-only release channel, set
  `NORN_HOMEBREW_BOOTSTRAP_TAG=v0.2.0`, then create the signed tag only after a
  final preflight.
