# Canonical workspace records

The terminal and Rust review engine consume `workspace-data-v1.json` through
`workspace.read` and `workspace.mutate`. The import/export owner remains the
service introduced with PR #340; there is no second frontend persistence store.
Provider, repository and pull-request identities remain part of record keys.

## Reads and edits

Reads return at most 100 items and 256 KiB per page. A nonzero offset requires
the first page's record version. The backend retains at most four immutable
record snapshots and 32 MiB of encoded snapshot data. A captured read can finish
against its original version after a concurrent write; eviction returns an
explicit refresh error. The client checks scope, kind, version, availability,
total, offsets, progress and aggregate bounds before assembling pages.

Mutations replace one prompt/theme scalar or add/update/remove one draft,
reference or viewed path. A SHA-256 version of the current complete record
fences every write. Changing another record does not cause a false conflict.
Validation happens before the canonical atomic replacement. Corrupt files,
stale versions, mismatched record kinds/PR identities and invalid fields preserve
existing bytes; consumers never retry a stale write automatically.

A stable sibling `.workspace-data-v1.json.lock` coordinates new import and
mutation writers across processes. Settings saves also use a sibling lock.
The file handle owns the advisory lock and is dropped after the atomic write;
its pathname stays in place to avoid two independent lock inodes. Acquisition
waits at most two seconds; symlink sidecars are refused. This uses Rust's standard
[File::try_lock](https://doc.rust-lang.org/std/fs/struct.File.html#method.try_lock),
stable since Rust 1.89 (the current build/CI toolchain is 1.94).
Older desktop/config writers that do not acquire these locks are not claimed
safe concurrently; their consolidation remains part of #307.

## Terminal workflow

Press `n` after selecting a target to open its repository prompt. Imported
prompts and PR viewed marks are loaded from the canonical owner. Type/paste
multiline text, Ctrl+S to save, Ctrl+D to discard edits, Ctrl+U to clear, or Esc
to close/discard unsaved edits. Input is bounded to 64 KiB of UTF-8 and shortcut
characters pasted into the editor never trigger review/fix/publication actions.
Failed saves retain the draft. Reopen to reload after a version conflict; copy
any edit that should be retained before closing. Late responses cannot populate
a closed or newly selected panel.

Ctrl+V toggles the selected file's viewed mark in a PR scope. Working-tree
targets have no PR key and show that persisted PR marks are unavailable rather
than inventing a pull-request ID. The file picker restores canonical marks and
refreshes them when the panel closes.

A nonblank saved repository prompt replaces repository/default policy, matching
the desktop resolver. An empty override falls back to the selected repository's
`review.prompt.replace`/`extend` configuration and shared bundled default. Invalid
repository policy fails clearly when no user override is active. A local target
finds its provider/repository owner only through a matching canonical configured
local path; another provider with the same slug cannot supply its prompt.

New reviews and fresh-thread/fallback replies use this resolver. A successful
Claude resumed reply sends only the new message and retains the existing agent
conversation, matching the shared desktop/native reply implementation. Changing
a saved prompt does not rewrite that conversation. Headless prompt semantics
remain unchanged.

## Evidence and remaining scope

Tests use temporary settings, canonical data, review databases, Git repositories
and fake agent executables; no Norn review or live provider publication is run.
Coverage includes record CAS, immutable paginated snapshots with 100 maximum-size
reference bodies, cache eviction, full draft publication/finding identity,
wrong-PR rejection, corrupt-byte retention and cross-process lock ownership.
Real stdio tests exercise record read/save/stale rejection/provider isolation,
selected replace/extend/override/empty/invalid policy, and fresh-thread reply
payload/invalid-policy rejection before spawning an agent. Native input tests
exercise restored viewed marks, bracketed-paste fencing and explicit save.

Storybook exposes saved, dirty, conflict and disposed prompt scenes through the
actual native renderer, including a 50x15 conflict frame. Reference enrichment,
staged draft publication, imported theme application policy, archived platform
recovery and complete onboarding/readiness remain with #305/#304/#306/#307.
This increment does not close those issues or authorize desktop removal.
