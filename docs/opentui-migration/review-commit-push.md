# Terminal fix commit and push (internal)

After a successful native fix, `c` opens a commit-message composer initialized
from the agent suggestion. Enter requests a preview; it does not commit. The
preview names the selected files and exact message. `y` authorizes it; Escape
or `n` dismisses. Messages are capped at 4096 Unicode characters.

Rust validates paths, binds the approved file list and content/index identity,
and commits with literal pathspecs and `--only`, preserving unrelated staged
files. The fix stash is restored with its index so staging that predates the
fix survives. Empty, external and unsafe path sets are refused. Selected paths
are bounded to 100 entries and 32 KiB in total.

`p` requests a separate push preview after a successful commit. It binds the
source branch, exact committed HEAD and origin fetch/push URLs. The operation
uses an explicit source refspec to origin with no force flag; multiple origin
push destinations are refused. Commit and push approvals cannot authorize one
another or a fix and are never reused after execution, expiry or failure.

The result panel exposes the created commit SHA. Cancellation reaps the owned
Git/hook process group. A failed or interrupted push is an ambiguous remote
outcome: inspect local and remote branches before retrying. No fix, commit or
push action publishes review comments.

Verification uses fake agents, temporary Git clones and a local bare origin,
not live providers. It covers the complete fix→commit→push flow, exact Unicode
message handling, preservation of unrelated staged files before and after the
fix, path validation, stale/wrong-action/replayed approvals and cancellation
during a pre-commit hook. Composer/approval input tests and native frames cover
commit composition, commit preview and wide/compact push preview.

Remaining #303 deliveries include conflict resolution and branch sync. Final
operation recovery and cross-platform lifecycle proof belong to #307.
