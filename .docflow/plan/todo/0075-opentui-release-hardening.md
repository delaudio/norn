# OpenTUI release hardening

## Owning ADRs

- `../../adr/0020-opentui-rust-stdio-workspace.md`

## Scope

Issue #307. Preserve existing owned native journeys and corrected HTTP fixtures;
close client framing failures, enforce strict bounded incoming frames, reject
pending requests and reap the owned backend on protocol failure. Add complete
headless/core and transport regression gates on the supported native OS matrix.
Record native lifecycle, visual and streaming performance evidence with honest
platform limits. No live provider/agent, private credentials or release publication.

## Exit Criteria

1. Oversized terminated/unterminated/Unicode frames, malformed DTOs and protocol
   mismatch cannot grow retained buffers, dispatch events or leave pending work
   and an owned backend alive. Fragmented valid frames remain accepted.
2. Owned native review/diff/finding/conversation/draft/publication/restart tests,
   race/cancel/crash/EOF tests, renderer frames and PTY/ConPTY restoration pass.
3. Complete core and stdio gates run on Linux/macOS/Windows. Streaming navigation
   retains bounded rows/logs/memory and produces repeatable benchmark artifacts.
4. Root/UI typecheck/tests, protocol drift, lint and Archgate pass. Release
   report records actual evidence; installed artifacts remain #308.

## Dependencies

PR394 must merge before this delivery integrates. #306 desktop parity retired by
maintainer; its terminal/data/security dispositions remain in force.

## Status

Claimed 2026-10-14 by default-agent on fix/opentui-hardening.
