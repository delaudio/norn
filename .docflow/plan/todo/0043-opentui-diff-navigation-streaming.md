# Complete diff navigation state and streaming-load evidence

## Owning ADRs

- `../../adr/0020-opentui-rust-stdio-workspace.md` (AC 4, 5, 9).

## Scope

Finish the bounded native diff interaction requirements of #302 before comment
publication: per-target/file/mode scroll positions, plain-text search with
next/previous matches and correct source anchors in unified/split views, and
bounded navigation while agent events arrive. Preserve the existing Unicode
wrap geometry, image bounds and authenticated browser owner. Do not fabricate
syntax/clipboard parity or physical image display proof; inventory actual old
capabilities and record explicit evidence/remaining platform checks.

Search is presentation state and never initiates review, mutation or publication.
Input/paste must isolate ordinary shortcut characters, allow explicit apply or
cancel and report empty/no-match outcomes. Search results/scroll snapshots must
not survive a changed target identity or attach to another file. Preserve large
diff collapse/expand behavior and clear explanations when a requested result is
outside the currently collapsed view.

Record a repeatable native streaming-load test/benchmark that asserts real
rendered diff content, bounded mounted rows, navigation correctness and event
ownership under a burst. Compare against the budgets in the rendering baseline;
separate deterministic rendering evidence from physical terminal image smoke.

## Exit Criteria

1. Returning to a file/mode restores its own scroll position without another
   file or target's anchor leaking. Layout/resize and anchor/search requests
   have explicit precedence; retained scroll state is bounded and cleaned up.
2. Native input tests and frames cover search apply/cancel/paste, no matches,
   next/previous wrapping and source-line navigation in both diff modes.
3. Burst-stream plus real diff navigation remains bounded/responsive with
   target/operation fencing; repeatable evidence reports workload and limits.
4. Root/UI/protocol/Rust/Archgate gates pass. Physical image terminal checks and
   any additional parity items remain explicit until actually verified.

## Dependencies

- Plans 0040/0041 merged at PRs #342/#343.
- Integrate after PR #344; isolated preparation while its CI is pending.

## Status

- Claimed by: default-agent, 2026-10-11, `issue-302-diff-navigation-streaming`.
- Blockers: None; PR #344 merged at `5eb33fd` with both CI gates green.
- Stopped:
