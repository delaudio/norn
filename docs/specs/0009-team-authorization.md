# Spec 0009 - Team identity and authorization

- Status: Implemented
- Date: 2026-07-28
- GitHub issue: #102

## Purpose

Lachesi exposes a provider-neutral identity and authorization boundary for
optional team-service operations. Local desktop, terminal, and headless review
remain independent from any identity provider.

The versioned request schema is `v1`. It identifies an organization, optional
team, optional repository, and requested operation. It never carries actor
roles or memberships, audit idempotency data, login tokens, provider
credentials, prompts, diffs, or finding text.

The authorization function receives a separate `TeamActor` principal produced
by a trusted server-side authentication adapter. `TeamActor` is not
deserializable from the request contract, its fields are private, and its
constructor validates authenticated claim shape. API and IPC adapters must
resolve this principal before calling authorization; they must never construct
it from role or membership values supplied in an operation request.

The same trusted adapter supplies a separate
`TeamAuthorizationAuditContext`. Its non-deserializable, private fields carry a
tenant-unique attempt id, bounded timestamp, and bounded correlation id. A
caller cannot select an audit delivery key through the operation request.

## Roles and permissions

Every operation maps to one explicit permission:

| Operation | Permission |
|---|---|
| Administer policy | `manage_policy` |
| Enroll repository | `enroll_repository` |
| Trigger review | `trigger_review` |
| Record finding feedback | `record_finding_feedback` |
| Publish review | `publish_review` |
| Read metrics | `read_metrics` |
| Export audit | `export_audit` |

The initial role matrix is:

| Role | Permissions |
|---|---|
| `admin` | all known permissions |
| `member` | trigger review, record feedback, publish review, read metrics |
| `viewer` | read metrics |
| `service_account` | trigger review, publish review |

Unknown roles, operations, and permissions are preserved as unknown enum
variants and always deny. Deserialization never grants a new role implicitly.

## Scope

The actor and target organization must match. Repository operations also
require a team and repository scope whose organization and team agree. A
non-admin actor must belong to the selected team. Policy administration and
audit export are organization-scoped; metrics may be organization- or
repository-scoped.

Scope checks run before the role matrix. Matching repository names in another
organization or team never grant access.
Organization-only operations reject repository or team fields instead of
silently changing scope. Metrics accept exactly organization scope or a paired
team and repository scope; team-only scope is invalid. Audit timestamps and
derived delivery identifiers use the same bounds as administrative audit
events.

## Denial audit

Every valid denied request is converted to an administrative audit event with
the `authorization_denied` action and `denied` outcome before it reaches the
configured audit sink. Organization-only denials omit repository scope.
Authorization denials use administrative audit schema `v2`, which allows
organization scope only for `authorization_denied` events while preserving the
required repository field and stable wire shape of existing `v1` events. A
mismatched repository or team is also omitted from the denial audit rather
than crossing the tenant boundary.
The audit event serializer and deserializer both enforce these version rules:
`v1` cannot omit repository scope or carry an authorization denial, and
repository-less `v2` is valid only for a denied authorization request.

Only bounded identifiers, operation, permission, denial reason, and correlation
metadata are eligible for storage. The existing audit redaction boundary runs
before the sink receives an event. If audit preparation or persistence fails,
authorization fails closed and does not expose the sink error.
The audit tenant always comes from the authenticated principal, never from the
operation request. Repository metadata is included only when organization,
team, and repository all match that trusted tenant.

## Local mode

`TeamActor::local_single_user()`, `TeamOrganization::local()`, and the local
team/repository helpers create a stable local admin identity. This identity has
no issuer, session, token, or group claim and therefore does not require OIDC,
SAML, or another login provider.
