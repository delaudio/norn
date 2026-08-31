# Current Focus

This file is the live snapshot of any in-flight session. It is short on
purpose; the durable record lives in git, `_agent/WORKLOG.md`, and
`plan/done/`. Queued work lives in `plan/todo/`.

If status files and git disagree, git is authoritative; correct this file.

## Active state

- **Branch:** `release/v0.2.0`.
- **Active item:** prepare the first governed Norn release, `v0.2.0`.
- **Plan item:** operational release of implemented ADR 0014; no new capability
  decision is required.
- **Verification:** PR #205 shipped at `643ff6f` and its Docflow closeout at
  `d78e108`. Version sources are being aligned to `0.2.0`. Publication is
  blocked until the eight required Actions secrets and the exact-tag bootstrap
  variable are configured; the historical `v0.1.0` release is not a complete
  governed baseline and the public tap has no Norn formula or cask yet.

## Last shipped

`643ff6f` - ship release, Homebrew lifecycle, durable installation, and sandbox
permission diagnostics through PR #205.

## Next item

- Merge the `0.2.0` version alignment, configure signing/notarization and tap
  credentials, set `NORN_HOMEBREW_BOOTSTRAP_TAG=v0.2.0`, then create the signed
  tag only after a final preflight.
