# Canonical saved-scope picker evidence

Internal evidence for ADR 0020 / plan 0053 / issue #355. This closes a bounded
scope-access gap in #306, not the whole parity or desktop-recovery gate.

## Behavior

`workspace.scopes` reads the existing canonical document without changing it.
Its pages contain provider, owner, repository, PR identity and draft/reference/
viewed counts; they contain no bodies, reference values, local paths or tokens.
Empty records and repository-only prompt/theme records do not invent PRs.
GitHub and Bitbucket identities stay separate, including unconfigured repos.
Case is preserved. Ambiguous PR-number aliases fail explicitly with the source
file preserved rather than selecting a different canonical key.

The picker merges those scopes with real saved review targets. A scope without a
run has a nullable run ID and a visible saved-data label; a real saved run keeps
its actual ID. Provider labels distinguish identical owner/repository/PR values.
Canonical content remains accessible even when no repositories are configured.
Missing local clones or saved reviewed revisions remain explicit errors and
never cause a PR diff to fall back to the working tree. Legacy local IDs keep
their high-bit classification and cannot start provider reviews or publish.

Saved review targets resolve the job's actual conversation/run and use the
saved run's provider, rather than job IDs and current configuration guesses.
Findings/history/conversations filter shared legacy stores by complete saved
run identity before returning data. A pinned run from a different provider
returns `targetStale`; filtering is read-only and does not rewrite old stores.

## Bounds and consistency

- Existing canonical input: 16 MiB, 10,000 records, validated record payloads.
- Inventory: at most 10,000 scopes; 100 items / 256 KiB per IPC page.
- Immutable metadata cache: four entries, eight MiB total, five-minute expiry;
  snapshots bind canonical path and whole-document version. Restart/eviction/
  expiry requires a fresh first page. Later pages cannot mix file versions.
- Client accumulation: eight MiB, 10,000 scopes, at most 101 requests, strict
  generated DTO validation and advancing/consistent cursors. Selection changes
  stop outstanding pages. Inventory encoding counts each scope once per page.
- Requests share the existing four-worker workspace limit; cancel/status and
  shutdown do not wait for an inventory read on the control loop.

## Verification

- Four Rust inventory tests: full-provider/case identity, local-only marker,
  immutable pages during edits, restart/expiry/cursor/corruption, maximum 10,000
  scopes, cache byte/count/path bounds and ambiguous-key rejection.
- Real stdio process tests: offline unconfigured content access after restart,
  immutable old page versus refreshed version, local provider-review refusal,
  actual saved run IDs, provider provenance and wrong-provider read rejection.
- Seven UI state/native input tests: imported unconfigured scopes, distinct
  providers, real-run preservation, paging, malformed/duplicate/inconsistent
  pages, late-selection fencing, settings refresh, local-only refusal and draft
  access across provider switches and a 50-column resize.
- Four new native frames/Storybook stories: unconfigured imported picker and
  draft panel at 120×30 and 50×15. Ten existing review scenes change only picker
  description text to show the provider. All 93 native frames were inspected
  for changed text and pass the exact golden-frame check.
- Root typecheck, 123 tests plus tooling, lint (two existing informational
  syntax-test suggestions), protocol regeneration/check and Archgate 12/12;
  UI typecheck, 143 tests / 864 assertions; full Rust 666 passed / two ignored,
  51 stdio, five CLI/five onboarding, no-GUI dependency guard and strict
  all-feature/all-target Clippy. React Doctor reports no changed-file issues.

The complete gate reproduced inherited localhost-fixture failures. Diagnostic
assertions showed an accepted connection with no HTTP bytes, which the serial
fixture counted as an empty request after blocking subsequent connections.
The test server now handles at most sixteen concurrent connections, records
only complete bounded HTTP requests, joins its workers and writes each response
in one buffer. A regression opens an idle connection before an authenticated
request and verifies it neither blocks nor becomes a counted request. The full
suite passes with its default concurrency. Production provider timeouts,
assertions and test selection are unchanged; no Norn review or live HTTP access.

## Remaining parity and recovery

Live provider PR browsing/filters/detail, explicit local branch/base/layer
selection, explorer/branch operations, global history and retained analytics,
Jira/Notion content materialization and automatic refresh remain #306 work.
Provider-less legacy job/conversation recovery and the provider namespace of
legacy SQLite writes remain explicit history/hardening work: this change does
not fabricate provenance for a job that has no verified saved run. Native
desktop-profile export/runtime evidence remains required despite #305 being
closed on GitHub. No live credentials, providers, agents or private profiles
were used; no Norn review, release publication or data removal occurred.
