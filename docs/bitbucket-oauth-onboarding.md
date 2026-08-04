# Bitbucket Cloud OAuth onboarding

The shared review service connects Bitbucket Cloud through OAuth rather than a
manually configured long-lived API token. Desktop credential storage is not
changed by this flow.

## Requested scopes

Norn requests only these Bitbucket Cloud scopes:

| Scope | Purpose |
| --- | --- |
| `account` | Identify the OAuth-authorized account. |
| `workspace` | Resolve the selected workspace. |
| `repository` | Read repository and committed pull-request content. |
| `pullrequest` | Read pull-request state and publish review findings. |

## Administrator flow

1. Start authorization with a browser-bound, single-use state value.
2. Complete the callback over HTTPS. Forged, expired, mismatched, or replayed
   state values are rejected before the authorization code is exchanged.
3. Select the repositories to enroll. The service validates, but does not
   create, the required webhook subscription for every selected repository.
4. Persist only tenant, workspace, repository, and authorization-status
   identifiers in the local service database. The OAuth refresh token is sent
   directly to the encrypted credential store boundary and is never returned or
   written to config, logs, review jobs, or enrollment storage.

Repository removal revokes local enrollment while retaining the identifier-only
record for auditing. A revoked authorization or removed workspace access marks
the enrollment inactive, stops review and publication, and requires an
administrator to authorize the workspace again.
