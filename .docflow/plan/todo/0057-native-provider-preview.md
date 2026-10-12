# Native provider snapshot preview in the terminal

## Owning ADRs

- `../../adr/0020-opentui-rust-stdio-workspace.md` (AC 3–5, 7, 9–10).
- `../../adr/0002-http-in-rust.md`.
- `../../adr/0009-terminal-image-diffs.md` (AC 1–6).

## Scope

Next bounded #357 delivery after verified PR #367 merge: connect actual provider
snapshots to the native file/diff UI, independent of prior AI review history.
Preserve saved-run selection and make entering/refreshing a live preview explicit.
Authenticated Pierre and image reads must use the same immutable evidence, with
bounded native acquisition or explicit unavailable errors. No AI or provider/Git
mutation occurs in this delivery. Review-start and explicit approve/external
provider browser remain subsequent retained-parity work under #306. The
maintainer closed #357 on 2026-10-11 at 22:01:23Z; preserve that state. Closure
does not replace verification of the retained workflows.

## Exit Criteria

1. Selecting a provider PR without a saved run loads provider.snapshot and the
   complete bounded inventory, then selected file's verified paged patch. Full
   provider/repository/PR/handle/generation identity fences each step. Existing
   saved-run selection stays pinned until explicit live-preview action.
2. Native UI clearly distinguishes live preview from a saved review, shows the
   captured head/destination/merge base appropriately without claiming a run ID,
   and supports wide/narrow file navigation, diff modes, search, syntax and copy
   semantics already implemented. Old findings/thread anchors cannot be applied
   silently to a newer preview. Returning to the saved run restores its evidence.
3. Explicit refresh changes only the live preview generation; expiry/eviction,
   changed metadata, missing/oversized/unsupported evidence and auth/API errors
   are visible. Failed refresh does not mix old inventory with new patches or
   manufacture an empty complete PR. No automatic refresh/retry/fetch/AI occurs.
4. For image before/after and authenticated browser preview, use only captured
   pins/allowlisted paths and complete raw snapshot bytes. No mutable PR route
   or working-tree substitution is allowed. Verify cache identity before any
   credential/Git access. Preserve existing browser security/token/asset policy
   and exit cleanup; error explicitly when immutable blobs are unavailable.
5. Owned synthetic fixtures cover both providers, identical names/IDs across
   providers, a fresh PR without a saved run, saved-run/live transitions, stale
   snapshot/files/diff/preview/browser results, expiry/errors/empty/rename/
   binary/mode/large UTF-8 patches and real native keyboard/paste/resize/exit.
   Native frame scenarios cover loading/list/diff/empty/error/stale and narrow
   states. Browser/stdio/cache tests use owned sources, not actual credentials.
6. Root typecheck/tests/lint/Archgate/protocol, UI typecheck/tests/frames and
   React Doctor, Rust core/stdio/isolated CLI/fmt/strict Clippy/no-GUI pass. Inspect
   actual native frames and all changed baselines. Exact CI-verified squash merge
   required; receipt does not ship merely by moving to plan/done.

## Dependencies

- PR #367 exact-head UI/verify CI and verified squash merge.
- Existing saved-run diff/browser/image delivery and strict snapshot bridge.
- Native preview before review-start integration. Legacy providerless store
  reconciliation remains #361; new UI must not attribute unknown legacy runs.

## Status

- Claimed by: default-agent, 2026-10-12, `issue-357-native-provider-preview`.
- Blockers: implementation, full local gates, exact-head CI and verified merge.
- Stopped:

## Inspection constraints

- ShellStore.loadSnapshot currently uses review.files/findings; fresh PRs have
  no reviewed range. Snapshot acquisition must not call those as substitutes.
- Saved entry runId remains actual durable data. Transient preview mode must
  invalidate selection generation and suppress stale finding/thread actions.
- loadDiff already coalesces requests; use client.providerSnapshotPatch with
  a live-generation callback without bypassing this coalescing/culling/syntax.
- provider.snapshot.files can return fewer than requested limit due JSON byte
  bounds; aggregate via strict facade, never assume page length implies EOF.
- Shell F3/F5 are currently available; inspect complete key routes before use.
- Existing image_preview_result validates stored range then immutable Git blobs;
  snapshot variant must validate cache/path/pins first. Local immutable blob
  reads may be permitted with a provider-qualified configured checkout; absent
  objects are explicit unavailable, never current worktree fallback/auto fetch.
- Existing browser state caches diff/diffstat but image route can still query
  provider metadata. A snapshot browser must not retain that mutable behavior;
  supply immutable image sources or explicit unavailable support.
- Production browser accepted sockets already reset blocking mode (unlike fixed
  test fixtures); do not redo that fix. Native browser launcher stdio/deadline
  hardening remains relevant and must precede opening from stdio.


## Concrete browser/image design

- Validate full cached handle and changed-file/side before credentials or Git.
  Native immutable provider reads are the primary image route and do not require
  a local clone: GitHub traverses bounded immutable Git trees, rejects symlink/
  submodule entries and gets the exact regular-file blob, verifying its Git
  object hash; Bitbucket verifies bounded file metadata/path/commit/attributes
  then reads raw content from the fixed commit endpoint, rejecting link/subrepo
  and media redirects. Never use a returned download URL or mutable PR preview.
- Capture authenticated source repository provenance in the transient cache
  for fork head images, distinct from the destination repository for base images.
  It is internal metadata, never a user-provided HTTP origin. Recheck source
  provenance during snapshot capture; inaccessible/deleted source fails safely.
- A bounded raster cache is separate from the 64 MiB patch/inventory cache:
  32 MiB/32 entries, 2 MiB per raster, lifetime no longer than its snapshot.
  Continuation requires cached immutable hash; eviction cannot silently refetch
  or restart midway. Provider reads retain bounded worker/concurrency/deadlines.
- Give WebDiffState a skipped comparable snapshot-source wrapper (actual ref +
  owning manager) so existing browser image calls dispatch to this immutable
  cache/native acquisition. No serialized native paths, credential or manager
  state. Browser source equality includes handle/manager identity; no stale
  image source reused for another preview. Preserve security/token/assets policy.
- Extract shared raster validation for native terminal/browser use, preserving
  2 MiB body, 8192 dimensions and 40M-pixel limits, format checks and SHA-256
  transfer integrity. Add typed full-scope request echoes and chunk continuation.
- Bound native launcher behavior and isolate stdout from protocol. Explicit OS
  launch request is not proof a browser displayed the page; no retries or actual
  user's browser in tests. Use owned process fixtures for stdout/deadline cases.
- Review-start follows with explicit provider/path/profile and exact cached patch.
  Preserve reviewed_base_sha as destination approval pin; optional actual
  reviewed_diff_base_sha serves diff readers. Replies preserve both. Legacy
  providerless thread/session/store reconciliation remains #361 and cannot be
  worsened by new starts. Seven existing ReviewRun literals need checking.

## Native source references

- GitHub immutable trees: https://docs.github.com/en/rest/git/trees#get-a-tree
- GitHub immutable blobs: https://docs.github.com/en/rest/git/blobs#get-a-blob
- GitHub content/symlink semantics: https://docs.github.com/en/rest/repos/contents#get-repository-content
- Bitbucket commit-file metadata/raw/attributes: https://developer.atlassian.com/cloud/bitbucket/rest/api-group-source/
