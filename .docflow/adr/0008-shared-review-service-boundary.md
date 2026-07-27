---
adr: 0008
title: Run shared reviews as an opt-in tenant-isolated service
status: Accepted
date: 2026-07-27
owner: default-agent
supersedes:
superseded-by:
depends-on: [0002, 0003, 0005, 0007]
tags: [service, trust, multi-tenant, open-core, review]
---

# ADR 0008 - Run shared reviews as an opt-in tenant-isolated service

## Context

Lachesi's trust model is local-first: developers can inspect the review engine,
keep provider credentials outside the webview, and run desktop, terminal, or
headless reviews without a Lachesi-operated service. Teams also need automated
reviews triggered by provider events, shared policy, durable publication, and
organization-level controls.

A shared service introduces a materially different trust boundary. It may
temporarily process source, prompts, evidence, and findings for more than one
organization. It must not silently turn the local product into a thin client,
receive uncommitted working-tree content, or reuse credentials and storage
across tenants.

The shared service is therefore an optional orchestration layer around the
public local review engine. Local desktop, TUI, and headless clients continue to
invoke that engine directly. A remote review is requested explicitly or begins
from an authenticated provider webhook for a provider-hosted pull request.

### Component boundary and data flow

1. A provider ingress adapter verifies a webhook signature, rejects unsupported
   events, converts the payload to the public provider-neutral event contract,
   and acknowledges delivery only after durable enqueue.
2. A job coordinator deduplicates by tenant, provider, delivery id, repository,
   pull request, and head SHA. It resolves immutable policy references and a
   credential handle, never raw credentials.
3. An isolated worker receives one tenant-scoped job, checks out only the
   authorized base and head commits into an ephemeral workspace, and runs the
   same public review engine used by local clients.
4. The worker writes a structured review result through a tenant-scoped storage
   port and destroys its checkout, prompt payload, and transient model input.
5. A provider publisher reads only a successful, current-head structured result
   and publishes idempotently. Publication retry never reruns the model.

The ingress adapter owns webhook verification and normalization. The
coordinator owns durable job state, idempotency, and retry decisions. A
credential broker owns secret resolution and rotation. Workers own transient
execution only. Tenant-scoped stores own durable results and operational
records. Provider publishers alone own remote comment and status mutation.

### Tenant and repository isolation

- Every event, job, credential handle, result, log, and publication key includes
  a non-empty tenant id. Repository or pull-request ids are never sufficient
  storage keys.
- A provider installation or workspace is assigned to exactly one tenant in a
  deployment. Repository access is derived from that assignment for each job.
- Workspaces, caches, queues, database authorization, and envelope-encryption
  data keys are tenant-scoped. Mutable source caches are not shared between
  tenants.
- Workers run in a per-job sandbox with bounded resources and egress limited to
  the selected source provider, model provider, and explicitly configured policy
  source.
- Workers receive short-lived provider access and model credential material
  just in time. Long-lived credentials never enter job payloads, result storage,
  or logs.
- Cross-tenant access is denied before repository lookup and is covered by
  conformance tests at every public storage and orchestration boundary.

### Data ownership and retention

| Data class | Owner | Storage and visibility | Default retention |
|---|---|---|---|
| Raw webhook body and signature | Tenant/provider installation | Ingress memory only; never logged or stored after normalization | Discard immediately after durable enqueue or rejection |
| Normalized event and job metadata | Tenant | Encrypted tenant-scoped queue/store; identifiers and state only | 90 days after terminal job state |
| Source checkout, diff, prompt payload, and model input | Repository owner | Per-job encrypted ephemeral workspace; worker only | Delete at job end; recovery sweeper removes remnants within 24 hours |
| Provider and model credentials | Tenant | Credential broker or external secret manager; workers receive short-lived material | Until rotation, revocation, tenant deletion, or repository disconnect |
| Repository policy and prompt | Repository owner | Read from the reviewed commit; durable records keep commit/version and digest, not an extra raw copy | Transient copy follows the job workspace; reference follows the review result |
| Organization policy | Tenant | Versioned tenant policy store; workers receive an immutable signed version | Until no retained review references the version, then tenant policy applies |
| Structured findings and normalized evidence | Tenant/repository owner | Encrypted tenant-scoped review store; authorized tenant roles only | 90 days in hosted deployments unless the tenant selects a shorter period |
| Operational logs and traces | Service operator, on behalf of tenant | Metadata and error codes only; no source, prompt, credential, raw evidence, or finding text | 30 days in hosted deployments |
| Administrative audit events | Tenant | Append-only tenant-scoped audit store with actor and action metadata | 365 days in hosted deployments |

Self-hosted deployments may select different retention periods, but may not
disable transient-workspace cleanup, secret separation, tenant keys, or
repository-deletion behavior. Repository disconnect or tenant deletion revokes
credentials immediately and schedules content-bearing data for deletion within
24 hours. A content-free audit tombstone may remain for the configured audit
period.

### Public and commercial extension boundary

