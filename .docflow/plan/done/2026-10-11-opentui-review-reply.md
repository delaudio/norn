# Review Reply and Terminal Composer

## Owning ADRs

- `../../adr/0020-opentui-rust-stdio-workspace.md`

## Scope

Add a typed `review.reply` request that continues a stored review thread using
the reply native, and a terminal composer to write the follow-up. The reply
reuses the shared inline session monitor, resolves the thread from the selected
run (`runId`), and preserves the provider recorded on that thread (falling back
to a Claude resume session id, then configuration). The composer supports
Unicode graphemes and keeps the draft on failure.

GitHub issue: #303 (epic #294 step 09/15).

Pending (tracked): `review.fix`, finding grouping/filtering.

## Exit Criteria

- `review.reply` starts a follow-up run on the selected thread and streams it
  through the same monitor as `review.start`.
- Replies to a stored run target the thread recorded on that run and resume the
  provider recorded on the thread.
- The composer opens only for pull-request review targets, blocks while an
  operation is active, keeps the draft on failure, and is grapheme-aware.
- `cargo clippy -D warnings`, `cargo test`, `pnpm run lint/typecheck/test`, the
  `ui` job and `archgate check` pass.

## Dependencies

- `../../adr/0020-opentui-rust-stdio-workspace.md`

---

Shipped through PR #333 (issue #303). Effective on merge to main. The owning
decision remains Accepted until the remaining migration criteria are complete.
