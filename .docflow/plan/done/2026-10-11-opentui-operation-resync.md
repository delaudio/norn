# Preserve Operation Identity and Resync Saved Review State

## Owning ADRs

- `../../adr/0020-opentui-rust-stdio-workspace.md` (AC 3, 4, 5).

## Scope

Separate accepted operation ownership from the current picker selection. Keep
cancellation available when a response arrives after navigation, reject events
for another target or old sequence, and fetch authoritative operation/saved
review snapshots after sequence gaps and completion. Never attach another
review's conversation or findings to the current target. Preserve submitted
reply drafts on failure while fencing composer updates by generation.

## Exit Criteria

1. Late start/reply/fix responses preserve operation IDs and original target
   labels; cancellation still reaches the exact backend operation.
2. Concurrent starts are prevented while another request is unresolved. Stale
   sequence/target events cannot mutate the tracked operation or current panes.
3. Event gaps trigger bounded coalesced authoritative status/snapshot refresh.
   Final review/reply success refreshes targets, findings/history/conversation
   for the original selection only, including the new saved run identity.
4. Failing or interrupted work does not auto-retry or publish; ambiguous request
   failure shows inspection guidance and retains draft state when applicable.
5. Deterministic delayed-response, wrong-target, out-of-order, gap/burst and
   completion-refresh tests pass, with UI frames and root gates.

## Dependencies

- Approved sync/conflict delivery (plan 0035) integrates first.

## Status

- Claimed by: default-agent, 2026-10-11, `issue-303-operation-resync`.
- Blockers: None; PR #337 merged at `2f9b58e`.
- Stopped:

## Delivery prepared for merge

- PR: https://github.com/delaudio/norn/pull/338
- Effective on merge to main; ADR 0020 remains Accepted until full cutover.
- Validation: 56 UI tests, 26 checked native frames, root lint/typecheck/tests
  and Archgate 17/17. Rust/protocol unchanged from the preceding verified merge.
- Coalesced await-in-loop is intentional response ordering, verified by burst
  tests; no React Doctor suppression. Live provider smoke is documented opt-in
  and was not run.
