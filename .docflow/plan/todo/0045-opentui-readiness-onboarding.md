# Complete terminal readiness and first-run onboarding

## Owning ADRs

- `../../adr/0020-opentui-rust-stdio-workspace.md` (AC 3, 5, 6, 9).

## Scope

Finish the remaining #305 readiness/onboarding presentation using existing
Rust configuration/credential/tool ownership. Expose a bounded status-only
readiness result: configured repository count, Git/tool availability, selected
agent readiness, canonical data/config location availability and credential
presence/source. Return neither stored credentials nor raw tool output.
Keep machine checks separate from repository policy scans; do not inspect an
unselected repository or rewrite configuration. Presence on PATH is not proof
of authentication or a working provider connection.

Show first-run guidance in the terminal and a refreshable readiness section in
settings. Masked credential editing remains separate; discard/save semantics
and arbitrary custom models remain intact. Surface unsupported native-store
platforms without a plaintext fallback. Preserve headless doctor/setup/auth
commands and documented recovery routes.

## Exit Criteria

1. Existing configured users load settings/status without entering stored
   credentials again; first-run users receive actionable configuration/tool
   guidance, with no implicit review, login or provider publication.
2. Readiness requests use bounded owned work and timeout/cancel/shutdown
   handling. Credential values and raw child outputs are absent from all
   status/error/frame payloads. Tests use fake probes and isolated data only.
3. Settings refresh is generation fenced, coalesces repeated input and cannot
   overwrite a cancelled/new form or erase unsaved edits. Save/credential
   mutations update status only after explicit successful acknowledgements.
4. Root, UI/native frame, protocol and Rust/core/Clippy gates pass, plus existing
   isolated first-run CLI regressions. Remaining data-consumption/recovery and
   broader parity gaps remain explicit until verified; #305 is not closed
   based solely on this readiness increment.

## Dependencies

- #301 is merged; plans 0037/0038/0042 own settings/import/record foundations.
- Integrate after the syntax/clipboard PR; isolated preparation while CI runs.

## Status

- Claimed by: default-agent, 2026-10-11, `issue-305-readiness-onboarding`.
- Blockers: syntax/clipboard PR must merge before integration.
- Stopped:
