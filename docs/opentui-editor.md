# Open a repository file from the terminal workspace

Press **F8** to browse a configured local checkout. Select a file and press
**Enter** to preview it. Press **o** to open it in a graphical editor, or
**Shift+O** to open your configured terminal editor.

Graphical opening tries `zed`, `cursor`, then `code`, followed by the operating
system's application association. A system application opens the file without
selecting a source line; the workspace reports this limitation. A graphical
window keeps running independently of Norn.

Terminal opening uses `VISUAL`, falling back to `EDITOR`. Configure an editor
executable, optionally followed by arguments, rather than a shell command.
An existing absolute executable path can contain spaces. Norn supports source
line positioning for vi/vim/nvim, nano, emacs, micro, hx/helix, zed, cursor and
code. Other editor commands can open a file without a requested source line;
unsupported line positioning produces an explicit error.

The current text or blame line supplies the source position. Diff and rendered
Markdown views open the file without guessing a source line. Binary files can
be opened externally if the selected application supports them. Missing files
and paths outside the configured checkout are refused.

While a terminal editor is running, it owns terminal input. Exit the editor or
use its normal interrupt behavior to return to Norn. The selected file and
review navigation remain available; press **r** to refresh the file preview
following an edit, or **Esc** to return to the review workspace. Finish an
active review or filesystem operation before opening another editor.
