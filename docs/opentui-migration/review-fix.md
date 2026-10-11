# Terminal review fix delivery (internal)

The terminal uses `a` to preview an existing saved pull-request review fix.
The preview lists the canonical checkout, current/source branch, existing edits
and the pipeline effects. `y` explicitly approves; Escape or `n` dismisses.
Other shortcuts and pasted text do not approve a pending fix.

Rust issues a random, single-use token valid for five minutes. It binds the
provider/repository/PR/run, saved thread payload, canonical clone, tracked
content identity, untracked content and origin URL. Execution rechecks the
approval and the worker checks the fingerprint before the first mutation.
Changing a target clears the UI preview. Timeout does not trigger a retry.

Preview reads Git state without invoking agent tools or mutating the checkout.
Untracked content is limited to 2 MiB per file and 16 MiB in total. Links,
special files, invalid UTF-8 paths and incomplete snapshots are refused.
The reviewed diff is limited to 256 KiB. The complete saved conversation is
used in the Rust-built prompt; oversized payloads are refused, not truncated.
`NORN_CONFIG_DIR` selects an isolated non-secret settings directory for tests
and development, bypassing the OS/legacy config fallback. Defaults are unchanged.

The existing Claude fix pipeline remains in Rust: stash/restore local edits,
checkout, fetch/pull, agent edits and content verification. It leaves edits
for inspection and does not commit, push or publish automatically. Progress
and result fields are bounded; provider resume identifiers are omitted.
Cancellation terminates and reaps the owned command/process group before
reporting cancellation. Partial edits and retained stashes survive failures
and cancellation and are explicitly reported for inspection. Command execution
has a five-minute deadline. Windows uses process-tree termination; its native
smoke is part of the cross-platform lifecycle delivery.

Result inspection exposes summary, touched paths, reported checks and a
suggested commit message. These are agent reports, not proof tests were run.
Commit/push/conflict/sync controls and richer PR/Jira context remain separate
parity deliveries under #303/#306.

Verification uses isolated settings/SQLite storage, temporary clones and a
local bare origin, fake agents and real stdio. It covers stale untracked edits,
approval replay/expiry/cross-target use, edits to already dirty files, failure,
cancellation with descendants and preservation of partial work. Native-frame
scenarios cover approval at 120×30 and 50×15.
