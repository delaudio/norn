# Native review profile selection

F6 opens the repository's declared review profiles when the backend advertises
`review-profile-catalog`. Choose **Repository default** to pass `null` to the
native review engine; choose a declared name to pass that exact name. Enter
selects a profile and closes the picker. It does not start a review. Escape
cancels, `r` refreshes and arrows/j/k/Page Up/Page Down navigate. Pasted text
cannot select a profile or execute commands. Older backends retain the manual
name composer.

The picker shows the effective native default, a bounded declared catalog and
generic policy warnings/errors. It never returns raw policy, prompts, analyzer
commands, credentials or local paths. Names are limited to 128 Unicode characters
and 512 UTF-8 bytes, without controls or surrounding spaces; the catalog permits
at most 100 names. Invalid catalogs fail without truncating into a different
selection. The selected long name is shown in full below the abbreviated list.

## Ownership and native semantics

`review.profiles` carries the complete provider/repository/PR/run target or local
snapshot identity. Provider lookup uses only its exact provider-qualified
configured checkout. Missing or ambiguous configured paths, non-root Git paths
and an origin for another repository fail; another provider with the same names
cannot inherit that checkout. A saved run must exist under its actual scoped
owner before policy lookup. A provider without a configured clone receives only
the native base/default option. Named or organization profiles still require a
valid local checkout at review start.

A local request first proves its canonical Git root, exactly one explicitly
configured provider/repository owner, matching origin and current native local
snapshot hash. Changed snapshots fail with a refresh instruction. This endpoint
does not establish the local workflow's complete branch/base/layer picker; that
remains separate retained parity work.

Catalog and inline review prompt resolution share the existing Rust policy
validation seam used by the desktop and native engine. Declared repository
profiles, implicit `default`, explicit configured defaults, signed organization
layers, enforcement and local overrides keep their actual native precedence.
Local overrides participate through the configured organization-policy resolver;
without that resolver, the existing repository loader remains authoritative.
Missing default names preserve its warning/base semantics. Invalid policies
fail before agent launch. Organization-policy rollback/signature/cache checks
are preserved, including existing native verified-cache maintenance. Catalog
navigation does not launch agents, send provider writes, rewrite policy or
modify the worktree/index. Policy file parsing keeps existing engine input
limits; only the new request/result catalog is bounded here.

Full-target and request-generation fences discard reads completed after refresh,
cancel, close, target/run/provider changes or reconnect. Target changes reset the
selection; refreshing a catalog removes a name that is no longer declared and
returns to the native default with a notice. Review start validates policy again.
The canonical repository/user prompt editor is preserved. Explicit policy-file
editing belongs to repository/editor integration; there is no invented profile
rewrite API.

## Verification

Owned native policy tests cover repository/default/named/local semantics, signed
layer precedence, invalid/missing defaults, strict target validation and bounds.
An owned real Git fixture proves local snapshots and rejects changed snapshots,
missing/ambiguous checkouts and cross-provider fallback. Real stdio tests verify
both providers, qualified saved owner, no-clone base, invalid policy and
continued control. The existing owned fake-agent review pipeline now fetches the
catalog before exercising the exact selected/default profile in real payloads.

Native input tests cover selection/start handoff, null default, removed names,
errors/refresh/cancel, late responses, malformed/oversized/foreign DTOs, paste,
older backend fallback and long Unicode names at 50 columns. Eight native frame
scenes cover catalog/base/error/loading at 120x30 and 50x18. Only owned fixtures
are used; there are no live provider writes or private profile reads.

This delivery does not complete the retained parity matrix, native desktop
profile recovery, installed platform checks or the final cutover.
