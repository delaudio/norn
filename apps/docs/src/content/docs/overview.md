---
title: Overview
description: What Norn is and how it fits into a review workflow.
---

Norn is an open-source, local-first review workspace for pull requests across Bitbucket Cloud and GitHub.

It keeps the code host as the source of truth, while moving high-context review work into a local desktop workspace. The app is built for reviewers who need to inspect diffs, understand branch state, run AI-assisted reviews, curate findings, and decide what is safe to publish.

## Product Surface

- Multi-repository pull request sidebar.
- Bitbucket Cloud and GitHub provider support.
- Unified and split diff review.
- Image previews for changed PNG, JPEG, SVG, GIF, and WebP files.
- Local draft comments and explicit publication.
- AI review runs with Claude or Codex.
- Local clone integration for branch sync, fix, commit, and push workflows.
- Closed PR analytics for author, repository, churn, lead time, risk, and review coverage.

## Local-First Model

Norn keeps credentials and review state local:

- provider tokens are stored in the local credentials layer;
- non-secret settings are stored in the OS config directory;
- review runs, jobs, logs, findings, and publication state are stored locally;
- AI output is draft material until the reviewer decides to publish.
