---
title: Review Workflow
description: How review runs, findings, and publication fit together.
---

Norn separates review generation from publication.

## Review Loop

1. Select a pull request.
2. Inspect metadata, branch status, comments, files, and image previews.
3. Run an AI review with Claude or Codex.
4. Read the generated review thread and structured findings.
5. Convert useful findings into local draft comments.
6. Publish only the comments you explicitly approve.

## Local Clone Workflows

When a repository has a configured local clone, Norn can support:

- branch checkout and sync;
- branch conflict resolution;
- AI fix execution;
- verification;
- commit;
- push.

## Publication Rule

AI output is never treated as automatically publishable remote feedback. The reviewer controls what is posted back to the provider.
