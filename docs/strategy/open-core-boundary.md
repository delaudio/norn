# Open-Core Boundary and Commercial Extension Points

- Status: Draft
- Date: 2026-07-06
- GitHub issue: #42

## Purpose

Norn should remain credible as a local-first, open-source review workspace while leaving room for commercial policy packs, team workflows, and services. The core repository should make the review engine inspectable and adoptable; paid work should sit above that core as curated content, distribution, reporting, and operational help.

This document defines the working boundary so new features do not accidentally move commercial value into the wrong layer or weaken the open-source trust model.

## Boundary

| Area | Public / OSS | Private / paid | Rationale |
| --- | --- | --- | --- |
| Desktop review workspace | Tauri desktop app, PR list/detail, diff review, comments, approval, local review history | Team dashboards, shared workspace profiles, org-wide review analytics | The app demonstrates immediate value and builds trust. Team coordination can be commercial later. |
| Providers | Bitbucket and GitHub base review provider support | Enterprise integrations such as GitLab, Azure DevOps, SSO, on-prem packaging | Base provider support makes Norn useful. Enterprise distribution and administration are paid surfaces. |
| AI review | Basic Claude/Codex review execution, structured review output, local reviewer-controlled publication | Advanced commercial review profiles, custom agent workflows, support SLAs | The engine must be usable and auditable. Paid value comes from tuned workflows and support. |
| Findings model | `ReviewRun`, `ReviewFinding`, evidence, publication state, JSON/markdown contracts | Compliance-specific report templates and audit exports | The schema should become a public contract. Commercial reporting can package and operationalize it. |
| Repo config | `.norn.yaml`, `.norn.local.yaml`, config precedence, validation behavior | Hosted/team policy distribution and update channels | Repo config is the adoption hook and must stay open. Distribution can be paid. |
| Policy engine | Local ADR, markdown, YAML rule loading, path rules, suppressions, minimal evaluation | Curated vertical policy packs, private team bundles, policy registry | The loader and rule model are infrastructure. Curated policies are product content. |
| Evidence pipeline | Local analyzer invocation, normalized evidence, basic adapters for typecheck/lint/tests/security scanners | Team-required analyzer bundles, compliance presets, managed CI rollout | Evidence is a credibility feature. Packaged rollout and compliance mapping can be commercial. |
| CLI | Headless review, config validation, JSON/markdown output, CI exit codes | Hosted execution, organization-wide reporting, managed policy updates | The CLI enables adoption and standards. Hosted orchestration can be paid. |
| Documentation | User docs, architecture/spec docs, examples, public demo policy packs | Customer delivery templates, sales scripts, private playbooks | Public docs reduce trust friction. Service operations can remain private. |
| Website | Landing, docs, Storybook/design system, service positioning | Customer-specific materials, private case-study source notes | Public positioning should explain the open core. Private customer context stays private. |

## Extension Points

Commercial features should attach through explicit extension points instead of forking the core.

### Policy Packs

The public core should load external packs from local directories or future registries. A pack may contain:

- rules and path rules
- review profiles
- prompt extensions
- analyzer defaults
- examples and expected output
- documentation for adaptation

The complete curated packs can be private, while the pack format, loader, validator, and small example packs stay public.

Related issues:

- #44 Add policy pack loading from local directories
- #45 Add review profile support to config and review execution
- #48 Add public example policy packs

### Review Profiles

Profiles turn a raw rule set into named operating modes such as `agentic-fast`, `agentic-balanced`, or `frontend-strict`.

The core should own profile resolution and validation. Paid packs can provide opinionated profile definitions.

Related issue: #45.

### Config Validation and Policy Doctor

Validation should stay public because teams need deterministic checks before running local or CI reviews.

Commercial value can come from diagnostics that recommend paid packs or setup services, but the command should remain useful without paid content.

Related issues:

- #46 Implement `norn config validate`
- #49 Add `lachesi policy doctor`

### Reports and Artifacts

The core should export basic review reports using the public findings/evidence contract.

Paid layers may add:

- audit/compliance report formats
- multi-repo summaries
- stakeholder-ready HTML/PDF templates
- organization-wide trends

Related issue: #50.

### Setup Services

Setup services should package existing public primitives into a working review operating system for a team:

- `.norn.yaml`
- policy profile
- analyzer setup
- CI/headless review
- report template
- rollout plan

Private service repositories may hold discovery scripts, pricing notes, delivery checklists, and customer templates.

Related issue: #52.

## What Will Not Be Closed

These parts should remain public unless the strategy is intentionally revisited:

- the desktop app core
- the base Bitbucket and GitHub review workflows
- basic Claude/Codex AI review execution
- the findings and evidence schema
- `.norn.yaml` and `.norn.local.yaml` config contracts
- minimal policy engine and local policy source loading
- local evidence pipeline primitives
- headless CLI review and config validation foundations
- mock/browser development support needed for contributors
- architecture decisions and public specs needed to understand the system

The reason is practical: Norn asks developers to trust a local-first review tool with source code, credentials, and model-mediated review workflows. The base engine must stay inspectable.

## Initial Pricing Hypothesis

These are directional hypotheses, not published pricing.

| Offer | Buyer | Hypothesis |
| --- | --- | --- |
| Single curated policy pack | Individual developer or small team | EUR 99-299 one-time or annual updates |
| Team policy bundle | Startup or product team | EUR 499-999 per team/year |
| Agentic-code review pack | Teams using Codex, Claude Code, Cursor, or similar tools | EUR 199-499 for pack, higher with setup |
| Review Workflow Audit | CTO, tech lead, engineering manager | EUR 750 light setup; EUR 1,500-2,500 team setup; EUR 5,000+ custom rollout |
| Hosted policy registry | Teams with multiple repos | Subscription once pack loading and validation are proven |
| Team edition | Teams needing shared profiles, reports, audit trail | Subscription after the core workflow is validated |

The near-term commercial wedge should be policy packs plus setup service. Hosted registry, team dashboards, and enterprise integrations should wait until local packs and validation are useful on real repositories.

## Decision Rules

Use these rules when deciding where a feature belongs:

1. If the feature is required to trust or adopt the local engine, keep it public.
2. If the feature defines a contract others build on, keep it public.
3. If the feature is curated expertise, packaging, distribution, reporting, or service delivery, it can be private or paid.
4. If a paid feature needs a new runtime hook, build the hook publicly and keep the commercial content separate.
5. If a feature would make Norn useless without paid access, it belongs in the core or should be delayed.
6. If a feature contains customer-specific context, pricing, sales scripts, or proprietary rules, keep it out of the public repo.

## Public Roadmap Links

Relevant specs:

- [Repository config](../specs/0003-repository-config.md)
- [Policy engine](../specs/0004-policy-engine.md)
- [Local evidence pipeline](../specs/0005-local-evidence-pipeline.md)
- [CLI and headless review mode](../specs/0006-cli-headless-review.md)

Relevant issues:

- #43 Create agentic code review policy pack prototype
- #44 Add policy pack loading from local directories
- #45 Add review profile support to config and review execution
- #46 Implement `norn config validate`
- #48 Add public example policy packs
- #49 Add `lachesi policy doctor`
- #50 Add review report export
- #52 Add Review Workflow Audit service page
- #54 Add dogfooding guide for external/private policy packs
