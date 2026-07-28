# Spec 0007 - Signed organization policy

- Status: Implemented
- Date: 2026-07-28
- GitHub issue: #99

## Purpose

An organization may supply shared policy defaults and enforced constraints
without replacing repository-owned `.lachesi` configuration. Organization
policy resolution is explicit: repositories with no configured organization
source continue through the existing local config loader and do not perform a
network request.

## Source contract

An organization policy source receives a `tenantId` and `sourceId` and returns
one signed, versioned bundle. Source errors distinguish temporary
unavailability from a rejected request. A rejected request always fails
closed.

The version 1 bundle contains:

- `tenantId` and `sourceId`, which must exactly match the request
- a positive, monotonically increasing `version`
- `issuedAtMs` and `expiresAtMs`
- a JSON object named `defaults`
- a JSON object named `enforced`
- integrity metadata containing `algorithm: ed25519`, `keyId`, and a standard
  base64 signature

The signature covers the compact JSON serialization of the typed `bundle`
object, excluding the integrity envelope. Maps use deterministic key ordering.
The verifier accepts only a key explicitly trusted for the configured source.
Unsigned, expired, oversized, identity-mismatched, or incorrectly signed
bundles are rejected before any layer is applied. Credential-shaped fields are
also rejected before caching, using the same rule as repository configuration.
Signed layers apply an additional recursive check that rejects credential-like
extension keys, including prefixed token, password, secret, credential, and
username fields.
Versions are capped at `9007199254740991`, preserving exact JSON/TypeScript
integer representation in review metadata.

Signed layers cannot use `policy.packs` or `policy.sources`, because those
paths resolve inside the reviewed repository. Organization rules must be
embedded directly in the signed `defaults` or `enforced` layer.

## Deterministic precedence

The resolver applies layers from lowest to highest precedence:

1. built-in and already-resolved app defaults
2. signed organization `defaults`
3. repository-owned policy
4. non-committed local overrides
5. signed organization `enforced`

Object fields merge recursively. A scalar, array, or `null` in a later layer
replaces the earlier value. Organization layers cannot change the repository
config schema version. The final value must deserialize and validate as a
supported `RepoReviewConfig`. Repository policy packs and the selected profile
are expanded before the signed `enforced` layer is applied for the final time,
so repository-controlled indirection cannot override enforced settings.
When `enforced.review.profile` is present, that profile is selected for
expansion instead of any caller-supplied profile override. Resolution fails
closed if that enforced profile does not exist.

This ordering lets a repository specialize organization defaults, lets a
developer apply local non-secret configuration, and guarantees that mandatory
organization constraints remain authoritative.

## Availability and offline behavior

Unavailable-source behavior is configured explicitly per source:

- `FailClosed` stops resolution.
- `UseVerifiedCache { maxStalenessMs }` permits the last verified bundle only
  while it is unexpired and younger than the configured bound.
- `ContinueWithoutOrganizationPolicy` is valid only for an optional source and
  continues with built-in, repository, and local layers.

No implicit fallback exists. A source that returns a bundle which then fails
validation or signature verification never falls back, regardless of whether
the source is mandatory or optional.

The SQLite cache is tenant- and source-scoped. It rejects a lower version and
also rejects different signed content reusing the same version. Cached
envelopes are signature-verified and matched to their stored digest on every
offline use. A live bundle is fetched and verified before cached content is
parsed, so a damaged cache cannot block recovery from an available source.
Unverifiable cached rows are removed before a verified live bundle is stored;
only a signature- and digest-valid cached version participates in rollback
protection.

## Local runtime configuration

Desktop, TUI, and headless review opt in through
`LACHESI_ORGANIZATION_POLICY_CONFIG`, which points to a local JSON file outside
the repository. This keeps organization trust roots and availability choices
under administrator control rather than allowing a reviewed commit to select
them.

The path must be absolute. Lachesi resolves both the repository and config
paths before reading the file and rejects either a direct path or a symlink
whose target is inside the reviewed repository.

```json
{
  "tenantId": "tenant-acme",
  "sourceId": "engineering",
  "requirement": "mandatory",
  "unavailableBehavior": {
    "kind": "useVerifiedCache",
    "maxStalenessMs": 86400000
  },
  "trustedKeys": {
    "root-2026": "<base64-ed25519-public-key>"
  },
  "bundlePath": "organization-policy.bundle.json"
}
```

`bundlePath` is resolved relative to the configuration file unless it is
absolute. Its resolved target must also remain outside the reviewed
repository. The signed file is one implementation of the public source port;
a managed or self-hosted adapter may fetch the same envelope centrally. If
the environment variable is absent, review uses the existing local path
without reading an organization source.

Repository-config validation used by desktop and TUI prompt construction
returns the same organization-aware config used by execution. The shared
pipeline verifies again at execution time, adds resolved rules and paths to the
model prompt, selects requested analyzers from the resolved config, and stores
the selected profile and source versions on the review run.
Normal desktop and TUI review continues to skip analyzer execution because
evidence is expected upstream. An explicitly enabled and `required` analyzer
in resolved organization policy is the exception and must run successfully.
The resolved-policy appendix contains runtime constraints but omits
`review.prompt`, whose replacement or extension text is already materialized
in the review prompt. Execution-time resolution checks the submitted payload;
if the resolved prompt text is absent, a separate authoritative instruction
section supplies it without duplicating text already present.

`.lachesi.local.yaml` is a non-committed override for configured organization
resolution. When no organization source is configured, Lachesi retains the
pre-existing repository loader unchanged and does not load that file.

Desktop and TUI review fail closed when organization policy is configured but
the selected repository has no local clone, because repository and local
layers cannot be resolved safely without that path.

## Traceability

Each resolved source contributes the following review metadata:

- tenant and source identifiers
- bundle version and SHA-256 digest
- signing key identifier
- whether the verified cache supplied the bundle

The same source and version are recorded through the administrative
`policy_resolved` audit action. Audit storage keeps its existing opt-in
collection and redaction behavior.

## Security boundaries

- Signing public keys may be stored in local configuration; private signing
  keys are never accepted by this contract.
- Provider and model credentials remain outside policy bundles.
- The organization policy source is a public port. Hosted and self-hosted
  implementations may provide it without changing the local review engine.
- Organization policy resolution does not enable remote review execution or
  upload a local working tree.
