# Current Focus

Git is authoritative. Durable history lives in WORKLOG and plan/done.

## Active state — 2026-10-14

Single writer, no delegation or Norn review. #394 merged as c5bcabf.
Claim plan 0076 on fix/default-headless-opentui in
/private/tmp/norn-current-repo-first. The maintainer explicitly overrides the
old desktop-default instructions: ordinary builds are core/CLI/stdio plus
OpenTUI, optional Tauri/Ratatui checks remain separate. Preserve browser diff.
#396/plan 0075 is paused as a draft on fix/opentui-hardening; its followup is
in an owned stash. No concurrent changes or merges from that branch.
Preserve root untracked .norn.yaml, user stash and all stored data. Do not
publish releases. ADR0020 remains Accepted until full migration cutover.

Local release build and isolated installed runtime smoke pass (including real
PTY handoff and served browser assets). Core 776, UI 318, frames 223, root
tests/tooling, default graph, Clippy and Archgate pass. Preparing PR; await CI.
