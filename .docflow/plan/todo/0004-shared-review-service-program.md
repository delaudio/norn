# Shared Review Service Program

## Owning ADRs

- `../../adr/0008-shared-review-service-boundary.md`

## Scope

Implement the shared review service program described by GitHub issues
#90-#114 in dependency order. Deliver the public provider-neutral contracts and
extension ports first, then webhook ingress, durable orchestration, isolated
workers, publication, tenant identity, credential management, policy,
retention, telemetry, deployment, and conformance capabilities.

Every implementation item must preserve fully independent local desktop, TUI,
and headless review. Provider-hosted committed pull-request content is the only
source accepted by the shared service.

Out of scope: billing implementation, a production hosting-vendor decision,
uploading local working trees, or moving public local-engine behavior into a
commercial component.

## Exit Criteria

- ADR 0008 AC1-3: local review remains independent and public contracts define
  the ingress, queue, credential, execution, storage, and publication ports.
- ADR 0008 AC4-7: tenant/repository identity, credential separation, retention,
  deletion, and cross-tenant conformance are implemented and tested.
- ADR 0008 AC8-9: review and publication idempotency, stale-head handling,
  durable retry, and dead-letter behavior are implemented.
- ADR 0008 AC10-11: managed and self-hosted implementations use public extension
  points without changing local review semantics.
- ADR 0008 AC12: deployments expose effective trust and retention policy.
- GitHub issues #90-#114 are closed or explicitly removed from the program by a
  later accepted decision.
- The repository verification gate passes: `pnpm run typecheck`,
  `pnpm run test`, and `archgate check`.

## Dependencies

- `../../adr/0008-shared-review-service-boundary.md`
- `../../adr/0007-headless-review-cli.md`
- GitHub issue #90 is the first implementation item.
