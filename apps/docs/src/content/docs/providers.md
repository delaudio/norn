---
title: Providers
description: Configure Bitbucket Cloud and GitHub repositories.
---

Norn supports Bitbucket Cloud and GitHub as review providers.

## Bitbucket Cloud

Bitbucket repositories use:

- workspace;
- repository slug;
- Atlassian email;
- Bitbucket API token.

Bitbucket credentials are stored separately from GitHub credentials.

## GitHub

GitHub repositories use:

- owner or organization;
- repository name;
- GitHub token.

The GitHub token is stored separately in the local credentials layer.

## Provider-Aware Features

Provider support covers the main review workflow:

- pull request list;
- pull request detail;
- diff and diffstat;
- comments;
- approvals;
- branch status;
- image previews;
- local clone discovery;
- closed PR analytics.
