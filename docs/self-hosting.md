# Self-hosting the review service

The optional self-hosted service is a process boundary around Norn's public
shared-review contracts. It does not change desktop, TUI, or `norn review`:
those remain fully local and do not depend on this process.

## Start from an empty volume

Build and start the service with a named Docker volume:

```sh
docker compose -f compose.self-hosted.yaml up --build -d
curl --fail http://127.0.0.1:8080/healthz
curl --fail http://127.0.0.1:8080/readyz
curl --fail http://127.0.0.1:8080/metrics
```

The startup boundary creates the configured data directory with owner-only
permissions, opens `/var/lib/norn/norn.sqlite3`, and applies SQLite
migrations before binding its HTTP port. A successful `/readyz` response means
configuration, the persistent directory, and migrations are valid. A failed
startup never opens the port, so orchestration should treat it as unready.

Compose retains the `lachesi-data` volume name as a storage compatibility alias
so an image upgrade continues to attach the existing volume, but mounts it at
the canonical `/var/lib/norn` path. The runtime copies `lachesi.sqlite3` to
`norn.sqlite3`, validates the copy, and retains the legacy database for rollback.

The Compose service key also remains `lachesi` during the bounded compatibility
window because operators commonly script it directly. This avoids orphaning
the old container during upgrade; the image entrypoint, executable,
environment, storage path, healthcheck, and output are canonical Norn.

## Configuration

| Variable | Required | Default | Purpose |
| --- | --- | --- | --- |
| `NORN_SERVICE_DATA_DIR` | Yes | none | Absolute path to a persistent, writable volume. |
| `NORN_SERVICE_BIND_ADDR` | No | `0.0.0.0:8080` | HTTP listener address. |
| `NORN_REVIEW_DATA_DIR` | Internal | set from service data dir | Storage location used by the public SQLite store. Do not set it separately for the container. |

Equivalent `LACHESI_*` names remain fallback aliases during the compatibility
window; when both forms are present, the `NORN_*` value wins.

The container image contains no provider, model, OIDC, webhook, or database
credentials. Deploy provider and model credentials through the credential
broker referenced by the shared-review configuration, using your platform's
secret-reference mechanism. Never place raw secrets in compose files, image
layers, environment-file commits, logs, events, jobs, findings, or audit data.

The service listens on TCP port `8080` by default. `GET /healthz` reports that
the process is alive; `GET /readyz` is available only after the persistent
store has initialized successfully. Put provider ingress behind an authenticated
reverse proxy and expose only the routes required by your deployment.

## Operational metrics

`GET /metrics` exposes Prometheus text metrics for received events, queued and
completed jobs, failures, scheduled retries, dead-letter jobs, publications,
queue wait, review duration, and publication duration. Labels are bounded to
provider, outcome, and the fixed `repository` scope level. Repository names,
tenant ids, delivery ids, prompts, findings, and model responses are never
metric labels or metric values.

Each accepted review also receives an opaque correlation id retained in a
bounded in-process trace map keyed by durable job id. Operators can use that
id across ingress, job execution, and publication adapters without adding it
as an unbounded metrics label. Scrape `/metrics` only through an authenticated
operations network boundary; it is intentionally machine-readable rather than
a public status page.

The Compose healthcheck runs `norn service healthcheck`, which requests the
loopback `/readyz` endpoint with bounded socket timeouts. It therefore verifies
both the process and its serving readiness, rather than only checking for a
database file.

## Offline smoke test

Run the smoke path against an empty or disposable persistent directory:

```sh
docker compose -f compose.self-hosted.yaml run --rm \
  -e NORN_SERVICE_DATA_DIR=/var/lib/norn \
  lachesi service smoke
```

It accepts one synthetic provider-neutral event and completes a durable job
with an offline executor. It does not call GitHub, Bitbucket, Codex, Claude,
or any other network provider. The command exits non-zero if migration,
enqueueing, claiming, or durable completion fails.

## Persistence and upgrade

Keep the `lachesi-data` volume when restarting or replacing a container. It
holds cursors, durable jobs, stored review findings, reviewer feedback, and
administrative audit events. The service uses WAL-mode SQLite and applies
known migrations synchronously at startup; do not run two containers against
the same SQLite volume.

For an upgrade:

1. Stop the current container after in-flight work has drained.
2. Back up the persistent volume using your platform's volume snapshot tool.
3. Start the new image against the same volume.
4. Wait for `/readyz`, then run `norn service smoke` against a disposable
   volume as a deployment check.
5. Roll back to the backed-up volume if the new process does not become ready.

The service does not delete persistent state during normal restart. Retention,
credential revocation, repository deletion, and tenant deletion remain policy
and control-plane responsibilities of the shared-review implementation.

## Backup and restore

Create a backup while the service is running with an absolute, new destination
directory:

```sh
norn service backup /secure-backups/norn-2026-07-29
```

The backup is a consistent SQLite snapshot plus a manifest and SHA-256 digest.
It contains the durable database state: cursors, jobs, findings, feedback,
audit events, policy and organization configuration, and credential metadata
including encrypted credential ciphertext. It deliberately excludes repository
source code and prompt bodies; do not add those artifacts to this backup format
without an explicit retention and access-control decision.

Restore only into a new empty data directory, before starting a serving
instance:

```sh
NORN_SERVICE_DATA_DIR=/var/lib/norn-restored \
  norn service restore /secure-backups/norn-2026-07-29
```

Restore rejects non-empty destinations, malformed manifests, checksum changes,
corrupt SQLite databases, and backups from a newer schema. Credential ciphertext
is restored as ciphertext only: it remains unusable until the deployment is
configured with the same external master key or the appropriate rotated key
material. Keep backup artifacts encrypted and access-controlled by your backup
platform.

## Retention and repository deletion

The service exposes a repository-scoped retention API with `dryRun` and
`execute` modes. The default organization policy is: source-derived review
content, including stored prompts and responses, for 30 days; completed job
records for 90 days; finding feedback, publication state, and pull-request
state for 365 days; and administrative audit metadata for seven years.
Aggregate metrics are retained indefinitely only in non-identifying form.
Repository deletion removes the source-derived classes immediately but keeps
aggregate metrics and separately governed audit metadata.

Each retention operation reports counts for `review_content`, `review_jobs`,
`finding_feedback`, `finding_publications`, and `pull_request_state`, plus the
explicitly retained aggregate-metrics and audit-metadata classes. Dry runs,
successful executions, and failed executions are durably recorded. Execute
mode is one SQLite transaction: an error rolls back every repository-content
deletion, allowing a safe retry without a partial result.
