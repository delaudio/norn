# Immutable provider snapshots

Internal evidence for #357 / plans 0056–0057. PR #367 supplies immutable
snapshots and the typed bridge. Plan 0057 connects native terminal and Pierre
previews; review-start integration, explicit approval and native external
provider browsing remain subsequent #357 work.
No saved review run is created by a snapshot read.

## Evidence boundary

`provider.snapshot` takes an explicit provider/repository/PR identity and reads
native metadata before and after capturing evidence. Source head, destination
tip and actual merge base are separate full immutable revisions. GitHub uses
an immutable merge-base/head comparison with diff media, and requires its parsed
complete patch inventory to match the PR's actual changed-file count. The JSON
compare file array has a 300-file ceiling and is not treated as complete.
Bitbucket uses source-first immutable comparisons in the authenticated source
repository, including fork identity, a native merge-base query and complete
bounded paginated diffstat. Missing source provenance or inaccessible revision
comparisons fail explicitly. No mutable PR diff redirect is followed and no
working-tree replacement is used.

Both adapters reuse fixed native API origins and stored authentication; the
only alternate origins/authentication exist in owned Rust test fixtures. API
requests never start AI, fetch/checkout Git, publish or modify canonical data.
Redirects and rate limits fail without retries. Connect/request/total deadlines
are 5/10/20 seconds. Metadata bodies are streamed within 1 MiB, patches within
8 MiB; diffstat permits at most 100 pages and 10,000 unique paths. UTF-8 patches
remain byte-identical to provider evidence. Unsupported or malformed patches
fail instead of guessing paths or inventing hunk positions.

## Cache and frames

A random opaque handle is bound to the complete provider/repository/PR scope.
It is independent of durable run IDs and exists only in the current backend:
16 entries, 64 MiB total inventory/patch data, a ten-minute lifetime and LRU
replacement. Restart, expiry, eviction or a changed scope produces an explicit
unavailable error; file/diff cache reads do not load credentials or invoke Git.
The existing two-worker provider gate keeps control requests independent.

`provider.snapshot.files` echoes handle, offset and requested limit and returns
an exact next offset. A requested page can contain fewer files because encoded
inventory data is capped at 384 KiB, including JSON escaping. Diff chunks use
byte offsets, preserve UTF-8 boundaries and contain at most 64 KiB of raw text.
Section hashes are calculated once at insertion, rather than rehashing an
entire large file for every chunk. Whole-patch hashes, byte counts and revision
pins are supplied by snapshot metadata. Encoded frames stay below 1 MiB.

The generated protocol provides strict request/results; the TS facade checks
structural request echoes, full scope and SHA format consistency, page counts,
exact continuations, duplicate/hostile paths and stale generations. Complete
inventory reads are bounded by 10,000 paths / 8 MiB; reconstructed individual
patches verify stable total/hash and actual SHA-256 bytes before returning.

## Owned verification

The committed patch fixture was generated from an owned local repository with
an ahead destination, unrelated dirty working tree, Unicode/space rename,
added and deleted text, header-like added source lines, binary content and an
executable-mode change. The snapshot cache asserts actual inventory and exact
section bytes, distinct destination/merge base, expiry/LRU/restart/full-scope
fences, invalid paths/offsets, split UTF-8 and worst-case JSON quote expansion.

Native synthetic HTTP tests cover GitHub and Bitbucket immutable URLs/fork
provenance, complete 301-file comparison despite an empty compare JSON subset,
multi-page diffstat, wrong pagination origin and duplicate paths, stale head,
destination and source-repository metadata, missing source provenance, partial
inventory, redirect/429 responses, oversized patch bodies and a real request
timeout. All HTTP requests are asserted to be GETs. Real subprocess tests
verify the capability, unavailable cache errors and subsequent responsive
control without acquiring credentials. Shared fixtures are checked on both
protocol boundaries; client tests cover strict DTOs, tampering, byte/hash
continuations and stale selections. No private provider/profile data is used.

Full-suite concurrency reproduced inherited and new fixture races. The actual
owned-loopback client failure was `hyper::Error(IncompleteMessage)` with zero
completed requests. Native socket inspection on macOS showed inherited
`O_NONBLOCK` (flags 6); a deliberately delayed/fragmented request failed before
the fix with `WouldBlock` (OS 35, 18.75 microseconds, zero bytes), followed by
`BrokenPipe`. Both owned HTTP fixtures now explicitly restore blocking mode on
accepted worker sockets while preserving their read limits/timeouts and bounded
concurrency. The delayed/fragmented regressions and existing held-idle connection
regression pass. Temporary transport/socket diagnostics were removed. The
production browser server already sets accepted sockets to blocking mode and
needed no change. No production provider deadline, retry or assertion was
changed. #307 records the observed cause and final supported-environment evidence.

