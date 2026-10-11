# Staged comments

Select a pull request and press `d` to open its local staged comments. Imported
drafts use the selected provider, repository and PR scope. General notes,
file/inline comments, replies and local-only observations retain their saved
identity, original creation time and review metadata.

Use `j`/`k` to select, `n` to add a general comment, or `e`/Enter to edit.
In the form, Tab/`j`/`k` selects a field and Enter edits it; Enter on the
publication mode cycles general, inline, file and local-only. `to` is a new-side
line and `from` an old-side line. Keep the side, path and parent appropriate for
the eventual provider comment. Saving does not check that the remote PR still
has that anchor; publication must check it separately.

Enter keeps the edited field, Shift+Enter adds a body newline, Ctrl+U clears it,
and Escape undoes that field. Ctrl+S saves the form locally. Escape outside a
field discards the unsaved form. Paste is treated as text and rejects unsupported
controls or inputs beyond the field's byte limit. Empty body/path fields remain
empty strings; line fields are optional positive integers.

Press `d` in the list to request removal and `y` to confirm, or `n`/Escape to
cancel. PageUp/PageDown scrolls the selected comment's full body. Editing keeps
the saved finding/run/fingerprint and reviewed base/head; those metadata fields
are read-only in this form. Changing the selected PR/provider clears transient
edits, while refreshing metadata for the same PR retains them.

If another writer changed the record, the save fails and keeps your unsaved
form. Preserve the edited text before closing/reopening to inspect the latest
saved record. A backend disconnect may leave a mutation's outcome unknown;
reopen the panel to inspect persisted content before retrying.

This editor performs local saves/removals. Provider publication requires a
separate preview, approval and result reconciliation; a locally saved draft is
never an acknowledgment that a remote comment exists.

## Stage a saved finding

Select a finding in a pinned PR review and press `i`. Use `j`/`k` to choose
inline, file, general or local-only, then Enter to prepare the actual comment
from the saved review. Inspect its body, path/side/line and reviewed revisions.
Press `y` to stage that preview locally, `n` to dismiss it, or Escape to close.
Paste cannot trigger these actions. Enter refreshes a preview after an error.

Inline/file positions need a supported relative path and recorded full base/head
revisions. Unsupported positions produce an error; choose general/local-only
explicitly or rerun the review. Staging does not establish that the remote PR
still has the same revision; publication must check that separately.

If a draft is already linked to this finding, its saved edits and mode are
retained. The preview tells you this and displays the existing comment. `d`
opens the scoped draft list to edit it. A saved publication acknowledgment
prevents creating another finding draft; a manual follow-up remains an explicit
separate action.

Changes to the previewed content or draft record block staging. A refresh of
metadata for the same pinned review keeps the preview, while selecting another
target clears it. Closing an in-flight save does not undo a durable write;
inspect the draft list before retrying after an interruption. CRLF bodies keep
their original saved text and display as ordinary terminal lines.
