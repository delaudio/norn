# Native readonly repository exploration

Internal migration evidence for the first bounded #359 slice, implementing
ADR0020 AC3–5/8–10. The native F8 tree and preview now use the real Rust API;
external-editor lifecycle proof remains a following slice. Desktop readonly
adapters remain until the final cutover.

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
| `repository.explore.find` | Non-overlapping case-insensitive substring in the selected text or raw diff | 32 matches/page; full preview hash; original UTF-8 byte and UTF-16 offsets, line and column |

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
control characters remain data; the native renderer escapes them
for display. Windows paths reject drive, stream and backslash syntax.

## Native presentation

F8 opens the selected configured repository; Escape/F8 returns to the preserved
review selection. Explicit `localPath` is the source: there is no guessed clone
discovery. Missing configuration, old backend capabilities or stale root/version
produce a visible refresh/settings recovery state. Both reader capabilities are
required because direct find jumps depend on the matching byte-offset contract.

The independent panel has a 32-entry paged file inventory with expandable folder
groups, all/changed filtering (`c`) and an explicit path filter (`/`, Enter to
apply, Escape to cancel). Tab changes focus; Enter selects a file or toggles its
folder. It preserves raw paths/status/rename source for requests and uses only
escaped text for display. Selection is fenced by full provider/root identity
and read generation; late results after closing or switching never apply.

Keys `1`, `2`, `3` show current content, HEAD/worktree diff and line blame.
Content/diff keep one 32KiB UTF-8 window and bounded previous-offset metadata.
`j/k` scroll rows, arrow left/right pan long source lines without splitting a
Unicode scalar, and PgUp/PgDn move the viewport before crossing a byte-window
boundary. Home returns to the start. Binary, oversized, deleted, missing and
unsupported previews are explicit; none silently becomes replacement text.
Blame preserves complete metadata and commit body; Tab focuses its detail
scrollbox for keyboard scrolling independently of the selected blame line.

Ctrl/Cmd-F finds within selected content or raw diff. Enter applies; `n/N`
navigates/paginates results. Original byte offsets jump directly into a new
pinned window, avoiding a full-file load in TypeScript; original UTF-16 offsets
retain highlight meaning. Applying an empty query clears the prior search.

For Markdown text, `v` toggles the native renderer. Its bundled Markdown worker
initializes asynchronously; tests and golden-frame generation wait for actual
heading/body output, rather than accepting an empty initial frame. Fenced-code
info strings are removed only from the rendered copy, keeping preview grammar
selection bounded and the exact original source intact. Rendered PgUp/PgDn
scrolls, then crosses byte windows at the scroll boundary. Raw source is always
available. No browser renderer or production mock substitutes for this view.

Paste is inert outside active text input. Active input rejects controls and is
bounded to 256 bytes; pasted shortcut-looking text cannot dispatch an action.

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
The UI slice adds actual F8/filter/file/content/diff/blame/find/return input to
that same production workflow. Native wide/narrow tests verify Markdown output,
full blame-message scrolling, safe paste and preserved review selection. Store
tests cover direct byte jumps, paged/stale inventory, provider switches, disposed
reads, unavailable backend and malformed replies. Twenty-two native explorer
Storybook frames cover wide/narrow tree, previews, find and unavailable states.

Existing desktop component/parser tests, shared raster regression tests, core
and CLI gates remain required. No provider HTTP, credentials, actual agent or
private profile is needed for these proofs. The complete explorer and explicit editor baseline is now preserved through
PR379/b15d80b, PR380/d623231 and PR381/29f5e2a. Exact editor source
e883fc9 passed all five CI38280325908 jobs, including actual GUI IPC and
terminal success/error/Ctrl-C/backend-death on Linux/macOS/Windows. Native
console modes/focus and owned editor cleanup are proven. This completes #359;
branch/worktree operations remain separately owned by #360.

## Approved branch operations under verification

The native explorer opens branch operations with `b`. It lists configured clone
state and bounded pages of local/remote references. Enter previews checkout;
`f`, `p` and `s` preview fetch, fast-forward pull and include-untracked stash.
A separate `y` press approves a single-use preview that expires after30seconds.
Pasted text never approves an operation. `Ctrl-D/U` scrolls the complete
approval or recovery details, including long branch descriptions. Each approval binds the full provider,
repository, canonical clone, reference, checkout bytes and effective Git config.

The Rust service preserves existing local branch commits when selecting an
origin reference; it creates a tracking branch only when no local branch exists.
Other remote references explicitly produce detached HEAD. Checkout and pull
refuse local changes and protect ignored files. Pull fetches its actual upstream,
checks local state again and uses fast-forward only. A divergent branch keeps
its commits. Stash saves tracked/untracked changes and reports the actual stash
object; ignored files remain. Existing conflicts block checkout/pull/stash;
fetch can update remote references while preserving conflict content/index.

An active write blocks competing workspace mutations. `x` or Ctrl-C requests
cancellation of its owned Git process tree. The panel waits for observed status
and shows actual after-state, failure or uncertainty, without replay or rollback.
If the backend exits, approvals are discarded and the outcome must be inspected
after restart. Existing captured review evidence, prompts and staged drafts stay
intact; matching local target options refresh without starting another review.

`make ui-repository-operations` / `just ui-repository-operations` run the real
backend against disposable clones and owned bare remotes. A bounded loopback
proxy maps registered fake GitHub/Bitbucket origins only to git-http-backend on
that bare repository. It never resolves or forwards provider hosts. Tests cover
actual state, configuration drift, approval expiry/reuse, cancellation, restart,
local commits, ignored files, existing conflicts and provider/root isolation.

These source changes are prepared for #360. Exact-head native CI on all three
supported systems and an actual verified squash merge remain required before
marking this parity row shipped. Linked-worktree creation adds no shipped UI
capability and is outside this migration slice.
