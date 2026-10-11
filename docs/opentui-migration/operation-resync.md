# Internal migration evidence: operation identity and saved state

This is an internal builder record, not product documentation.

An accepted start/reply/fix belongs to its original target even if the picker
changes before the response arrives. The operation panel keeps the original
label and `x` cancels that exact ID. Its text is never added to another target's
saved conversation/findings. Unresolved mutation/start requests prevent another
start while ownership is unknown. A failed request can be ambiguous and never
causes an automatic retry.

Operation events must match both ID and original target; duplicate/old sequence
numbers are ignored. Gaps trigger coalesced authoritative status reads with at
most one request in flight plus a pending follow-up, a bounded 100-line log and
snapshot refresh only for the matching selected target. Status reads intentionally
await dependent responses in a loop; parallel reads would permit stale snapshots
to overwrite newer ones. React Doctor's await-in-loop warning was checked against
the gap/burst tests and is not suppressed.

While the mounted UI has accepted/running work, a one-second status poll recovers
missing terminal events. It stops with the mounted effect on state transition or
unmount. `z` explicitly refreshes operation status and the selected saved snapshot;
it never starts or retries a review, fix or publication.

Successful review/reply completion refreshes the saved target list and adopts the
new run only if the original exact selection/generation is still active. Stable
finding IDs and selected file paths survive snapshot refresh. Failure/cancellation
keeps the submitted reply draft for the original composer only. Fix results remain
fenced to their target, and returning to that target inspects native fix state.

Tests use delayed, controlled transports to cover late start/reply/fix ownership,
correct cancellation IDs, wrong targets, old/duplicate sequences, burst gaps,
missing terminal replies, new saved run selection, stable finding selection and
concurrent request prevention. No real provider or user credentials are accessed.

## Opt-in real-provider smoke (manual, not run by this agent)

Use a disposable repository/PR and isolated Norn configuration/data roots. Configure
one supported provider/model at a time (Claude, Codex or OpenCode) using the existing
Rust setup/config commands and external agent authentication. Build the core backend
and run the OpenTUI workspace with its explicit backend path. Select the test PR;
press Enter only when ready to invoke a paid/live agent. Navigate a diff while the
review streams, confirm its saved run/thread/findings, compose a follow-up and test
cancellation. Switch target during an intentionally slow run and confirm the original
operation label/cancel ID. Repeat on a fresh process with the saved thread selected
to inspect resume behavior. Stop before fix/commit/push/publication unless separately
intended. Record provider/model, platform, bounded timing, outcome and redacted logs;
never export authentication values or private agent resume tokens. This procedure
is an opt-in verification route, not evidence that a live smoke has passed.
