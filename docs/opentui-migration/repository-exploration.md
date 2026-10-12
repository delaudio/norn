# Native readonly repository exploration

Internal migration evidence for the first bounded #359 slice, implementing
ADR0020 AC3–5/8–10. Native tree/editor presentation remains a follow-up; this
slice provides its real Rust API and preserves the desktop readonly adapters.

## Authority and methods

`repository.explore.info` receives a complete configured provider/workspace/repo
identity. It requires exactly one matching configured checkout, its canonical
Git root and matching origin. It returns an opaque root identity, branch and
HEAD; the protocol never accepts a caller-provided absolute filesystem path.
Every subsequent request repeats that context and resolves it again. A moved
configuration, mismatched provider or ambiguous clone fails explicitly.

| Method | Meaning | Bound / continuation |
| --- | --- | --- |
| `repository.explore.files` | Sorted tracked/untracked inventory including deleted, renamed and conflicted paths; rename source preserved | 50,000 files, 32 rows/page; hash pins bounded path/status/index/HEAD and exact filters |
| `repository.explore.content` | Current worktree text or explicit binary/tooLarge/missing/deleted/unsupported state | 512KiB text limit; 32KiB UTF-8 pages with complete byte hash |
| `repository.explore.diff` | HEAD/worktree diff, including rename source; unborn and untracked use Git no-index | 8MiB complete patch limit, 32KiB pages with complete byte hash |
| `repository.explore.blame` | Current selected text's line blame with author, mail, timestamp, summary and complete commit message | 32 lines/page, 32KiB/message, bounded raw metadata and adaptive serialized page size |
| `repository.explore.find` | Non-overlapping case-insensitive substring in the selected text or raw diff | 32 matches/page; full preview hash; original UTF-16 offsets, line and column |

Path filtering and changed-only filtering preserve the shipped explorer meaning.
Find is not a new whole-repository content search. Preview paths are not finding
anchors and grant no fix, publication, editor or other write authority. These
are live explorer reads, not immutable stored review evidence.

Git uses bounded cancellation-aware argv calls, literal pathspecs, no shell,
disabled external diff/textconv, a 20-second aggregate request deadline and two
reader workers. Shutdown/EOF cancels their owned Git work. Responses echo the
whole request and are checked against generated strict DTOs plus semantic page,
identity, version, byte and row constraints in the production client.

Secure opened-file handling is shared with local captured images. Relative path
validation rejects traversal; canonical destinations outside the selected root
are rejected before reading. Internal symlinks remain readable through a
canonical destination and confined no-follow handle. Non-UTF-8 bytes are never
silently replaced. Unicode, spaces, option-like names and legitimate Unix
control characters remain data; the upcoming native renderer must escape them
for display. Windows paths reject drive, stream and backslash syntax.

## Verification

Owned temporary Git tests cover status/rename/conflict inventory, unborn HEAD,
literal paths, hostile Unix names, outside/internal links, UTF-8/binary/large
states, cancellation, complete messages containing the legacy delimiter,
10,000-line paged blame, provider distinction, moved/ambiguous clones and stale
continuations. Limit tests reject an inventory above 50,000 and a patch above
8MiB without exposing a partial result.

A real stdio subprocess exercises all six methods and verifies stale content,
inventory drift, foreign provider, malformed path rejection and continued
control/shutdown. The existing native OpenTUI/production transport/Rust workflow
now also calls all six explorer methods, including actual untracked no-index
diff and binary detection, in its Linux/macOS/Windows CI lanes. Local macOS
runtime passed; other platform results require the PR's own exact-head CI.

Existing desktop component/parser tests, shared raster regression tests, core
and CLI gates remain required. No provider HTTP, credentials, actual agent or
private profile is needed for these proofs. #359 remains open for native tree,
navigation, find presentation and the actual external-editor lifecycle proof.
