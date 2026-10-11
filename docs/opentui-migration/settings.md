# Terminal settings delivery evidence (internal)

Owning decision: [0020](../../.docflow/adr/0020-opentui-rust-stdio-workspace.md).
Owning issue: #305. This is a partial delivery, not the desktop cutover gate.

## Interaction and ownership

Press `m` in the workspace to open settings. `j`/`k`, arrows and Tab navigate;
Enter edits text or cycles a finite option. Text and bracketed paste stay inside
the active field. Ctrl+U clears its draft; Enter keeps the draft and Escape undoes
that field. Outside an editor, Escape or `q` closes and discards unsaved fields.
Ctrl+S explicitly saves preferences. Arbitrary model and effort IDs are preserved;
empty optional fields mean the existing default. Add/remove changes the repository
list only and never deletes clones or review history.

Rust owns the settings file, validation and SHA-256 version. Saves reject stale
versions and invalid/oversized existing files. A same-directory temporary file is
flushed and atomically replaces the canonical file. Unrelated existing settings
are preserved; legacy files remain read-only input. This does not guarantee a
cross-process compare-and-swap against a concurrent unrelated application write.

Credential fields are independent of preference saves. Ctrl+K explicitly writes
the selected credential through the existing native OS store. Presence and source
are returned, never the stored value. New input is transient and masked in every
rendered frame; close/dispose clears it. `x` followed by `y` removes the selected
keychain credential; environment variables remain untouched. Native keychain
support gaps remain tracked by #251; there is no plaintext fallback. Tests use
fake credential access and never inspect the developer's OS credential store.

Existing configured repositories, provider/model preferences and credentials
continue to be used. The empty workspace points to settings and existing CLI setup
and auth entry points; adding a repository is explicit in this form. Preference
saving reloads workspace targets and applies theme/diff defaults. The automatic
sync interval is persisted here; terminal polling parity is a subsequent gate.

## Verification and remaining #305 work

The native settings frames cover wide/narrow forms, masked input, load failure and
light theme. Tests cover arbitrary custom models, stale writes, invalid source
preservation, symlinks, legacy fallback, unchanged unrelated settings, secret debug
redaction, explicit credential writes/clear and field/shortcut isolation.
The real stdio test uses temporary config/data directories without credential
access. Malformed protocol diagnostics are generic because deserialization errors
can echo submitted values.

Desktop-local drafts, references, viewed state and user prompts still require a
bounded export/recovery/import path with preview, backup and conflict handling.
Credential enrollment UX, full readiness/connection checks, shortcut help and
automatic sync behavior remain subject to the final #305/#306 parity audit.
No legacy data is removed and #305 must remain open after this delivery.
