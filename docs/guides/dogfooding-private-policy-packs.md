# Dogfooding private and external policy packs

This guide shows how to exercise external or private policy packs against real
repositories locally without committing proprietary context, client names,
internal paths, or secrets.

The goal is to make private policy content useful during local review while the
reviewed repository keeps a public, sanitized configuration.

## Safety rules

Apply these before adding any pack:

- Never commit customer names, private repository paths, internal incident
  details, credentials, model transcripts, or unpublished architecture.
- Keep private pack files outside the reviewed repository, or inside a
  directory that the repository explicitly ignores.
- Wire machine-local paths through `.norn.local.yaml`, and make sure that file
  is ignored before it exists.
- Sanitize every rule, profile, and example that reaches the repository.
- Review the working tree for accidental secrets before committing:
  `git status --short` and a search for known client or token strings.

Norn validates the local override and warns when the file is not ignored, but
the check is a safety net, not a substitute for reviewing what you stage.

## Local pack paths

`policy.packs` accepts a relative path inside the repository or an absolute
path that your machine controls:

```yaml
version: 0.1

policy:
  packs:
    - ./policies/team-rules
    # or an absolute, machine-local path that never ships with the repo
    - /Users/me/norn-policies/client-acme
```

A pack is a directory with a `pack.yaml` manifest. The public prototype at
`examples/policy-packs/agentic-code` shows the supported shape: `policy.rules`,
`policy.pathRules`, `policy.astRules`, `profiles`, `review.prompt.extend`,
`review.findings`, and `analyzers` defaults.

## Machine-local wiring

`.norn.local.yaml` is the canonical per-machine override. It is merged as a
local layer, so it can point at private packs, disable a slow analyzer for one
machine, or narrow paths without rewriting the committed config:

```yaml
# .norn.local.yaml - never commit this file
version: 0.1

review:
  profile: agentic-strict

policy:
  packs:
    - /Users/me/norn-policies/client-acme
```

Add the ignore entry before creating the file:

```gitignore
/.norn.local.yaml
```

`.lachesi.local.yaml` remains a read-only legacy fallback for repositories that
have not migrated; new configuration should use `.norn.local.yaml`.

## Scenario: reviewing Norn itself

Dogfood the public prototype on this repository to sanity-check the pack
format:

```yaml
# .norn.yaml
version: 0.1

review:
  profile: agentic-balanced

policy:
  packs:
    - ./examples/policy-packs/agentic-code
```

Then run a local review of the working tree and confirm the pack rules reach
the review payload. Keep machine-specific toggles in `.norn.local.yaml`.

## Scenario: a generic React SaaS repository

Start from the public example and layer a small team pack that holds only the
rules you are willing to publish:

```yaml
version: 0.1

policy:
  packs:
    - ./policies/react-saas
```

The pack can also define named profiles (for example a strict team mode) and
review prompt extensions; see the public prototype for the manifest shape.

```yaml
# ./policies/react-saas/pack.yaml
id: react-saas

policy:
  pathRules:
    - id: saas.data-access-boundary
      severity: medium
      paths:
        include:
          - "src/features/**"
      instruction: >
        Feature code must call typed data-access modules rather than the raw
        fetch or provider SDK.
      remediation: >
        Move the call behind the feature's data layer and cover the failure path.
```

This pack is public-safe because it describes patterns, not customers.

## Scenario: a work or client repository

Keep the pack in a private directory outside the repository and reference it
from `.norn.local.yaml`:

```yaml
# .norn.local.yaml - ignored, machine-local
version: 0.1

policy:
  packs:
    - /Users/me/norn-policies/client-acme
```

Guidance for client work:

- Store the pack in a private location or a separate private repository, never
  in the client's reviewed repository.
- Sanitize rule text so findings do not echo internal codenames, ticket ids, or
  private architecture. Rules should describe a pattern and a remediation.
- Do not paste incident write-ups, incident channels, or customer data into
  rules, rationale, or examples.
- Treat `expiresAt` suppressions and analyzer commands as reviewable content
  too: an analyzer command can leak a private hostname or path.
- When the client repository itself needs a pack, keep only the public-safe
  portion there and hold the rest in the private override.

## Verifying a pack without a model

Validate configuration and packs before running a review:

```sh
norn config validate --repo-path .
```

`config validate` loads the effective repository configuration, including
`policy.packs`, and reports unknown or invalid fields without contacting a
provider. Add `--format json` for machine-readable output; it exits non-zero
when the configuration is invalid.

Use `norn doctor --repo-path .` when you also want a read-only readiness report
for the machine and repository. Run either after changing `.norn.yaml` or
`.norn.local.yaml`, and before committing any pack wiring.

## References

- Policy engine behavior: [`docs/specs/0004-policy-engine.md`](../specs/0004-policy-engine.md)
- Repository configuration: [`docs/specs/0003-repository-config.md`](../specs/0003-repository-config.md)
- Loadable example pack: [`examples/policy-packs/agentic-code`](../../examples/policy-packs/agentic-code)
- Policy pack loading from local directories:
  [issue #44](https://github.com/delaudio/norn/issues/44)
