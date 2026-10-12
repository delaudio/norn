# Explicit native external provider PR navigation

## Owning ADRs

- `../../adr/0020-opentui-rust-stdio-workspace.md` (AC 3–5, 8–10).
- `../../adr/0002-http-in-rust.md`.
- `../../adr/0003-credentials-keychain.md`.

## Scope

Complete explicit external GitHub/Bitbucket PR navigation from the native PR
browser without borrowing Pierre/diff state or accepting a UI URL. Preserve
full repository/provider/PR ownership, native launcher lifecycle, strict protocol,
credential boundaries and responsive control. Bitbucket UUID repository identity
must resolve through bounded authenticated native repository metadata to an
actual public slug URL instead of assuming API UUID paths work in the web UI.
This bounded retained provider capability lives under #306. Approval and profile
catalog/editor are subsequent work; this alone does not close provider parity.

## Exit Criteria

1. A typed provider.browser request accepts only explicit validated provider,
   workspace/repository and ordinary positive PR identity; rejects reserved local
   IDs/unsafe path/control/unknown fields before launcher or credentials.
2. Rust generates only allowlisted canonical HTTPS provider PR URLs. UI cannot
   supply host/URL. GitHub and ordinary Bitbucket slugs need no provider call;
   Bitbucket UUID aliases resolve via bounded authenticated native repository
   metadata, with trusted identity/provenance and normal read gate/deadline/error
   contracts. No redirects, shell interpolation or arbitrary metadata URLs.
3. Invoke existing platform-native opener only on explicit user input. Report
   launch-request acknowledgment/error; do not claim page visibility. Disabled
   launcher fixtures issue no process. Reuse Unix bounded reaper/null streams and
   Windows native dispatch, preserving responsive control and EOF lifecycle.
4. Native PR list/detail expose the action with full current selection fencing;
   unavailable integration/errors are clear and recoverable. Browser/Pierre and
   terminal clipboard remain separate explicit actions. Navigation/late results
   cannot overwrite another PR. Native keyboard/paste tests and wide/narrow
   Storybook frames cover success/unavailable/late read.
5. Owned core/HTTP/real stdio/client fixtures verify both providers, same names/
   numbers, unsafe input, UUID aliases, opener acknowledgment, no unintended
   network/credential/provider writes and control responsiveness. Full root/UI/
   Rust/protocol/strict Clippy/fmt/no-GUI/Archgate/docflow/frames/ReactDoctor gates.
6. Verify/merge #371 first, rebase onto actual squash baseline and audit number
   collisions before next PR. Exact-head CI and verified squash merge required;
   ADR0020 stays Accepted and #306/epic remain open.

## Dependencies

- PR #371 fixed head `78ca5d07949fe12303b618eca2affbe849787545`, CI38197325446
  verified merged at `3322ff050c8d7c6bc115c01c9341cb0594cb4730`,
  2026-10-12 03:00:52Z after exact-head UI/verify success. Rebase required before PR.
- Native PR browser, explicit repository identity and bounded native URL opener.

## Status

- Claimed by: default-agent, 2026-10-12, single agent.
- Branch: `issue-306-provider-external-browser`.
- Location: isolated `/private/tmp/norn-provider-external-browser`.
- Blockers: verified predecessor merge, implementation/gates and exact next PR CI.
- Stopped:
