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

## Status

- Claimed by: default-agent, 2026-10-11, `issue-357-provider-pr-reads`.
- Blockers: implementation, local verification and exact-head PR integration.
- Stopped:
