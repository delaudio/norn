# Approved publication delivery

Internal implementation evidence for epic #294 / issue #304 and plan 0052;
implements ADR 0020 AC 3–6, 8, 9. Parent journal PR #353 is merged at `3385fe1`.

Rust owns provider credentials, canonical draft snapshots, expiring approval
payloads, durable per-item publication state, remote comment reads and finding
feedback. OpenTUI owns selection, exact preview presentation and transient
composer state. No automated test uses a live provider, native keychain or Norn
review.

| IPC method | Boundary |
| --- | --- |
| `publication.prepare` | Read-only current revisions, supported parents, exact file/inline positions and canonical draft/source validation; up to 20 distinct drafts, preview at most 512 KiB. |
| `publication.apply` | One approved item, single-use five-minute token, source/content/CAS rechecks and journal reservation before provider access. |
| `publication.cancel` | Responsive control request; invalidates unused items and stops a pending provider POST before send when cancellation is observed. An already sent request retains its durable result. |
| `publication.status` | Provider/PR-scoped journal pages, up to 50 entries and 512 KiB per page. No provider credential access. |
| `publication.reconcile` | Read-only exact authenticated-author/body/marker/parent/anchor scan; never POST. Confirmed unchanged results repair local metadata/cleanup. |
| `comments.list` | Bounded complete provider inventory, scoped/versioned pages; a changing inventory restarts pagination rather than joining inconsistent pages. |

GitHub and Bitbucket adapters use fixed production API origins, bounded bytes,
items/pages and timeouts, disabled automatic redirects and no POST retry helper.
Pagination cannot redirect credentials to another origin or endpoint. Bitbucket
patch redirects are limited to the same repository's diff route. File comments
need membership in the changed-file inventory; inline comments need an exact
old/new single-line position in a complete supported patch. Unsupported modes
and stale roots never become different comment modes or top-level comments.

The terminal sends sequentially after one explicit batch approval and stops on
partial/uncertain or incomplete local acknowledgment. Each approval item is
consumed before work; retries inspect persistent status. Draft edits/removals
cannot bypass a pending/confirmed publication. Confirmed remote IDs survive
postflight revision changes, read failures and local storage failure. Changed
or uncertain results retain drafts; no absence scan proves a request safe to
retry automatically.

SQLite schema 17 preserves schema-16 journal rows and raises the serialized
entry limit to accommodate quoted original Markdown. Original optional fields
remain omitted from legacy payload hashes. Durable journal timestamps and
finding associations support restart repair after canonical draft cleanup.
Finding publication metadata and feedback commit together against the latest
review JSON. Narrow updates preserve unrelated/unknown chat, run, finding and
publication fields and later reviewer dispositions. Canonical cleanup checks
all original draft fields and CAS; external/concurrent edits remain intact.
SQLite acknowledgment and canonical JSON cleanup are separate durable steps,
not a cross-store transaction.

Native controls: `d` staged comments, `p` publication, Space selection, Enter
read-only preview, `y` approval, `n`/Escape cancellation, `s` status, `r` read-only
reconciliation, `t` remote threads and `r` local reply staging. Bracketed paste
cannot trigger publication. Root replies keep the actual parent/path/side;
GitHub general comments explicitly lack threaded replies. The selected-item
list renders at most 21 rows and full bodies remain scrollable. Eight native
visual fixtures cover wide/narrow selection/preview, partial results, status
and threads, bringing the frame catalogue to 89.

Verification: root typecheck, 123 tests plus tooling checks, lint and Archgate
12/12; UI typecheck, 136 tests / 830 assertions and 89 native frame baselines;
Rust core 660 tests (2 ignored), 49 actual backend subprocess tests, five CLI
and five onboarding integration tests, no-GUI dependency guard and strict
all-feature/all-target Clippy. Fake-provider cases include explicit batch
approval, scope/CAS/source fencing, durable reservation, ambiguous responses,
partial batches, cancellation before POST, timeout-after-success/restart,
unsupported/stale anchors, schema migration, idempotent metadata repair and
transaction rollback on feedback conflict.

An initial full Rust run failed one loopback fixture before completing its
intended pagination scan; the isolated test passed. Increased the owned server
read allowance and subsequent complete suites passed, retaining the full-scan
assertions. Initial UI input code used a nonexistent testing helper and was
corrected to real bracketed paste; native input and full suites passed. Earlier
migration-test fixture used the wrong table name; corrected to
`schema_migrations` and full core passed. React Doctor findings were inspected:
lookup/state-reset diagnostics were corrected; native `flexShrink` is declared
by installed OpenTUI types and verified by frames/input, not a DOM attribute.
No diagnostic suppression or live publication smoke test.

GitHub #305 was already closed at 16:16:54 UTC. Remaining work includes native profile recovery/runtime evidence and scope access under #306 parity,
#307 hardening, #308 installation and #309 removal/cutover. ADR 0020 remains
Accepted until the complete migration ships. Historical ADR 0011 plan coverage
still prevents claiming a globally clean documentation audit.