The public core owns the provider-neutral event, job request, review result,
finding, evidence, publication, error, and idempotency contracts. It also owns
the local review engine and ports for ingress normalization, queueing,
credentials, storage, policy resolution, worker execution, and provider
publication. Base GitHub and Bitbucket contract adapters remain public where
they are needed to trust or extend the engine.

A managed or paid team service may implement the multi-tenant control plane,
hosted webhook endpoints, managed queues and databases, organization policy
distribution, KMS-backed secret operations, SSO and role administration,
dashboards, backups, deployment packaging, operational telemetry, SLOs, and
support. Commercial implementations attach through the public ports and do not
fork or replace the local review semantics.

### Failure behavior

- Service unavailability never disables local desktop, TUI, or headless review
  and never causes a local client to upload content automatically.
- Ingress returns a retryable failure when it cannot durably enqueue. After a
  successful acknowledgement, at-least-once delivery is absorbed by idempotency
  keys instead of creating duplicate reviews or comments.
- Transient worker failures use bounded durable retry. Exhausted jobs enter a
  dead-letter state visible to operators and publish nothing.
- A result whose head SHA is no longer current is marked superseded and is not
  published. A new provider event may create an incremental review job.
- Publication failure retries the publisher against the stored result and does
  not rerun analyzers or the model.
- Authentication, authorization, policy, or isolation failures are terminal and
  fail closed. They do not fall back to another tenant, credential, service, or
  model provider.

## Capability statement

Lachesi will support an optional shared review service that reuses the public
local review engine through provider-neutral contracts while isolating every
tenant, repository, credential, job, and retained artifact; local review
surfaces remain complete and independent when the service is absent.

## User stories / scenarios

- As a developer, I can keep using local Lachesi when the shared service is
  unavailable or prohibited for a repository.
- As an organization administrator, I can authorize automated review without
  exposing long-lived credentials or one tenant's data to another tenant.
- As a service operator, I can retry ingestion, execution, and publication
  independently without duplicating reviews or comments.
- As an integrator, I can implement compatible ingress, storage, credential, or
  publication adapters against public contracts.
- As a repository owner, I can understand where source, prompts, findings, and
  logs are stored and when they are deleted.

## Acceptance criteria

1. Local desktop, TUI, and headless review execute without a network dependency
   on the shared service and never opt into remote execution implicitly.
2. The service accepts only authenticated provider events or explicit remote
   requests for provider-hosted commits; no contract accepts an uncommitted
   local working tree.
3. Public contracts separate webhook parsing, durable queueing, credential
   resolution, review execution, result storage, and provider publication.
4. Every durable and executable service boundary requires tenant, repository,
   delivery, pull-request, and head-SHA identity where applicable.
5. Automated conformance tests prove that one tenant cannot read or mutate
   another tenant's credentials, jobs, policies, findings, logs, or publication
   state.
6. Credential implementations provide short-lived worker access, rotation,
   revocation, and repository/tenant deletion without serializing secrets into
   events, jobs, findings, or logs.
7. Retention and deletion tests cover every data class in this decision,
   including the 24-hour maximum for transient remnants and disconnected
   repository content.
8. Review jobs are idempotent for a delivery id and head SHA, stale-head results
   are not published, and publication retry does not rerun review execution.
9. Queue, worker, and publisher failures expose stable retryable or terminal
   states; exhausted work is dead-lettered and never partially published.
10. Public provider-neutral and review-engine contracts are sufficient for an
    independent compatible implementation without access to managed-service
    source.
11. Managed-service code uses public extension ports for orchestration and does
    not make local review behavior depend on paid components.
12. Hosted and self-hosted deployments expose their effective retention,
    isolation, credential, and failure policies to administrators.

## Out of scope

- Implementing an HTTP server, queue, worker deployment, or provider webhook.
- Selecting a cloud, queue, database, secret manager, or model vendor.
- Defining billing, pricing, sales packaging, or contractual service levels.
- Implementing SSO, organization dashboards, backup tooling, or retention UI.
- Sending local working-tree content to a shared service.

## Open questions

- None.

## References

- ../../docs/strategy/open-core-boundary.md
- ../../docs/specs/0003-repository-config.md
- ../../docs/specs/0006-cli-headless-review.md
- ./0002-http-in-rust.md
- ./0003-credentials-keychain.md
- ./0005-agentic-policy-pack-prototype.md
- ./0007-headless-review-cli.md
- https://github.com/lachesi-hq/lachesi/issues/89
- https://github.com/lachesi-hq/lachesi/issues/90
- https://github.com/lachesi-hq/lachesi/issues/95
- https://github.com/lachesi-hq/lachesi/issues/96
- https://github.com/lachesi-hq/lachesi/issues/108
- https://github.com/lachesi-hq/lachesi/issues/112
- https://github.com/lachesi-hq/lachesi/issues/113

## Revision History

| Date | Revision | Author | Change |
|------|----------|--------|--------|
| 2026-07-27 | r1 | default-agent | Accepted the opt-in shared-service trust boundary and operating invariants. |

## Approvals

| Role | Name | Date | Signature |
|------|------|------|-----------|
| Maintainer | fdg | 2026-07-27 | approved autonomous issue implementation and progression in chat |
