# Provider-qualified native review storage and session identity

## Owning ADRs

- `../../adr/0020-opentui-rust-stdio-workspace.md` (AC 3–5, 8–10).
- `../../adr/0007-headless-review-cli.md`.
- `../../adr/0003-credentials-keychain.md`.

## Scope

Bounded foundation for retained provider review workflows under #306 after
verified PR #368. Define provider-qualified, unambiguous native persisted review
and runtime session identities. Preserve legacy rows and CLI/Tauri contracts;
read legacy data only when full saved-run provenance proves its provider and
repository. Do not copy old runs into a second mutable owner or attribute unknown
legacy conversations. New native scoped storage must coexist with existing
publication updates and saved-run discovery. Exact cached-evidence review start,
profile execution, approval and external navigation follow in subsequent work.
The fresh preview review action remains unavailable until that integration ships.

## Exit Criteria

1. Native keys include provider, length-framed workspace/repository and PR ID;
   identical GitHub/Bitbucket names and IDs cannot share a row or session key.
   Local SHA-scoped and legacy key formats remain compatible.
2. Owned native rows are validated against full run provenance. Read projections
   combine native and proven legacy runs without duplicating ownership, exposing
   foreign or unproven threads, or guessing provider from configuration. Fence the old inline agent handoff:
   native-owned or ambiguous context cannot silently resume a legacy session.
3. Saved review discovery/readers and staged finding/publication reflection can
   resolve the actual owning row. Publication updates remain transactional and
   cannot overwrite a foreign provider row. Legacy data is not rewritten merely
   by reading; unknown conversations remain preserved for #361 reconciliation.
4. Owned SQLite/core tests cover provider/name/ID collisions, malformed native
   rows, mixed legacy provenance, ambiguous thread/run identities, coexistence,
   and narrow publication updates. No actual provider credentials or mutations.
5. Root typecheck/tests/lint/protocol/Archgate, Rust core/stdio/CLI/fmt/strict
   Clippy/no-GUI and applicable native UI tests pass; docflow collision audit,
   exact-head CI and verified squash merge establish completion.

## Dependencies

- PR #368 merged at `1f9b3d3e0d491c2321817e21bd6f1944194665df`.
- Existing full saved-run provenance checks, SQLite transactional publication.
- #361 retains ownership of global/unknown legacy history reconciliation.

## Status

- Claimed by: default-agent, 2026-10-12, `issue-306-provider-review-scope`.
- Blockers: implementation, local gate, exact-head CI and verified merge.
- Stopped:

## Prepared completion receipt

Prepared 2026-10-12 for verified PR integration. This path becomes a completion
record only after exact-head CI success and verified squash merge into main.

- Feature commit: `48cafd4e21e144dc6cbad1f049fc717866d560fb`.
- Verified legacy-cleanup follow-up: `7fe8a53dc9e49f08fb3e18fdbc5d405aa493c418`; native rows survive matching,
  nonmatching and empty legacy keep lists. Full Rust/Clippy gate rerun.
- Verified legacy inline handoff follow-up: `2140d09a6b73278f3ab06bb6cb24c86d6c5bd3f6`; four owned
  real-engine cases stop before fake agent spawn and preserve legacy bytes.
- Local gate: root typecheck/123 tests plus tooling/lint/protocol; UI typecheck,
  172 tests/1057 assertions/115 unchanged frames; Rust 705 core plus two existing
  ignored/55 stdio/six CLI/five onboarding/three bin tests; strict all-feature/
  all-target Clippy, fmt, no-GUI, Archgate 12/12 and docflow audit.
- Owned two-provider storage/session/publication fixtures and real stdio restart
  pass. Final targeted publication/ownership tests rerun after SQL scope change.
- No React changes, Norn review, live credentials/provider writes or private
  profile data. Existing coverage/Archgate advisories unchanged.
- Exact snapshot review-start, profile and native reply/fix activation remain
  subsequent integration; this foundation does not complete retained #306 parity
  or advance ADR 0020 / epic #294 to completed.
