---
adr: 0014
title: Deliver Norn through reproducible releases and verified upgrade channels
status: Accepted
date: 2026-08-31
owner: codex
supersedes:
superseded-by:
depends-on: [0011, 0012]
tags: [release, distribution, homebrew, cli, tui, desktop]
---

# ADR 0014 - Deliver Norn through reproducible releases and verified upgrade channels

## Context

Norn has initial tag, archive, Homebrew formula, lifecycle, and desktop cask
scaffolding, but those pieces do not yet form one fail-closed delivery chain.
Release builds can run with an ambient Rust toolchain and unlocked dependencies;
formula and cask metadata can omit exact checksums; lifecycle checks can skip
missing artifacts; and the public tap is not advanced by the verified release
run. Source installation also links commands back into checkout build output,
which can disappear or become stale.

The naming and onboarding decisions deliberately leave signing, publishing, and
package-manager plumbing to a separate delivery decision. That gap must close
before released CLI, TUI, or desktop binaries can be treated as the canonical
installed Norn runtime.

## Capability statement

Norn will deliver supported binaries through a fail-closed release chain that
builds immutable artifacts from locked inputs, binds package metadata to exact
checksums, verifies clean installation and upgrade before advancing a public
channel, and exposes desktop packages only when their signing and notarization
contract is satisfied.

## User stories / scenarios

- As a user, I can install or upgrade Norn through Homebrew without a source
  checkout or development toolchain.
- As a maintainer, I can trace every published binary and package checksum to an
  exact tag, commit, toolchain, target, and workflow run.
- As an existing user, I can upgrade from the previous stable release without
  losing local settings, review data, or compatible credentials.
- As a contributor, I can install a durable local build without depending on a
  checkout-relative `target/release` symlink.

## Acceptance criteria

1. A stable tag publishes nothing unless version alignment, formatting, lint,
   complete tests, architecture policy, and locked release builds all pass.
2. Every supported CLI/TUI archive has an immutable URL, exact SHA-256 checksum,
   and metadata naming its version, commit, target, toolchain, and workflow run.
3. Homebrew formula metadata is rendered from artifacts produced by the same
   release run, contains exact per-architecture checksums, and advances the
   public tap only after required smoke tests succeed.
4. Clean supported macOS hosts can install the CLI and TUI, resolve their
   canonical command names, and run bounded version, help, and readiness checks
   without Node.js, pnpm, Rust, or a source checkout.
5. CI upgrades representative state from the previous stable formula to the
   candidate formula, verifies compatibility and data preservation, and treats
   upgrade, uninstall, or reinstall failures as release failures.
6. Source-based developer installation places durable executables in a
   configurable prefix and leaves the prior installation usable if replacement
   fails.
7. Desktop cask metadata is published only with matching checksummed, signed,
   notarized app artifacts that pass Gatekeeper, launch, upgrade, and uninstall
   checks without conflicting with formula binaries.
8. Missing, partial, mismatched, unsigned, or unverified artifacts leave the
   previous public package metadata unchanged and produce actionable sanitized
   diagnostics without credentials or private machine paths.

## Out of scope

- An application-managed self-update mechanism.
- Package managers other than Homebrew in the initial delivery slice.
- Expanding the supported operating-system matrix as part of release hardening.
- Changing review, provider, repository configuration, or publication semantics.

## Open questions

- None.

## References

- ./0011-norn-naming-and-compatibility.md
- ./0012-norn-onboarding-contract.md
- https://github.com/delaudio/norn/issues/199
- https://github.com/delaudio/norn/issues/200
- https://github.com/delaudio/norn/issues/201
- https://github.com/delaudio/norn/issues/202
- https://github.com/delaudio/norn/issues/203
- https://github.com/delaudio/norn/issues/204

## Revision History

| Date | Revision | Author | Change |
|------|----------|--------|--------|
| 2026-08-31 | r1 | codex | Accepted the reproducible release and verified upgrade delivery contract after guided assessment. |

## Approvals

| Role | Name | Date | Signature |
|------|------|------|-----------|
| Maintainer | fdg | 2026-08-31 | approved in chat |
