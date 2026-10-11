# Native pull-request browser

Internal evidence for the second bounded #357 delivery, plan 0055, based on
PR #365 merge `2f2cd50`. Integration is effective only after the verified squash
merge of this delivery. #357/#306 remain open for pinned pre-review files/diffs
and explicit at-most-once PR approval/native browser opening.

## Behavior and bounds

F2 opens native OpenTUI browsing. Configured repositories and saved/imported PR
scopes contribute explicit provider/owner/repository identities; local high-bit
IDs do not become remote PRs. Repository All/individual, Open/Draft/Merged/All,
actual provider author IDs/current user and a 256-byte query are independent
fields. Tab selects fields, arrows change filters, j/k select list rows, Enter
opens detail, Space selects the workspace, n loads the selected repository's
next page, c folds its group and r refreshes explicitly. Escape/F2 close, m
opens settings, q/Ctrl-C quit, including while reads are pending.

The scheduler runs at most two native reads, including across rapid filter
changes and close/reopen. Replaced queues are discarded; generation checks
fence list, detail and user responses. Detail has priority after current reads
finish. Rust HTTP deadlines still bound a pending worker. Each group maintains
its own values, numeric cursor, page, loading/error and collapse state. Empty
filtered pages retain next-page actions. Duplicate IDs across live pages update
the same PR, never alias another provider. Refresh reloads only previously
requested pages and preserves full-scope selection if still present; disappearance
shows an explicit notice without selecting a different workspace.

Inventory is at most 10,000 identities/8 MiB. All-repository reads require at
most 100 groups; larger inventories show a choose-repository action before any
read. Loaded results are at most 5,000/8 MiB, pages remain bounded by the backend
contract (100 pages, 30 requested records/page). Exceeding a bound leaves the
previous loaded data/error visible and requires narrower filters. Author choices
retain up to 5,000 actual provider/ID identities across filtering; identical
display names never become an identity. Unknown current-user identity is explicit,
and a successful recovery clears that provider's previous identity error.

Only visible rows mount (at most 32); detail scrolls separately. Detail displays
actual description, branches, revisions, state/draft, timestamps, author and
reviewer/approval evidence. URL copying is an explicit OSC52 terminal request;
delivery is not proof the terminal updated its clipboard. No metadata request
starts AI, changes Git, publishes, or writes canonical data.

Workspace selection preserves existing actual saved-run IDs and canonical
scopes. Missing remote PR identities are transient, bounded to 1,000 and retained
through settings/operation picker rebuilds; they carry no invented review run.
An unreviewed PR surfaces the existing missing-reviewed-range limitation and
cannot fall back to working-tree files/diff. A subsequent required delivery
provides immutable provider revision pins and pre-review diff access. Browser
refresh does not alter the workspace; opening/selection is guarded while a reply,
fix approval or another editor is active.

## Owned evidence

Twelve new store/native-input tests cover cross-provider identities, actual
authors, independent and empty-page pagination, refresh/selection, bounded rapid
filter generations, late detail disposal, errors/recovery, Unicode query limits,
large inventory, capability mismatches, saved-run preservation and transient PRs.
Native input covers F2/Tab/arrows/query paste/Enter/Space, wide-to-narrow resize,
Escape and settings/Ctrl-C during pending reads; no review or publication starts.
The fake provider is owned synthetic data and never accesses credentials/HTTP.
The previous bridge tests remain responsible for malformed/oversized/cursor/URL
responses and native rate-limit/HTTP semantics.

Twelve new native-cell Storybook frames cover list/detail/empty/error/loading/
unavailable identity at 120x30 and 50x15. All 105 frames match the generated
baseline. Visual inspection covered these scenes and existing wide/compact
workspace layouts. Of the previous 93 frames, 39 change only their final help
line to advertise F2; no other previous line or dimension changes. Cell layout
inspection is not a new physical-terminal glyph or platform-runtime claim.

React Doctor ran before/after: its confirmed Intl allocation warning was fixed
by reusing a segmenter. The remaining unknown DOM properties are false positives:
`text`/`box`/`scrollbox` are OpenTUI renderables, with their native properties
checked by the OpenTUI TypeScript contract and exercised by its native renderer.
Two maintainability warnings concern the bounded panel's combined keyboard/render
flow; there are no reported React errors. No rules/configuration were suppressed.

Local gates: root typecheck/123 tests plus tooling/lint/Archgate 12/12, protocol,
UI typecheck/160 tests/105 native frames, Rust 672 core tests plus two existing
ignored/52 stdio/six isolated CLI/five onboarding/three binary tests, strict
all-feature/all-target Clippy/fmt and no-GUI dependency guard. Exact-head CI and
the verified squash merge remain required to ship; this is partial #357 parity,
not approval or pre-review diff delivery.
