# Verify native image display in an isolated physical terminal

## Owning ADRs

- `../../adr/0020-opentui-rust-stdio-workspace.md` (AC 4, 9).

## Scope

Run the existing synthetic image demo in an owned new native terminal window,
without touching existing sessions, user profiles/preferences, credentials,
provider/backend/configuration or actual review data. Capture only the owned
synthetic window if OS capture permission is already available. Verify detected
Kitty/Sixel protocol, BEFORE/AFTER image display, wide/narrow resize and clean
quit/terminal restoration. Record actual observed evidence and limitations.
Do not infer image display from terminal text dumps or application capability
claims. Preserve unobserved gates if automation or capture is unavailable.

## Exit Criteria

1. Physical-terminal evidence records exact application/version/OS, protocol,
   dimensions and observed before/after/resize result; screenshots contain
   synthetic content only.
2. Quit/restoration is verified in the owned test session, and only owned
   temporary resources/windows are cleaned up. Existing sessions stay intact.
3. If a defect is observed, fix it with native regressions and existing gates
   before declaring #302 complete. If a required observation is unavailable,
   document it rather than declare completion.
4. Root typecheck/test/Archgate and any changed UI frame/runtime gates pass;
   integrate sequentially after PR #347, with no renderer removal.

## Dependencies

- PR #346 provides the synthetic demo and syntax/clipboard parity.
- Integrate after PR #347; isolated evidence preparation while CI runs.

## Status

- Claimed by: default-agent, 2026-10-11, `issue-302-physical-image-smoke`.
- Blockers: final gate and PR CI/merge are required.
- Stopped:

## Delivery

Effective on merge to main of this image-fix PR. Implementation `a134f5d`,
based on PR #347 merge `58247d3` (both CI gates green). Physical observations
on macOS 26.2/iTerm2 3.6.11/OpenTUI 0.5.17 show both synthetic image colors
at 120×30, 50×20 and back to 120×30. Explicit quit exits 0 and restores
`stty -g` exactly; screenshots/diagnostics are committed under
`docs/opentui-migration/physical-images/2026-10-11/`. Only owned new windows
were controlled/captured and all seven were closed.

The physical check exposed single-cell image sizing and blank Kitty
placements on this exact combination. Panes now fill their allocated geometry
and auto iTerm selection prefers the observed working Sixel route, with
labeled blocks otherwise and explicit overrides preserved outside tmux.
Native regressions exercise both panes at wide/narrow/wide sizes and listener
cleanup, protocol preference, no-resolution/no-Sixel/tmux/override fallbacks.
Root typecheck, 123 tests/tooling and Archgate 17/17 pass; UI typecheck,
104 tests/611 assertions and 59 updated native frame baselines pass.
Lint passes after formatting the diagnostic JSON newline. Rust is unchanged.
No Norn review, provider/agent/keychain/backend or actual application data access.

Retina Sixel scaling occupies roughly half the pane in each axis. This is
explicitly recorded as a hardening limitation; pixel-perfect display and
physical Kitty/Linux/Windows are not claimed. Browser comparison remains
available in production for precise image review. #302 closure waits for
all relevant scope/gates, with installed asset resolution retained for #308.
