# Current Focus

This file is the live snapshot of any in-flight session. It is short on
purpose; the durable record lives in git, `_agent/WORKLOG.md`, and
`plan/done/`. Queued work lives in `plan/todo/`.

If status files and git disagree, git is authoritative; correct this file.

## Active state

- **Branch:** `fix/browser-diff-shared-ui`.
- **Active item:** replace the browser viewer's duplicated renderer with the
  shared React diff UI.
- **Plan items:** `.docflow/plan/todo/0012-browser-diff-shared-ui.md`.
- **Verification:** pending implementation; Norn v0.2.8 is the clean release
  baseline at `a361c77`.

## Last shipped

`a361c77` - release the bounded browser viewer in Norn v0.2.8.

## Next item

- Implement and verify the shared React browser diff entry point.
