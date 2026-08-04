---
adr: 0011
title: Adopt Norn names with a bounded Lachesi compatibility window
status: Accepted
date: 2026-08-04
owner: codex
supersedes:
superseded-by:
depends-on: [0003, 0006, 0007]
tags: [norn, naming, compatibility, migration, distribution]
---

# ADR 0011 - Adopt Norn names with a bounded Lachesi compatibility window

## Context

The product, its CLI/TUI binaries, repository configuration, local data, and
public delivery surfaces currently use Lachesi identifiers. Distribution and
first-run onboarding must not introduce another set of temporary names or ask
users to migrate secrets manually. A mechanical rename without a compatibility
contract risks lost review history, unreachable credentials, broken scripts,
and ambiguous repository configuration.

The alternatives are to retain Lachesi permanently, rename every surface in a
single breaking release, or make Norn canonical while accepting old inputs for
a defined compatibility window. Permanent dual naming keeps documentation and
support ambiguous; a flag-day migration is unsafe for persisted local data and
repository configuration. A bounded compatibility window provides a clear new
surface while allowing safe, observable upgrades.

For Lachesi, this decision preserves the local-first credential boundary in
ADR 0003 and the reusable CLI/TUI surfaces in ADRs 0006 and 0007 while the
product moves to Norn.

## Capability statement

Norn is the canonical product identity. Fresh installations create only Norn
identifiers; supported Lachesi inputs remain readable through one documented
compatibility window, emit a deprecation notice where a user invokes them, and
are removed only after upgrade coverage and a published removal release.

## User stories / scenarios

- As a new user, I can install and run `norn` and configure a repository with
  `.norn.yaml` without learning legacy names.
- As an existing user, I can upgrade without losing local settings, review
  history, or access to credentials stored through the existing secure layer.
- As a repository maintainer, I can migrate configuration predictably and see
  an actionable conflict instead of an implicit old/new merge.

## Acceptance criteria

1. Fresh builds, packages, documentation, and user-visible runtime output use
   Norn as the current product name and `norn` as the canonical executable.
2. The migration matrix below assigns every externally consumed or persisted
   Lachesi identifier a keep, migrate, alias, or remove action.
3. Norn resolves compatible old configuration and environment inputs only when
   the canonical input is absent; coexistence that would change behaviour
   stops with actionable guidance.
4. Local data and credential migration is atomic or leaves the legacy source
   usable; no secret value is copied to repository files, output, or logs.
5. Compatibility aliases emit a deprecation notice and are removed only in the
   first major release after six stable releases and passing fresh-install and
   upgrade regression gates.

## Migration matrix

| Surface | Canonical Norn identity | Legacy action and precedence |
|---|---|---|
| Product, desktop app, UI, docs, website | Norn / `Norn.app` | Replace current-product copy; keep historical mentions only in migration notes. |
| Main executable | `norn` | `lachesi` is an alias during the window; it prints a deprecation notice to stderr. |
| Terminal UI executable | `norn-tui` | `lac` is an alias during the window; it prints a deprecation notice to stderr. |
| Cargo package/library/binaries | `norn`, `norn_lib`, `norn-tui` | Rename in the release that introduces the aliases. |
| npm workspace packages | `@norn/*` and root `norn` | Rename package metadata; no published legacy package is implied. |
| Tauri product/bundle/application ID | `Norn`, `app.norn.desktop` | Migrate settings and keychain service references before first canonical open. |
| Repository config | `.norn.yaml`, `.norn/`, `.norn.local.yaml` | Read `.lachesi.yaml`, `.lachesi/`, and `.lachesi.local.yaml` only when the equivalent Norn source is absent. Never merge old and new roots implicitly. |
| Environment variables | `NORN_*` | Read the equivalent `LACHESI_*` only when `NORN_*` is unset; canonical wins. |
| OS config/data directories and SQLite | `norn/`, `norn.sqlite3` | Atomically migrate from `lachesi/` and retain a recoverable legacy source until validation succeeds. |
| Keychain service/accounts | `app.norn.desktop` identifiers | Resolve legacy entries through the secure credential layer, then create canonical references without printing secrets. |
| JSON schemas, metrics, report namespaces | `norn.*.v1` | Readers accept documented `lachesi.*.v1` versions; writers emit only Norn versions. |
| GitHub repository and URLs | `lachesi-hq/norn` | Rename only after the runtime migration is stable; rely on GitHub redirects and retain a migration notice for non-redirectable integrations. The organization remains `lachesi-hq`. |
| Websites, documentation, design system, release/tap | `norn.dev`, `docs.norn.dev`, `design-system.norn.dev`, `lachesi-hq/tap/norn` | Migrate or redirect after ownership is provisioned; never claim a redirect until it is verified. |

## Out of scope

- Renaming the GitHub organization.
- Removing legacy aliases in the initial Norn release.
- Copying credentials, settings, or repository configuration through shell
  scripts or manual user instructions.
- Signing, notarizing, or publishing a release artifact; those are separate
  delivery slices.

## Open questions

- None. Domain ownership and DNS provisioning are delivery prerequisites, not
  naming decisions; their absence blocks public URL migration rather than the
  canonical contract.

## References

- ./0003-credentials-keychain.md
- ./0006-terminal-ui.md
- ./0007-headless-review-cli.md
- ../../.archgate/adrs/ARCH-001-tauri-react-rust-bitbucket-boundary.md
- ../../.archgate/adrs/ARCH-007-drive-repository-commands-through-platform-native-task-runners.md
- GitHub issue #173

## Revision History

| Date | Revision | Author | Change |
|------|----------|--------|--------|
| 2026-08-04 | r1 | codex | Accepted through the maintainer's autonomous implementation instruction. |

## Approvals

| Role | Name | Date | Signature |
|------|------|------|-----------|
| Maintainer | fdg | 2026-08-04 | approved through autonomous issue-completion instruction |