See [Rust socket blocking mode](https://doc.rust-lang.org/std/net/struct.TcpStream.html#method.set_nonblocking)
and [BSD accept flags](https://man.freebsd.org/cgi/man.cgi?manpath=FreeBSD+13.0-RELEASE+and+Ports&query=accept&sektion=2)
for the portability contract; the actual macOS flags/error and before/after
regression are owned local evidence, not an assumption about every platform.

Final full gate results and the exact merged commit are recorded in the shipped
plan receipt. This evidence does not claim terminal preview/review-start parity,
native profile recovery, platform GUI runtime or packaging/cutover completion.

## Native API sources

- [GitHub compare commits](https://docs.github.com/en/rest/commits/commits#compare-two-commits)
- [Bitbucket comparisons and merge base](https://developer.atlassian.com/cloud/bitbucket/rest/api-group-commits/)
- [Bitbucket PR diff redirects](https://developer.atlassian.com/cloud/bitbucket/rest/api-group-pullrequests/)


## Native preview integration (plan 0057)

A fresh provider PR opens an immutable preview without creating or selecting a
review run. Existing saved reviews retain their actual run until F3 explicitly
switches to current provider evidence; F3 returns to that saved run. F5 requests
a new snapshot, clears prior evidence and exposes acquisition failures. Settings
refresh retains the current pinned preview rather than silently refreshing its
PR. Native status labels distinguish saved runs, loading, verified preview and
unavailable evidence; head, merge base and destination pins remain distinct.

The preview runs through existing file navigation, diff modes, syntax and search.
Inventory, patches and images are fenced by provider/repository/PR/opaque handle
and selection generation. Findings, threads and their anchors are cleared in a
current preview and restored from the saved run on return. Agent replies and
mutations cannot borrow the saved run while displaying a newer diff. Starting
AI from the cached evidence is a separate required delivery; Enter currently
reports that limitation. Clipboard copies the verified selected preview diff.

`provider.snapshot.image` validates the complete cached handle and allowlisted
path/side before touching credentials. It preserves fork source repository
provenance for head bytes and uses the destination repository for merge-base
bytes. GitHub resolves the actual immutable commit to bounded nonrecursive Git
trees, permits regular-file modes only, and verifies the raw Git blob's object
hash. The Contents API is deliberately avoided because it can dereference
symlinks. Bitbucket checks commit-file metadata, exact path/commit/size and
regular attributes, then reads the fixed revision endpoint. Neither adapter
follows download URLs, symlinks/submodules or redirects, including LFS media
redirects. Missing revisions, permissions and unsupported data fail explicitly.
No local clone, working-tree replacement or automatic fetch is required.

Raster bytes are capped at 2 MiB and checked against PNG/JPEG/GIF/WebP/BMP magic,
8192-pixel dimensions and 40M pixels. A separate 32 MiB/32-entry LRU cache cannot
outlive the evidence snapshot. Chunk transfers are at most 192 KiB, echo the
complete typed request, require the cached SHA-256 for continuation, and verify
assembled byte count and digest in the UI. Eviction or restart during transfer
fails rather than re-fetching midstream. Cache misses share the provider's two
native read slots with PR queries; cache hits and missing add/delete sides need
no provider call. Snapshot identity is checked again after acquisition.

`provider.snapshot.browser` supplies the exact cached patch and inventory to the
existing loopback/token/CSP/assets server. Its skipped native source wrapper
routes raster requests to the same cache and rechecks source identity before
responding. It never invokes the mutable PR image endpoint. Browser launch
returns a request acknowledgment and any launch error; it does not claim the
page was displayed. Unix launchers have null standard streams and one bounded
child owner/reaper, so an opener cannot corrupt stdio or hold a protocol response.
Windows uses native URL/document shell dispatch, without command interpolation.

Owned tests cover both adapters, fork/rename pins, malformed/partial/redirected
provider responses, hash/chunk/cache bounds, missing sides, expiry/restart and
shutdown, authenticated native browser bytes and delayed UI selection. Real
OpenTUI inputs exercise F3/F5 at 120 and 50 columns. Ten additional deterministic
Storybook frames cover both providers, saved runs, failed acquisition and image
comparison; the previous 105 frames remain identical. This evidence does not
complete #306 or waive native recovery/platform/install/cutover gates. The
maintainer closed #357 on 2026-10-11 at 22:01:23Z; remaining provider workflow
verification continues under #306 without reopening the issue.

Native API references: [GitHub commits](https://docs.github.com/en/rest/git/commits#get-a-commit),
[trees](https://docs.github.com/en/rest/git/trees#get-a-tree),
[blobs](https://docs.github.com/en/rest/git/blobs#get-a-blob),
[contents/symlinks](https://docs.github.com/en/rest/repos/contents#get-repository-content),
[Bitbucket source metadata](https://developer.atlassian.com/cloud/bitbucket/rest/api-group-source/),
and [Windows shell dispatch](https://learn.microsoft.com/en-us/windows/win32/api/shellapi/nf-shellapi-shellexecutew).
