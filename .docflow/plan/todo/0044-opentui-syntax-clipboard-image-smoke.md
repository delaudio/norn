# Preserve syntax/clipboard and make native image smoke reproducible

## Owning ADRs

- `../../adr/0020-opentui-rust-stdio-workspace.md` (AC 4, 5, 9).

## Scope

Port the shipped syntax-highlighting and explicit review-output clipboard
workflows of #302. Reuse the existing language mapping/grammar semantics with
bounded selected-file work, preserving multiline source context and all diff
text/line identities. Highlighting is presentation only; unsupported/oversized/
failed syntax shows an explicit plain-text fallback and never hides source.
Keep parsing outside repeated row/event renders and fence stale results.

Copy review output only on explicit input. Rust remains owner of persisted
review/thread reads; the frontend invokes the native terminal clipboard bridge
without shell commands. Bound result/clipboard size, preserve target/run identity
and distinguish unavailable/unsupported/oversized content from a successful copy.
Do not copy a truncated excerpt while claiming the complete output was copied.
Use fake clipboard captures and synthetic review stores in automated tests;
never modify the maintainer's actual clipboard or credentials as a test.

Provide a deterministic, no-backend/no-credentials terminal image demo for
supported Kitty/Sixel physical smoke, paired with encoded-protocol/decoder/
unsupported-terminal tests and a clear manual checklist. Text frames alone do
not prove physical image display. Any physical check not performed remains
explicit, not fabricated.

## Exit Criteria

1. Language fixtures include TypeScript/TSX, Rust, multiline comments/strings,
   Unicode, unknown languages and size/error fallback; syntax preserves exact
   text and line mapping in unified/split views.
2. Syntax work/memory/lifecycle is bounded, cached for the selected file and
   stale-target fenced; native frames and streaming/render budgets still pass.
3. Explicit copy returns the selected stored review output without scope leakage;
   fake clipboard tests prove no clipboard action on load/navigation and honest
   unsupported/oversized/failure handling.
4. Isolated native image demo and manual instructions are runnable without
   provider/backend/config access. Actual physical smoke results are recorded
   only if observed; remaining evidence stays a cutover gate.
5. Root/UI/protocol/Rust/Archgate gates pass; #302 remains open until its gates
   actually pass, and neither old renderer is removed here.

## Dependencies

- Plans 0041/0043 for measured native rendering and event-load evidence.
- Integrate after the diff-navigation PR; isolated preparation while CI runs.

## Status

- Claimed by: default-agent, 2026-10-11, `issue-302-syntax-clipboard-smoke`.
- Blockers: diff-navigation PR must merge before integration.
- Stopped:
