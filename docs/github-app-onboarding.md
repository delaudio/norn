# GitHub App onboarding

The shared review service connects a GitHub organization through a GitHub App
installation. It does not accept or persist personal access tokens.

## Required permissions

Configure the GitHub App with the following repository permissions only:

| Permission | Access | Purpose |
| --- | --- | --- |
| Contents | Read-only | Read the committed pull-request revision for review. |
| Metadata | Read-only | Identify an installed repository. |
| Pull requests | Read and write | Read pull-request metadata and publish review findings. |

Subscribe the App only to pull-request events needed by the service. Webhook
deliveries and publication are accepted only when the installation is active
and the repository is currently enrolled for the tenant.

## Administrator flow

1. Start installation with an opaque, URL-safe state value bound to the tenant.
2. Complete the GitHub installation and fetch the repositories available to it.
3. Explicitly select one or more repositories. Lachesi configures delivery
   only for those selected repositories.
4. Store the tenant id, installation id, organization login, and selected
   repository ids locally. Credentials remain with GitHub and are resolved only
   at execution time by the credential boundary.

Removing a repository revokes its local enrollment and disables future webhook
processing and publication for it. The installation remains connected and the
removed enrollment record is retained as an audit-safe tombstone. A suspended
or uninstalled GitHub App installation disables all enrolled repositories until
an administrator completes a new active installation.
