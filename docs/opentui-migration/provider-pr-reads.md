# Native provider pull-request reads

Internal migration evidence for the first #357 delivery, plan 0054, shipped in
PR #365 at `2f2cd50d3314b09528f323f40778c0eb45cc4879`, after exact-head
`8e2e36d` UI/verify CI success. This delivery supplies
the backend contract; terminal browsing, pinned pre-review diffs and explicit
PR approval remain required before #357/#306 can close.

## Behavior

`provider.prs`, `provider.pr.detail` and `provider.user` require explicit
GitHub/Bitbucket owner/repository identity. The generated contract covers strict
params and results; native validation precedes credential access. Owners/repo
segments are bounded ASCII slugs or actual Bitbucket UUID aliases encoded as single URL segments, provider PR IDs exclude local high-bit
IDs, list pages are 1–100 with 1–100 results. Query and author filters are bounded.

Existing native provider parsers preserve title, author, state/draft, branches,
revisions, timestamps and participant meaning. Open includes drafts, matching
the shipped desktop list; Draft is Open plus actual draft metadata. GitHub
Merged uses actual merge evidence among closed PRs. Author/query filtering never
changes raw-page continuation. Empty filtered pages retain their next page.

Native credentials stay in Rust. Requests use fixed production HTTPS API roots,
no redirects, five-second connect and ten-second request deadlines; detail
reads have a twenty-second total budget. Each body is at most 1 MiB, results
512 KiB; fields, participants, counts and IDs are checked before exposure.
Pagination verifies the exact origin/collection, unique consecutive page and
matching page size, then emits a numeric next page. It never follows a provider
URL with credentials. Rate limits, incomplete reads and malformed responses fail
with safe messages instead of exposing remote response/error bodies.

GitHub detail obtains actual reviews with at most five 100-review pages and
requires complete, ordered review evidence; approved/changes-requested/dismissed
states update the corresponding reviewer. Requested reviewers alone are not
reported as approved. Bitbucket uses actual participant approval metadata.
Browser URLs are constructed from trusted identity in Rust; opening them and
approving a PR are subsequent explicit-action deliveries.

Two worker reads at most run concurrently, leaving protocol control responsive.
Read workers have bounded HTTP lifetimes. Client results echo the exact full
request, are limited to 512 KiB and schema validated; generation callbacks fence
late responses, page/PR IDs and browser URLs receive semantic checks. No read
performs Git changes, AI execution, provider publication or canonical data writes.

## Owned verification

Five native HTTP tests cover both providers, multiple/empty filtered pages,
actual merge and author/query evidence, current user, details/revisions/review
approval changes, provider identity collisions, malformed states, oversized
bodies/fields, unsafe redirects/pagination, rate limits and wrong PR IDs.
Fixtures use local ephemeral listeners and synthetic credentials exclusively;
origins/auth injection is private and test-only. Every fixture rejects non-GET
methods and checks failed reads do not retry. Five transport tests cover optional
filter normalization, strict DTOs, stale generations, wrong provider/filter/PR,
cursor/URL checks and oversized results. Shared schema fixtures cover valid
requests/results and invalid owners/local IDs/page bounds. Real stdio verifies
the advertised capability, rejects invalid requests before credential dispatch
and remains responsive to control.

Final local gates pass: root typecheck/123 tests plus tooling/lint/Archgate 12/12,
generated protocol, UI typecheck/148 tests/886 assertions/93 frames, Rust 672 core
plus two existing ignored/52 stdio/six CLI/five onboarding/three binary tests,
strict all-feature/all-target Clippy and no-GUI dependency guard. Exact-head CI
passed before the verified squash merge cited above. Existing native frames remain unchanged because this delivery
does not add a terminal screen. Do not count a typed backend as the complete
provider-navigation parity row or waive the remaining #357 actions.

Full-suite verification also exposed an inherited CLI test that invoked actual
`auth status` with the maintainer environment and could query the OS keychain.
That owned test process was interrupted. The test now uses a child with the mock
keyring for the actual argument dispatcher and verifies real executable routing
through `auth --help`; remaining CLI invocations clear the environment and use
owned temporary home/config/data directories. Auth JSON still verifies missing
credentials/redaction, and skill/status/help/version assertions remain intact.
There is no production environment flag to replace credentials or bypass checks.
