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
The native terminal local controls, finding context, real/fake review payload,
cancel/history and Pierre integration are still required in the next step.
