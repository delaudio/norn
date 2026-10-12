// Single catalogue of the actual global shell bindings, used by the help
// overlay. Keep in sync with the Shell key handler; do not list bindings that
// do not work.

export interface HelpBinding {
  keys: string;
  label: string;
}

export const HELP_BINDINGS: readonly HelpBinding[] = [
  { keys: "F2", label: "Track a provider pull request" },
  { keys: "F3 / F5", label: "Preview the provider snapshot" },
  { keys: "F4", label: "Open the working-tree comparison" },
  { keys: "F6", label: "Choose a review profile" },
  { keys: "F7", label: "Open saved local snapshots" },
  { keys: "F8", label: "Open the repository explorer" },
  { keys: "Tab", label: "Cycle panes" },
  { keys: "j / k", label: "Move the selection" },
  { keys: "h / l", label: "Scroll the diff horizontally" },
  { keys: "PgUp / PgDn", label: "Scroll the detail pane" },
  { keys: "Enter", label: "Start a review or select" },
  { keys: "f / g", label: "Cycle finding filter / group" },
  { keys: "t / v", label: "Toggle threads / conversation" },
  { keys: "s / e", label: "Toggle split mode / expand diff" },
  { keys: "/ ] [", label: "Search diff, next and previous match" },
  { keys: "c / p", label: "Commit / push (preview then approve)" },
  { keys: "u / o", label: "Sync / resolve conflicts (preview)" },
  { keys: "a", label: "Address findings with a fix (preview)" },
  { keys: "r", label: "Reply to the review conversation" },
  { keys: "b", label: "Open the browser diff viewer" },
  { keys: "y", label: "Copy the review output" },
  { keys: "H / M", label: "Global review history / closed-PR metrics" },
  { keys: "m / n", label: "Settings / workspace scope" },
  { keys: "i / w", label: "Draft a finding / toggle references" },
  { keys: "z / x", label: "Refresh operation / cancel" },
  { keys: "? ", label: "Toggle this help" },
  { keys: "q", label: "Quit" },
];
