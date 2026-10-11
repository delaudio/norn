# Canonical reviewer references (internal migration evidence)

Owning decision: [0020](../../.docflow/adr/0020-opentui-rust-stdio-workspace.md).
Scope: #305 canonical imported-content consumption, coordinated with #306
reference enrichment and #304 staged publication.

The terminal `w` panel uses the existing typed `workspace.read` paginated
snapshots and `workspace.mutate` per-record CAS. Its key includes provider,
workspace, repository and positive PR ID. A working-tree target has no scope.
All five reference kinds and optional metadata fields are editable; imported
IDs/source/creation time survive edits, updated time changes on explicit save.
Removal requires a separate confirmation. Conflicts retain edits and never retry
a write automatically; generation fences drop disposed/changed-scope results.

The frontend validates bounded known fields, type/source, safe timestamps,
positive PR numbers, unique IDs and unsupported controls before rendering. At
most 20 reference summaries mount; the selected detail/form uses a bounded
record body. Native completed-frame geometry follows the selected field after
wrap/resize and scrolls body input to its end; a signature excludes scroll offset
to avoid a draw loop and the listener is owned/unsubscribed. PR scope uses scalar
identity dependencies, so a same-scope metadata/picker refresh retains the form.
The real Rust service owns canonical writes, validation, atomic replacement and
persistent cross-process storage locking.

Rust resolves saved references after prompt policy/override for provider PR
inline reviews and reply fallback payloads. JSON preserves typed text and a
Markdown fence widens beyond the longest backtick run. Empty records add nothing;
serialized reference context over 256 KiB fails before spawning an agent. It is
never silently truncated. Other providers/PRs cannot contribute context, and
local reviews do not infer reference scopes. A successful Claude resume sends the
new message and keeps the existing conversation; new saved reference context is
used only for new/fresh-session payloads. The backend does not fetch URLs or read
reference paths. Provider enrichment/automatic detected references remain #306.

Evidence uses synthetic canonical data, Git repositories and fake agent scripts
with isolated config/data. Native input proves restored text, paste fencing,
explicit saves/removal, listener cleanup, wrapped-field navigation/resize and
same-scope refresh retention. Rust fake-agent stdio proves new/fresh reply
context, wrong provider/PR exclusion, widened fences, 256 KiB rejection before
agent invocation and resumed conversation semantics. Core 627 tests (2 intentional
ignored), no-GUI dependency guard, 46 backend subprocess tests and strict
all-feature/all-target Clippy pass. UI/frames and root gates are recorded in the
owning plan. No Norn code review, actual provider/keychain or live publication.

Six dedicated native frames/stories cover saved/compact/edit/conflict/remove/
working-tree states; theme preview frames also gain named stories. The staged
comment UI/publish flow remains #304, and archive-native runtime/recovery limits
remain explicit before cutover. This increment does not close #305 or authorize
renderer removal.
