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
- Blockers: physical observations and PR #347 merge are required.
- Stopped:
