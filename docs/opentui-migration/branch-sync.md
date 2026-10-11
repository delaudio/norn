# Internal migration evidence: terminal branch sync

This is an internal builder record, not product documentation.

The saved review's source/destination branches and canonical clone are used.
`u` previews sync; `o` previews conflict resolution. Only `y` in the preview
starts an operation. Approvals are short-lived, one-use and action-specific;
they bind checkout contents, index, local refs, origin configuration and pending
Git operation identities. Changing the selection dismisses its preview.

A new merge requires a clean checkout on the saved source branch. The terminal
currently refuses dirty clones rather than stashing them. This restriction is
explicit in the preview and remains a parity/recovery follow-up; do not silently
claim the desktop stash/restore path has been ported. Existing merges are accepted
for conflict resolution only when MERGE_HEAD equals the saved destination's
origin ref. Other Git operations must be inspected first.

Sync fetches source/destination, fast-forwards source and merges the destination
with `--no-commit --no-ff`. It never commits or pushes automatically. Conflict
resolution uses the native Claude adapter, verifies that Git has no unmerged
entries and collects actual changed paths rather than trusting agent-reported
paths. Git/agent mutations use bounded output, cancellation, process-group
cleanup and a five-minute command deadline. Read-only Git inspection is bounded
to one MiB and ten seconds.

A merge commit cannot be partial in Git. Its preview therefore approves the
complete changed file set, and a new unrelated file/edit causes refusal. The
worker validates that set again before committing. Ordinary fix commits retain
`--only` and exclude unrelated stage. Both paths require separate explicit push
approval with the exact recorded HEAD and non-forced source refspec.

Failures/cancellation preserve the clone and pending merge for inspection;
there is no automatic abort/reset, stash deletion, commit or push. A successful
agent response with unresolved Git entries is an error. The result panel shows
actual native phase and bounded summary; claimed agent checks are not independent
verification. Persistent recovery after backend restart remains a hardening task.

Local bare-origin subprocess tests cover clean merge → separate merge commit →
separate push, conflict preservation → separate agent resolution → merge commit,
stale/wrong-action approvals, dirty checkout refusal, unresolved agent claims,
and refusal of unrelated stage during merge commit preview. No live provider or
user repositories are touched by these tests.
