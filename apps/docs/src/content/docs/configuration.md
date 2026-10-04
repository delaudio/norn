---
title: Configuration
description: App settings and repository-owned review configuration.
---

## App Settings

The desktop app settings include:

- review provider;
- tracked repositories;
- local clone paths;
- provider credentials;
- default diff view;
- AI provider, model, and effort;
- preferred terminal for review and fix flows;
- Jira and Notion context integration tokens;
- automatic sync interval;
- menu bar sync and notifications.

## Repository Config

Norn can read `.norn.yaml` from a configured local repository.

```yaml
version: "0.1"
review:
  profile: frontend-strict
  mode: balanced
  prompt:
    replace: |
      Full repository-owned review prompt.
  findings:
    minSeverity: low
profiles:
  frontend-strict:
    mode: strict
    minSeverity: medium
    policyPacks:
      - ./norn-policies/react-saas
    analyzers:
      tsc: required
paths:
  include:
    - "src/**"
  exclude:
    - "dist/**"
policy:
  packs:
    - ./norn-policies/agentic-code
  rules:
    - id: no-cross-module-imports
      severity: medium
      instruction: "Flag imports that cross module ownership boundaries."
publish:
  defaultMode: inline
  requireManualSubmit: true
```

Policy packs can contribute prompt extensions, rules, path rules, profiles, and analyzer defaults from local directories. Profiles can be selected from the desktop AI review panel per run. Repo config and policy packs should not contain credentials, tokens, private URLs, or other secrets.

If a repository does not have `.norn.yaml`, Norn also reads a
`.norn/` folder. `system-prompt.md`, `review-prompt.md`, `review.md`, or
`prompt.md` replace the built-in review prompt, and `packs/*/pack.yaml` entries
are loaded as local policy packs. When both roots exist, Norn rejects the
configuration and asks the maintainer to keep exactly one.

During the compatibility window, `.lachesi.yaml`, `.lachesi/`, and
`.lachesi.local.yaml` remain fallback-only inputs. Norn rejects mixed old/new
repository roots instead of merging them. Preview a safe migration with
`norn config migrate --dry-run`; omit `--dry-run` to execute it without
overwriting an existing canonical target.

The repository includes loadable example packs under `examples/policy-packs/`. Use them as local-path examples for review rules, profiles, analyzer defaults, and structured output samples:

- `agentic-code` - agent-authored change rules with `agentic-fast`, `agentic-balanced`, and `agentic-strict` profiles.
- `typescript-basic` - type-safety prompt rules, a public API path rule, and a catch-clause AST rule declaration.
- `react-basic` - effect lifecycle, list-key, state-ownership, and hook rules with a component path rule.
- `bitbucket-tauri-basic` - provider HTTP, secret, IPC command/mock parity, and native error mapping rules.

Each pack includes a `README.md` explaining how to install and adapt it.

Validate repo config locally before running a review:

```sh
norn config validate --repo-path . --format json
```

The command exits with code `2` when the config is invalid.
