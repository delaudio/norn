# Saved review references

Select a pull request and press `w` to open References. Imported records use the
same local store as newly saved references, separately for each provider,
repository and PR. Working-tree targets do not invent a PR reference scope.

Use j/k to select a record, e or Enter to edit, and n to add. In the form,
Tab/j/k moves between fields and Enter edits a field or cycles the reference
kind. Supported kinds are reviewer note, Jira, Notion, repository and related
pull request. Existing source metadata and creation time are retained.

Pasted text goes only into the active field. Enter keeps a field; Shift+Enter
adds a body newline; Ctrl+U clears that field; Escape undoes the field or discards
the unsaved form. Ctrl+S explicitly saves the record. Delete with d followed by
y approval; n/Escape dismisses that confirmation. PageUp/PageDown scroll detail
or form content. Refreshing the same PR retains an unsaved form; switching scope
or closing discards its transient edits.

References are local reviewer context, not provider comments. This panel does
not fetch a URL, inspect a local context path, publish or access credentials.
New PR reviews and fresh-session replies receive the selected PR's saved records
from Rust. Repository references describe read-only context; the agent must state
any limitation rather than inventing unavailable reference contents. A resumed
conversation keeps its earlier context; saving a reference does not rewrite it.

Each body is limited to 64 KiB UTF-8, URLs/paths to 4 KiB and other text fields
to 1 KiB. A PR may store at most 1,000 references. The complete serialized review
context is limited to 256 KiB; if it exceeds that size, review start/fresh-session
reply fails before agent launch. Reduce references after exporting a local backup
rather than relying on silent truncation.

If another process changed the record, an edit is retained and saving fails.
Keep that text, close/reopen the panel to inspect the current records, and
reconcile explicitly. A failed or uncertain acknowledgement never triggers an
automatic write retry. Removing a record changes only that provider/PR list;
other scopes and the original desktop export remain intact.
