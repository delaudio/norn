# Finding Filters and Conversation Thread Selection

## Owning ADRs

- `../../adr/0020-opentui-rust-stdio-workspace.md` (AC 3, 4, 5).

## Scope

Complete the finding navigation part of issue #303: severity filtering,
file/severity grouping, stable selection by finding identity, stored thread
selection and bounded conversation display. The Rust backend returns thread
summaries/messages tied to stored runs; selecting a thread loads its latest
run and fences stale results. Reply continues the selected run's thread.
Fix operations remain a separate delivery under #303.

## Exit Criteria

1. `review.threads` is validated by the shared protocol and returns bounded
   stored conversation data without provider credentials or resume secrets.
2. Filters/grouping preserve selected finding identity when possible and
   navigate only visible findings; empty results clear obsolete highlights.
3. Thread selection uses the associated run identity for diff/findings/reply
   and clears a draft when switching review targets.
4. Renderer/input/store and real backend tests cover thread identity, stale
   results, filters and Unicode conversation content; native frame scenarios
   cover grouping and conversation states.
5. Root typecheck/test/architecture and UI typecheck/test/frame gates pass.

## Dependencies

- PR #333, merged to main at `cdf6bdf`.

## Status

- Claimed by: Codex, 2026-10-11, `issue-303-finding-thread-navigation`.
- Blockers:
- Stopped:
