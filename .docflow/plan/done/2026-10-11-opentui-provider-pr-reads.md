# Bounded native provider pull-request reads

## Owning ADRs

- `../../adr/0020-opentui-rust-stdio-workspace.md` (AC 3, 5, 9, 10).
- `../../adr/0002-http-in-rust.md` (provider HTTP remains native).
- `../../adr/0003-credentials-keychain.md` (credentials remain native).

## Scope

First bounded delivery of #357, blocking #306 and #309: typed read-only provider
PR listing, detail and current-user IPC. Explicit provider/repository identity
and Open/Draft/Merged/All, query/author and page filters survive each response.
Reuse existing provider parsers while replacing unbounded transport. Preserve
raw-page continuation when local filtering removes every result; enforce byte,
page, time and concurrency limits and reject credential-bearing redirects.

The subsequent #357 deliveries integrate native terminal browsing, pinned
pre-review diffs and explicit at-most-once PR approval. This backend delivery
does not close #357/#306 or claim those flows already work.

## Exit Criteria

1. Strict request/result DTOs appear in the authoritative generated protocol;
   invalid owners, repositories, IDs, filters, page bounds and unknown fields
   fail before provider access. Result identity echoes the exact request.
2. GitHub and Bitbucket list/detail/current-user use native credentials and
   fixed HTTPS API origins, no redirects, bounded bodies/deadlines/results and
   safe product errors. Fake test authentication and origins are test-only.
3. Open/Draft/Merged/All and actual author identity behave correctly for both
   providers. Empty locally filtered pages preserve legitimate continuation;
   hostile/off-scope pagination cannot receive credentials or invent pages.
4. Actual detail includes bounded author/branches/revisions/state/draft and
   reviewers. GitHub approved-review information comes from actual reviews,
   rather than treating requested reviewers as approved. Unsupported or
   incomplete evidence fails explicitly; no guessed provider or working tree.
5. A bounded native worker handles reads without blocking control/cancel.
   Synthetic fixtures exercise multiple pages, identity collisions, malformed,
   oversized, redirect, rate-limit, unavailable and mismatched-ID responses.
6. Root typecheck/tests/lint/Archgate, protocol, UI existing checks, Rust
   core/stdio/no-GUI and strict all-feature/all-target Clippy pass; merge exact
   verified head and record the remaining #357 work in parity evidence.

## Dependencies

- Plan 0053 / PR #356 merged at `2a7f0fe605d3012c2e2b9a0c03e1d15b60f7c574`; #355 closed.
- Native provider parsing and settings/authentication remain authoritative.

## Completion (effective on verified merge to main)

Implementation `8f1a120` on PR #356 baseline `2a7f0fe` adds read-only provider
PR list/detail/current-user IPC, bounded native transport, strict generated
DTOs and client identity/generation/cursor/URL validation. Owned provider
fixtures cover both providers, filtered paging, actual approval metadata,
unsafe/malformed/oversized responses and Bitbucket UUID aliases.

Local verification exits zero: root typecheck/123 tests plus tooling/lint/
Archgate 12/12, generated protocol, UI typecheck/148 tests/886 assertions/93
unchanged native frames, Rust 672 core plus two existing ignored/52 stdio/six
CLI/five onboarding/three binary tests, strict all-feature/all-target Clippy
and no-GUI dependency guard. The actual CLI credential-status test now runs
under an owned child mock keyring, with real executable help routing and
isolated remaining CLI commands. Initial inherited publication-fixture
intermittency is recorded in #307; a passing rerun is not called its fix.

Evidence: `docs/opentui-migration/provider-pr-reads.md`. Prepared completion is
not shipped until exact-head CI passes and the PR merges to main. Synchronized
main has no new plan collision; contiguous ADR catalogue and owner links pass,
while historical ADR 0011 coverage is unchanged. ADR 0020 remains Accepted.
#357 terminal browsing, pinned pre-review diffs and explicit PR approval remain
required. #306–#309 and actual native-profile recovery remain open; no Norn
review, live provider mutation, private-profile inspection or data removal.
