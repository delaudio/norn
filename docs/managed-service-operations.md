# Managed Service Operations

This document defines pilot operating targets for an optional managed Lachesi
team service. It is not a contractual SLA and does not change the independent
desktop, TUI, or headless review paths.

## Service boundary and locations

The initial pilot supports one selected deployment region per organization.
Available regions and any data-location commitments are shown before
enrollment; source-provider, AI-provider, and support tooling processing
locations are disclosed separately. A region move is a planned migration with
an approved backup and restore runbook.

Managed service operators own the control plane, service runtime, patching,
backups, incident coordination, and provider integration health. Customers own
repository access, provider installation approval, policy selection, and the
accuracy of organization membership. Self-hosted operators own every runtime,
backup, upgrade, monitoring, provider credential, and incident-response task;
Lachesi supplies the documented service artifacts only.

## Pilot objectives

| Objective | Target | Measurement source and reporting window | Dependency boundary |
|---|---|---|---|
| Control-plane availability | 99.5% monthly | Synthetic `/readyz` probes, excluding announced maintenance; monthly report | Lachesi service only |
| Webhook acceptance | 99% accepted or explicitly rejected within 60 seconds | Ingress receipt and durable-job timestamps; rolling 7 days | Git provider delivery is measured separately |
| Review start | 95% of eligible jobs claimed within 10 minutes | Durable job `created_at_ms` to `started_at_ms`; rolling 7 days | Queue/worker capacity; source and AI availability reported separately |
| Review completion | 95% of claimed jobs terminal within 30 minutes | Durable job lifecycle timestamps; rolling 7 days | AI-provider execution and Git-provider reads separately labelled |
| Backup | Daily verified backup, recovery point no older than 24 hours | Backup manifest checksum and scheduled-job result; daily report | Managed storage only; implementation is documented by [backup and restore](self-hosting.md#backup-and-restore) |
| Restore drill | Successful restore within 4 hours, quarterly | Empty-volume restore drill with manifest verification; quarterly report | Managed operator; customer data-access approvals may delay a real incident restore |
| Incident acknowledgement | Severity 1 acknowledged within 30 minutes; Severity 2 within four business hours | Incident timeline; monthly report | Paid support tier determines customer notification channel |

The minimum pilot-ready threshold is all of: seven consecutive days of probe
data, a successful backup plus restore drill, webhook and job lifecycle
telemetry enabled, an on-call owner, a published status page, and no unresolved
Severity 1 isolation, credential, or data-loss incident.

## Provider dependencies

Git providers and AI providers are external dependencies and are never counted
as Lachesi control-plane availability. The status page shows them independently:

- Git-provider degradation: signature verification, webhook delivery, pull
  request metadata, source retrieval, and comment publication may be delayed.
- AI-provider degradation: accepted jobs remain durable and use the bounded
  retry/dead-letter policy; no result is fabricated or published.

Reports distinguish a Lachesi runtime failure from a provider response,
authentication, rate-limit, or model-execution failure using the operational
telemetry counters and durable job error class.

## Ownership and communication

| Event | Managed service owner | Customer-visible communication | Self-hosted owner |
|---|---|---|---|
| Upgrade or schema migration | Service operator validates backup, drains work, performs migration, and verifies readiness | Planned notice at least two business days before maintenance; completion or rollback update | Deployment administrator |
| Git or AI provider outage | Service operator classifies, pauses unsafe publication where necessary, and monitors recovery | Status page incident and recovery update | Deployment administrator |
| Security or tenant-isolation event | Security incident lead contains access, revokes affected credentials, preserves audit evidence, and coordinates disclosure | Direct affected-customer notification after containment, plus status-page update where appropriate | Deployment administrator and their security team |
| Backup or restore event | Service operator executes the documented backup/restore procedure and records the drill | Direct notice for a customer-affecting restore | Deployment administrator |

The public status page carries availability incidents and planned maintenance.
Organization administrators receive direct notices for incidents affecting
their tenant. Pilot support uses business-hours email; 24/7 response, named
incident coordination, region commitments, and shorter recovery objectives are
paid support-tier capabilities, not pilot defaults.
