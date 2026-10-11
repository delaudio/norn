# Immutable provider PR snapshots and bounded pre-review diff reads

## Owning ADRs

- `../../adr/0020-opentui-rust-stdio-workspace.md` (AC 3–5, 9–10).
- `../../adr/0002-http-in-rust.md`.
- `../../adr/0003-credentials-keychain.md`.

## Scope

Next bounded #357 delivery after PR #366: native immutable provider snapshot,
file inventory and paged diff reads, with strict generated IPC/client contracts.
The snapshot is a transient handle separate from actual saved review run IDs.
UI integration and review-start use of those exact pins follow in the next
required delivery; this backend increment cannot close #357/#306.

## Exit Criteria

1. Explicit provider/repository/PR requests obtain actual source/destination
   revisions and merge base, with full SHA validation. Immutable comparison URLs
   use captured revisions; mutable PR metadata is verified before/after and
   stale/changing/unsupported/fork data fails without working-tree fallback.
   Keep destination-tip SHA distinct from reviewed merge-base SHA.
2. HTTP uses fixed native origins/credentials, no automatic redirects/retries,
   bounded per-request/total deadlines and streamed bounded bodies. Any legitimate
   provider redirect is either replaced by an immutable trusted endpoint or
   separately validated for origin/path/revisions before a single explicit GET.
   GitHub compare metadata's 300-file bound cannot silently truncate inventory;
   a complete parsed patch must match the actual PR changed-file count or a
   complete bounded native diffstat, rather than guessing completeness from
   an API subset.
3. File/patch parsing handles rename/add/delete/binary/mode changes, quoted paths,
   spaces and UTF-8; hostile paths, duplicate/contradictory/missing metadata and
   malformed/truncated hunks cannot silently fabricate a complete snapshot.
   Preserve actual review patch bytes; display escaping must not change the
   engine snapshot or claim identical columns when unsupported data is escaped.
4. Cache is bounded by count/bytes/lifetime and full-scope identity. Snapshot
   IDs are opaque actual handles, never fabricated run IDs. Files/diff pages
   echo scope/handle/offset, keep exact UTF-8 boundaries and explicit next offsets;
   one protocol frame remains below 1 MiB even with JSON escape expansion.
5. Wrong provider/PR/handle/path/offset, stale/expired/evicted/restarted snapshots
   fail before credentials or Git. Reads run under the bounded native worker
   gate; status/cancel/shutdown remain responsive. No snapshot read starts AI,
   fetches/checks out Git, publishes or writes canonical data.
6. Owned tests exercise both providers, immutable URLs and ahead destination,
   fork/redirect cases, multi-page inventory/diff, malformed/oversized/slow/429
   responses, stale metadata, cache expiry/restart/eviction and identity
   collisions. Use synthetic native HTTP/keyring fixtures, never actual tokens
   or public/provider mutation. Real stdio capability/invalid/cache control,
   generated request/results fixtures and strict TS client page/generation fences.
7. Root typecheck/tests/lint/Archgate, protocol, UI typecheck/tests/frames,
   Rust core/stdio/isolated CLI/no-GUI and strict all-feature/all-target Clippy
   pass. Merge exact CI-verified head and update remaining #357 evidence.

## Dependencies

- PR #366 merged at `dd7562d29b712df6576ed31582123c4a8005e523` after exact
  `8acede54cc6cc503872e16507f224d35e0591c98` UI/verify CI success.
- Subsequent native preview selection and real review-start must consume the
  same cached snapshot, validate freshness without refetching a different payload,
  preserve profile/readiness/prompts/streaming/cancellation and actual provider
  provenance. Approval/native provider browser open remains separate.

## Native provider references

- GitHub compare commits: https://docs.github.com/en/rest/commits/commits#compare-two-commits
- Bitbucket immutable diff/merge-base: https://developer.atlassian.com/cloud/bitbucket/rest/api-group-commits/
- Bitbucket PR diff redirects: https://developer.atlassian.com/cloud/bitbucket/rest/api-group-pullrequests/

## Status

- Claimed by: default-agent, 2026-10-12, `issue-357-provider-snapshots`.
- Blockers: implementation, local gates and exact-head PR integration.
- Stopped:

## Follow-on review-start design constraints

- The exact snapshot patch must feed the shared prompt; do not validate pins
  and then refetch a different diff. Destination tip is not merge base.
- Existing stored reviewed_base_sha has been used as provider approval base
  and diff reconstruction base. Preserve compatibility deliberately: a new
  snapshot must record both the actual reviewed diff base and destination-tip
  provider pin, so finding draft approval checks remain coherent when the
  destination has advanced independently.
- The legacy inline start wrapper infers review provider/repo path from config.
  The new start must pass explicit captured provider/repo/pins/profile into
  the actual pipeline, rather than silently inferring from identical names.
- Legacy PR stores/session keys omit provider. Actual run provenance must fence
  thread selection and operation monitoring; no wrong-provider borrowed history
  or active session is allowed. Retained global/legacy history reconciliation
  remains #361, but introducing a new native start cannot worsen collisions.
- Owned fixture prepared for design verification:
  /tmp/norn-owned-pr-snapshot-vjcacx1w/{identity.json,expected.patch,repo}.
  Destination tip differs from merge base; patch excludes destination-only
  changes and an unrelated dirty working tree. Includes actual rename, Unicode,
  spaces, binary, mode-only, delete and header-like source lines.

## Concrete seam checklist from inspection

- `services/bitbucket/workspace_prs.rs`: Api has no-redirect native auth,
  strict full identity and bounded streamed JSON reads. Extract a private
  metadata reader for snapshot pin validation rather than fetching full reviewer
  history twice. Capture original GitHub changed_files for completeness checks.
- Immutable GitHub compare supports captured SHAs/fork network and diff media;
  JSON file inventory is limited to 300, so never treat its subset as complete.
- Bitbucket source-first `head..base` order is opposite Git diff. Resolve the
  actual merge base then compare head..merge-base with topic=false; validate
  any necessary PR redirect separately rather than restoring auto-follow.
- `services/bitbucket/approved_positions.rs` already handles quoted/octal UTF-8
  paths and strict hunk ranges. Reuse its internal parsing primitives where
  practical, adding file-section/binary/mode/rename completeness checks; don't
  copy the old Ratatui whitespace-only header parser into the native core.
- `backend.rs` has a bounded provider read pool and 1 MiB framed input. Cache
  handles need validation before Api::from_stored; stale/unknown handles must
  not trigger credential lookup or provider traffic.
- `BackendClient.providerRead` supplies generated schema, echoed request,
  512 KiB result and generation checks. Paged patch chunks of 64 KiB bound JSON
  escape expansion; byte offsets must be checked at exact UTF-8 boundaries.
- Do not add new production environment flags for fake credentials/API roots.
  Synthetic origins/auth stay private cfg(test) injections. Existing real
  stdio tests can exercise malformed/cache/stale/control paths without secrets;
  in-process owned native HTTP tests cover successful provider acquisition.
