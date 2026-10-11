# Canonical PR scope access and real saved review identities

## Owning ADRs

- `../../adr/0020-opentui-rust-stdio-workspace.md` (AC 3–6, 9).

## Scope

Implement #355, a bounded P0 follow-up blocking #306 and #309. Expose a
read-only, immutable paged inventory of canonical PR scopes and content counts
without exporting record contents. Merge those identities into the terminal
picker even without a saved review or configured repository. Preserve actual
saved runs rather than using job IDs or guessing providers from configuration.
Keep full provider/scope identity through selection, refresh and late pages.

## Exit Criteria

1. Rust validates canonical keys and emits typed bounded/versioned scope pages;
   total input/cache/frame bounds, snapshot expiry and invalid cursors are tested.
   Empty records and repository prompt/theme records do not invent PR content.
2. Owned imported GitHub/Bitbucket scopes without reviews and unconfigured repos
   remain selectable after restart; drafts/references/viewed content is accessible
   through existing canonical services without mutation or alternate storage.
3. Picker deduplication preserves distinct providers and real saved run IDs;
   job IDs never masquerade as run IDs. Missing Git/provider prerequisites remain
   explicit. Provider-mismatched review findings/history are never displayed.
4. Generation-fenced paging, settings refresh, reconnect and target changes
   preserve selection and reject stale data. Native input, wide/narrow stories
   and frame checks demonstrate reachability and navigation.
5. Root typecheck/tests/lint/Archgate, UI typecheck/tests/frames, Rust core/stdio,
   no-GUI dependency guard and strict all-feature/all-target Clippy pass. Merge a
   focused PR and record behavior/test evidence in the parity matrix. Do not
   close #306 or remove legacy surfaces until all retained workflows pass.

## Dependencies

- PR #354 merged at `4a303f1042e1747c7479d645392640da0921aed9`; #304 closed.
- Canonical storage and import PRs #340/#344 and existing review history.

## Status

- Claimed by: default-agent, 2026-10-11, `issue-306-canonical-scope-picker`.
- Blockers: implementation, local gates and verified PR integration.
- Stopped:
