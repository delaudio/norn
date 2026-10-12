# Immutable local snapshots

The Rust backend advertises `local-snapshots`. Its typed API captures local Git
comparisons without starting an agent or changing the index, working tree,
branches, remotes or policy. The terminal controls and local review engine must
use this API in the following integration step. The API alone does not establish
complete terminal local-review parity.

A source is either `{ "kind": "current" }`, scoped to the backend's startup
repository, or `{ "kind": "configured", "repository": { "provider": "github",
"workspace": "owner", "repo": "name" } }`. A configured source requires exactly
one matching configuration entry, a canonical Git root and an exactly matching
parsed origin provider/owner/repository. Current mode may resolve a startup
subdirectory and preserves the native fallback from origin to the first remote.
It still requires a supported provider remote; it never guesses a provider.
Clients cannot supply a filesystem path through these methods.

`local.options` returns the actual qualified owner, opaque root ID, optional
branch/HEAD and a page of local or remote-tracking base refs. It does not fetch
refs. Pages contain at most 32 refs, each at most 256 bytes, with at most 1,000
candidates. Symbolic remote HEAD aliases are excluded.

`local.snapshot` accepts a lowercase 64-character capture nonce and a comparison.
Working-tree mode uses the native upstream/merge-base comparison and a nonempty,
ordered selection of `committed`, `staged`, `unstaged` layers. Branch mode accepts
one listed base ref and only `committed`. Untracked contents are excluded. Branch
comparisons also exclude staged/unstaged contents, with native exclusion warnings.
Empty selected layers stay explicit. The same path in two layers remains two
files and two independent patches.

The capture runs the existing native collectors twice under one 30-second Git
budget and verifies source ownership again. A changed native fingerprint fails
capture. Filters, fsmonitor, external diff, textconv, lazy network fetch and
optional index locks retain the native runner protections. Branch capture cannot
silently switch to a working-tree comparison. Detached HEAD and unborn staged
working-tree comparisons remain supported where the native collector permits.

A snapshot reference contains source, actual provider/repository, opaque root ID
and snapshot ID. Its content identity includes the private canonical root,
comparison, selected layers, pins, native fingerprint and stored evidence. The
capture nonce is excluded from this identity. Identical names in different
checkouts or providers cannot identify the same snapshot. The native fingerprint
is returned separately; the opaque snapshot ID must not be treated as a Git SHA
or ordinary provider PR. Its compatibility review ID has the local high bit set.

- `local.snapshot.info` restores verified immutable metadata.
- `local.snapshot.files` pages up to 32 layer-qualified files.
- `local.snapshot.diff` reads one captured layer/path in UTF-8 aligned chunks of
  at most 64 KiB, with the complete section's SHA-256 and exact next byte offset.
- `local.snapshot.recheck` takes a new capture nonce, repeats the saved comparison
  against its re-resolved source, and rejects stale pins/tracked evidence. It
  returns the original saved info on success. It does not replace evidence.
- `local.snapshot.cancel` uses the complete source and capture nonce. Registration
  occurs before worker dispatch, so immediate cancel cannot miss a queued capture.
  A wrong source is rejected. Completed/unknown captures return `canceled: false`.

Saved reads never resolve an arbitrary local SHA through the process cwd and
never substitute fresh checkout bytes. Restart, cache eviction or changed files
do not change a saved diff. Recheck must precede subsequent operations; it is a
point-in-time check, not a filesystem lock or permission to publish provider data.
Changed excluded branch working-tree contents do not pretend to be reviewed.

Bounds: selected patch bytes 16 MiB, serialized durable envelope 32 MiB, 6,000
layer-qualified files, 128 warnings of at most 512 bytes, two local evidence
workers/captures, and 1 MiB protocol frames. The cache holds at most 16 verified
records/64 MiB of serialized evidence; native buffers and deserialized objects
add overhead. No claim is made that total process memory is capped at 64 MiB.
Durable evidence is limited to 1,000 records/128 MiB. Quota rejection rolls back
both a new empty native owner and its evidence. Errors require an explicit retry
or new capture, with no implicit checkout, provider or agent fallback. EOF and
shutdown cancel native captures, suppress late local responses and wait within
bounded shutdown cleanup.

SQLite schema 20 adds `local_review_evidence`, whose immutable metadata and
patches follow their actual local native review parent through a foreign key.
Load validates parent provider/run ownership, root ID, format, full content hash,
layer inventory, pins and all bounds. Existing review runs are never overwritten
by capture. Native history retention/deletion cascades to evidence; global
cross-provider history cleanup remains part of the later history work.

The consistent SQLite backup includes this table. Canonical workspace JSON
exports do not include it. Schema-19 binaries reject schema 20; packaging must
provide a verified compatible backup and a matched binary/database rollback.
Copying a live SQLite file is not a supported backup or recovery proof.

Owned tests exercise both provider identities, nested startup/current sources,
exact configured origins, overlapping stages, untracked exclusion, rename/delete/
binary metadata, detached/unborn state, base movement, a local patch above 8 MiB,
UTF-8 pages, stale/corrupt/foreign data, restart, backup/cascade, atomic quota,
reservation/cancel limits, responsive stdio control and bounded EOF cleanup.
Native terminal source/base/layer controls and captured review/cancel/history
were merged in PR377. Pinned consumers and saved selection are described below.

Captured local evidence also has a native review lifecycle. The backend
advertises `local-evidence-reviews`; these methods require the full saved
`SnapshotRef`, including source, actual provider/repository and canonical root
identity. They do not accept a filesystem path or an arbitrary local SHA.

| Method | Input | Result |
| --- | --- | --- |
| `local.review.start` | saved snapshot, optional profile | accepted operation and exact local snapshot target |
| `local.review.history` | snapshot, offset, limit 1–32 | newest first, bounded run metadata and next offset |
| `local.review.findings` | snapshot, optional run ID, offset, limit 1–32 | exact run findings and captured candidate layers |
| `local.review.output` | snapshot, optional run ID | saved canonical markdown, at most 64 KiB |
| `local.review.threads` | snapshot, optional run ID | at most 12 attributable threads, with bounded messages |

Omitting a run ID selects the latest saved run; supplying an unknown ID fails.
Requests and selected run IDs are echoed and checked by the typed client.
History reads inspect only the verified native local parent. They work after a
backend restart even if the original checkout no longer exists. Oversized or
corrupt storage fails without loading an unbounded SQLite value. Conversation
reads omit private agent resume tokens and orphan threads that have no owned
run. Truncated conversation excerpts are marked explicitly; canonical output
is never silently truncated.

A start rechecks the same saved comparison with native Git cancellation before
execution. A changed source, checkout identity, base, index or relevant tracked
content fails instead of capturing a replacement. This is a point-in-time
check, not a filesystem lock. The existing Rust inline engine receives the
actual saved root and provider, bypassing provider-less checkout discovery and
process cwd. Shared native prompt/profile/policy validation and finding parsing
remain authoritative. This API always uses the native engine; deterministic
integration tests substitute an owned fake child through the existing agent
binary seam.

Each selected layer is labeled independently in the payload. Unselected and
untracked content is excluded from the captured payload. A finding anchor can
match multiple captured layers; `candidateLayers` preserves that ambiguity for
an explicit UI choice, rather than selecting the first same-name file. Local
reviews retain the existing repository read-tool context; the payload exclusion
does not promise that the agent cannot read other files in that checkout.

The complete local agent payload is limited to 8 MiB, including policy/analyzer
appendices, and is rejected before agent launch when oversized. Navigation can
still retain the larger 16 MiB captured patch. Native operations stream through
the existing operation ID and `operation.cancel`; cancellation terminates owned
agent children, including required preflight commands, and EOF fences queued
starts. The operation cancellation flag is retained through policy/preflight
work and checked before agent construction and launch, including retries.
Synthetic local review IDs do
not authorize provider comment publication or approval.

## Pinned raster, browser and saved selection

The terminal uses `local.snapshot.image` with the complete snapshot reference,
explicit layer/path/side and verified chunk SHA. The capture stores independent
base/HEAD, base/index and index/worktree image sides, including rename sources
and known absent added/deleted sides. Supported PNG/JPEG/GIF/WebP/BMP rasters
share the provider validator: 2 MiB per side, 8192 dimension / 40 million pixel
limits, 128 stored side entries and an 8 MiB aggregate raw-byte budget. Invalid,
unsupported, oversized and quota-omitted images have explicit unavailable states;
no image request falls back to a mutable checkout or a provider.

Stored format2 includes image bytes and metadata in its content identity and
live stale recheck. Format1 preserves its original identity and remains readable;
its images are unavailable. SQLite schema20 remains unchanged. Package rollback
must still match binary and stored format compatibility; the schema number alone
is not a guarantee that an older binary understands format2.

`local.snapshot.browser` requires an explicit layer. The authenticated Pierre
server receives exactly that saved patch and serves the same captured old/new
rasters through its current session. Reads work after restart without the checkout.

F7 opens the paged saved-local selector through `local.snapshot.list`. Pages
contain verified source/provider/root/snapshot references and a continuity token;
changed catalogues require an explicit refresh. Clone and snapshot identities
remain distinguishable. Opening saved evidence restores its actual run, findings
and threads without recapturing the current directory. A previously chosen finding
layer is retained only for the same snapshot, run, finding and anchor.

Public legacy arbitrary Local SHA requests are rejected before side effects:
callers must use qualified saved-local APIs. Explicit configured working-tree
provider requests retain their separate legacy contract. Qualified local reply/fix
controls and global legacy-history parity remain retained follow-up work.

`make ui-native-workflow` / `just ui-native-workflow` builds the no-default Rust
backend and command binary, then runs real OpenTUI input/rendering with production
stdio transport. All Git, settings, storage and fake-agent files belong to the
test. The scenario covers current/configured source, branch base, layer selection,
pinned image sides, review, explicit finding layer, native cancellation and saved
run restoration after replacing the checkout. It also exercises the current-repo
headless CLI. Missing binaries fail. CI runs this gate on Linux, macOS and Windows;
a configured lane is not proof until its exact PR head passes.
