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

## Completion (effective on merge to main)

Implementation `e4264db` on PR #354 baseline `4a303f1` adds content-free immutable
scope inventory and native imported/unconfigured PR navigation. Saved targets
use real run IDs and provider provenance; exposed legacy reads filter full
identity, while local high-bit identifiers cannot start provider reviews.
Evidence: `docs/opentui-migration/canonical-scope-picker.md`.

Root typecheck/123 tests+tooling/lint/Archgate 12/12, UI typecheck/143 tests/864
assertions/93 native frames, core 666+2 ignored/51 stdio/five CLI/five onboarding,
no-GUI dependency guard and strict all-feature/all-target Clippy pass. Native
input/resize, process restart, provider mismatch, immutable paging, maximum
inventory and cache/expiry cases pass. React Doctor reports no changed-file
issues. The inherited HTTP fixture's empty-connection failure was diagnosed
and fixed with a bounded concurrent fixture and a regression; production
provider deadlines and test assertions are unchanged. Four new native scenes
were visually inspected; ten existing descriptions now display the provider.

Prepared for verified PR integration; not yet shipped before merge. No new
ADR/plan slot collision on synchronized main, contiguous ADRs/INDEX metadata
and owning Accepted status pass; historical ADR 0011 coverage is unchanged.
ADR 0020 remains Accepted until full cutover. #306–#309, native desktop-profile
recovery and provider-less legacy history remain open parity work. No Norn
review, actual provider/credential/agent/profile access or destructive cleanup.
