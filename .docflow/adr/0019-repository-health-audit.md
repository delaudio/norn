---
adr: 0019
title: Produce evidence-backed repository health audits from a stable snapshot
status: Accepted
date: 2026-10-04
owner: default-agent
supersedes:
superseded-by:
depends-on: [0007, 0017]
tags: [audit, repository, inventory, evidence, security, cli, tui]
---

# ADR 0019 - Produce evidence-backed repository health audits from a stable snapshot

## Context

Norn reviews pull-request diffs and unpublished local changes. Both are
change-scoped: a reviewer sees what a branch or working tree introduces, not the
whole repository. Technical debt, legacy code, architectural drift, and missing
verification are properties of the repository as a whole and are invisible to a
diff-scoped review. Feeding an entire repository to a model as one oversized diff
is unbounded, unreviewable, and not reproducible.

The useful product is a prioritized, evidence-backed local backlog: deterministic
inventory and analyzer evidence first, model judgment second, produced from a
stable snapshot without ever mutating the repository. Auditing a repository and
remediating it are separate permission boundaries; the audit must never edit
files as a side effect.

## Capability statement

Norn produces a bounded, deterministic, read-only health audit of a stable
repository snapshot and reports prioritized findings with provenance, separating
audit from any later remediation, and defaulting to tracked files at `HEAD` so
the result is reproducible.

## User stories / scenarios

- As a maintainer, I can audit a repository and get a prioritized backlog of
  technical-debt and legacy-code findings without opening a pull request.
- As a reviewer, I can see which deterministic evidence supports each finding
  and which model judgment added it.
- As a security reviewer, I can confirm an audit cannot modify the repository,
  cannot read sensitive paths, and cannot send excluded content to a model.
- As an automation author, I can run the audit from the CLI and get stable,
  machine-readable output for the same snapshot.
- As a terminal user, I can review the audit backlog and drill into a finding's
  evidence from the TUI.

## Acceptance criteria

1. An audit targets a stable snapshot that defaults to the tracked tree at
   `HEAD` and is identified by a deterministic snapshot fingerprint reported in
   the output.
2. A dirty working tree does not silently change the audited content; including
   local changes requires an explicit opt-in, and the output states which source
   was used.
3. Generated, vendored, binary, submodule, symlink, sensitive, and explicitly
   ignored paths are excluded from model evidence, and the exclusions are
   reported.
4. The audit is read-only with respect to the repository: it performs no write,
   no checkout, no stash, and no index mutation, and tests assert the repository
   state is unchanged after an audit.
5. Inventory and evidence collection are deterministic: repeating an audit
   against the same snapshot and configuration yields the same inventory
   fingerprint and the same deterministic evidence.
6. Resource use is bounded: file count, per-file size, total bytes, and
   subprocess duration are capped, and hitting a limit fails or truncates with
   an explicit warning rather than silently.
7. An audit is cancellable, and cancellation leaves no partial persisted result
   and no repository change.
8. Findings carry provenance: each finding identifies its category, severity,
   evidence references, and the snapshot it belongs to; deterministic evidence is
   distinguishable from model judgment.
9. Audit results can be persisted per repository snapshot with retained
   baselines so a later audit can report deltas, without colliding with pull
   request or local-review storage.
10. The terminal UI can present an audit backlog and open a finding's evidence
    without pretending the audit is a pull request or a local change review.
11. Audit findings never trigger repository edits; any remediation is a separate
    explicit action outside this capability.
12. Existing pull-request review, local review, headless CLI, and provider
    integrations remain backward compatible.

## Out of scope

- Automatically editing, formatting, or refactoring the audited repository.
- Repository-wide static analysis parity with dedicated tools (Semgrep, CodeQL,
  or a full cross-language AST runtime); analyzers remain opt-in evidence.
- Auditing content not present in the selected snapshot, such as untracked
  files (subject to the same explicit local-consent boundary as local review).
- A hosted or shared-service audit target; the initial capability is local.
- Bounded specialist-pass orchestration, which is a separate enhancement to the
  same pipeline.

## Open questions

- None.

## References

- [Run reviews through a headless local CLI](./0007-headless-review-cli.md)
- [Review local changes before publication](./0017-review-local-changes-before-publication.md)
- `../../docs/specs/0005-local-evidence-pipeline.md`
- `../../docs/specs/0004-policy-engine.md`
- GitHub epic and children: <https://github.com/delaudio/norn/issues/150>

## Revision History

| Date | Revision | Author | Change |
|------|----------|--------|--------|
| 2026-10-04 | r1 | default-agent | Accepted the repository health-audit snapshot and security contract. |

## Approvals

| Role | Name | Date | Signature |
|------|------|------|-----------|
| Maintainer | fdg | 2026-10-04 | approved in chat |
