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
   foreign or unproven threads, or guessing provider from configuration.
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
