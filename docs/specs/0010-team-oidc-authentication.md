# Spec 0010 - Team OpenID Connect authentication

- Status: Implemented
- Date: 2026-07-28
- GitHub issue: #103

## Purpose

The optional team service can authenticate users against a generic OpenID
Connect provider and produce the trusted `TeamActor` principal consumed by the
team authorization boundary. Local desktop, terminal, and headless review do
not construct this adapter and remain independent from an identity provider.

The adapter uses OpenID Connect Discovery and Authorization Code flow with
PKCE S256. Provider discovery, token exchange, ID token verification, and
UserInfo use the standards-based `openidconnect` implementation. The outbound
HTTP client does not follow redirects and enforces fixed bounded timeouts:
five seconds to connect and 30 seconds for the full request, including
response reads.

## Configuration

Deployments supply public configuration separately from the client secret:

```text
issuer_url = "https://id.example.com"
client_id = "lachesi-team-service"
redirect_url = "https://reviews.example.com/auth/callback"
group_claim = "groups"
scopes = ["profile", "groups"]
session_ttl_ms = 3600000
max_pending_logins = 1000
max_sessions = 10000
```

The issuer must use HTTPS. The redirect URI must use HTTPS, except that HTTP is
allowed for an exact loopback host during local development. Session lifetime
is bounded from one minute through 24 hours. Pending login and session
capacities are positive and have a hard maximum.
Issuer URLs cannot contain a query or fragment; redirect URLs cannot contain a
fragment.

The client secret is supplied through `OidcClientSecret`, not through
`OidcPublicConfig`. It must come from the deployment secret store or
environment and must not be serialized into application configuration,
request data, review metadata, or logs.
The service requires a confidential client secret. It accepts
`client_secret_basic` by default and `client_secret_post` when explicitly
advertised by provider metadata; other token-endpoint client authentication
methods are rejected during discovery.

## Login flow

1. Discovery requires the returned issuer to match the configured issuer.
2. A login start creates fresh state, nonce, PKCE verifier, and opaque browser
   binding values. The attaching HTTP adapter stores the binding only in a
   `Secure`, `HttpOnly`, `SameSite=Lax` cookie.
3. The server stores SHA-256 digests of state and browser binding alongside the
   nonce and PKCE verifier for at most five minutes.
4. A callback verifies and atomically consumes both matching values before
   token exchange. Invalid, expired, replayed, or cross-browser callbacks fail
   closed; the adapter clears the binding cookie after callback handling.
5. Before each callback token exchange, discovery and JWKS are refreshed and
   the verified client is replaced atomically. This accepts normal signing-key
   rotation without a service restart.
6. Token exchange sends the one-time PKCE verifier. The ID token verifier
   validates issuer, client audience, signature, nonce, and expiry.
7. UserInfo is requested with the verified ID-token subject as the expected
   subject, preventing token substitution.
8. The configured group claim is parsed as a bounded array of bounded strings
   and mapped to a trusted `TeamActor`.

Authorization codes, access tokens, ID tokens, nonce values, state values, and
PKCE verifiers are transient. They are not returned in errors or stored in the
resulting session.

## Identity and role mapping

The authenticated principal ID is a stable SHA-256-derived identifier over the
configured issuer and stable OIDC subject. This avoids using email or another
mutable personal attribute as identity. The configured organization and team
identifiers must satisfy the team authorization identifier contract.

Every group-to-role mapping names an exact group value, one of `admin`,
`member`, or `viewer`, and zero or more team IDs. Service-account and unknown
roles cannot be configured for an OIDC user. Multiple matched groups may
contribute team IDs only when they all select the same role; conflicting roles
are rejected as ambiguous.

No matching group is rejected before a session is created. Missing group claims
are treated the same way. A malformed claim is rejected. Email domains and
other unconfigured claims never grant a role.

## Sessions and logout

The reference session store is server-side and process-local. It retains only
the trusted principal and absolute expiry, keyed by a SHA-256 digest of a fresh
opaque session handle. Expired entries are pruned before capacity checks and
cannot be resolved.

An attaching HTTP adapter reconstructs a handle from its cookie value through
the bounded `OidcSessionHandle::from_secret` boundary before resolving or
revoking it. Debug formatting redacts both session handles and login URLs.

Logout removes the exact session digest. Other sessions remain valid until
their own logout or expiry. Raw session handles have redacted debug output and
must be transported only in secure, HTTP-only cookies by an attaching service
adapter. Session expiry is calculated after successful UserInfo validation.

Managed or self-hosted deployments that require restart survival may replace
the reference store with a durable implementation, but must preserve opaque
handles, digest-at-rest storage, absolute expiry, bounded capacity, and
per-session revocation.
