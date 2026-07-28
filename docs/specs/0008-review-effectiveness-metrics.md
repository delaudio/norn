# Spec 0008 - Review effectiveness metrics

- Status: Implemented
- Date: 2026-07-28
- GitHub issue: #100

## Purpose

Lachesi aggregates review activity from persisted structured review runs and
append-only finding feedback. The report describes review throughput, finding
mix, explicit reviewer dispositions, and first-review latency without ranking
individual developers or treating a finding as a prevented incident.

The versioned JSON schema is `lachesi.review-effectiveness.v1`. The same
aggregation powers the human-readable `lachesi metrics` report.

## Command

```sh
lachesi metrics \
  [--tenant <id>] \
  [--provider github|bitbucket] \
  [--workspace <name>] \
  [--repo <slug>] \
  [--from <unix-ms>] \
  [--to <unix-ms>] \
  [--format human|json] \
  [--json] \
  [--output <path>]
```

The default tenant is `local`. A repository filter requires its workspace.
`--from` is inclusive and `--to` is exclusive. Both are non-negative Unix
timestamps in milliseconds. The time range selects successful review runs by
completion time.

Tenant scope is mandatory at the storage boundary. A report never reads runs or
feedback from another tenant, and identical repository/pull-request keys may
coexist in separate tenants. Repository rows are ordered by provider, workspace,
and repository; count rows are ordered by their stable key.

## Review and finding counts

`reviewCount` is the number of successful structured review runs completed in
the selected range. Failed, cancelled, and incomplete runs do not contribute.
Repeated successful reviews of one pull request each count as review runs.

`findingCount` is the number of findings materialized by those selected runs.
`findingsBySeverity` always contains `critical`, `high`, `info`, `low`, and
`medium` keys. `findingsByCategory` always contains `architecture`, `bug`,
`docs`, `maintainability`, `other`, `performance`, `security`, `test`, and
`typing`. Keys are emitted in lexical order, including zero counts.

The top-level `summary` covers the complete filter. `repositories` repeats the
same summary shape for each matching provider/workspace/repository.

## Feedback metrics

Each finding is identified by tenant, provider, workspace, repository, pull
request, review run, and finding fingerprint. Feedback events are ordered by
`occurredAt`, then by event id. The latest event before the exclusive `to`
boundary defines the current disposition. Feedback before `from` still applies
to a finding whose run is selected because the report describes the current
known disposition as of the report end.

The feedback fields use these definitions:

- `eligibleFindings`: every finding in selected successful runs; this is the
  denominator for acceptance, false-positive, and fixed rates.
- `findingsWithFeedback`: findings with at least one explicit feedback event,
  including findings whose latest action is `reopened`.
- `findingsWithoutFeedback`: eligible findings with no feedback event. These
  remain in all rate denominators.
- `acceptedFindings`: findings whose latest action is `accepted` or `fixed`.
  A fixed finding is therefore a subset of accepted findings.
- `fixedFindings`: findings whose latest action is `fixed`.
- `falsePositiveFindings`: findings whose latest action is `false_positive`.
- `dismissedFindings`: findings whose latest action is `dismissed`.
- `reopenedFindings`: findings whose latest action is `reopened`.

Every rate object exposes `numerator`, `denominator`, and `basisPoints`.
`basisPoints` is `floor(numerator * 10000 / denominator)` and is `null` when the
denominator is zero. This avoids unstable floating-point output while keeping a
machine-readable percentage.

## Time to first review

Runs are grouped by provider, workspace, repository, and pull request. The
earliest successful completed run is the first review, with run id as the
deterministic tie-breaker for equal completion times. Its latency is
`finishedAt - createdAt`.

The first-review sample contributes only when that first completion falls
inside the selected time range. Later reviews never replace an earlier review
that falls outside the range. `timeToFirstReview` exposes sample count, total,
integer average, minimum, and maximum milliseconds. Average, minimum, and
maximum are `null` when there are no samples.

This latency measures Lachesi review execution from stored run creation to
completion. It does not claim to measure time from pull-request creation,
reviewer response time, or delivery to production.

## Missing and invalid data

Successful reviews with no findings contribute to `reviewCount` and
first-review latency but not to feedback denominators. Findings without
feedback remain visible through `findingsWithoutFeedback`; Lachesi does not
infer acceptance or rejection.

Legacy stores without structured review-run ids do not become synthetic metric
runs. Malformed structured runs, unsupported providers, invalid timestamps,
unknown severity/category values, duplicate fingerprints within one run, and
malformed feedback fail the report instead of silently changing its totals.
Malformed data outside the selected tenant/provider/repository scope cannot
affect the requested report.

## JSON shape

```json
{
  "schemaVersion": "lachesi.review-effectiveness.v1",
  "filter": {
    "tenantId": "local",
    "fromMs": 1000,
    "toMs": 2000
  },
  "summary": {
    "reviewCount": 1,
    "findingCount": 2,
    "findingsBySeverity": [
      { "key": "critical", "count": 0 },
      { "key": "high", "count": 1 }
    ],
    "findingsByCategory": [
      { "key": "bug", "count": 1 }
    ],
    "feedback": {
      "eligibleFindings": 2,
      "findingsWithFeedback": 1,
      "findingsWithoutFeedback": 1,
      "acceptedFindings": 1,
      "falsePositiveFindings": 0,
      "fixedFindings": 0,
      "dismissedFindings": 0,
      "reopenedFindings": 0,
      "coverageRate": {
        "numerator": 1,
        "denominator": 2,
        "basisPoints": 5000
      },
      "acceptanceRate": {
        "numerator": 1,
        "denominator": 2,
        "basisPoints": 5000
      },
      "falsePositiveRate": {
        "numerator": 0,
        "denominator": 2,
        "basisPoints": 0
      },
      "fixedRate": {
        "numerator": 0,
        "denominator": 2,
        "basisPoints": 0
      }
    },
    "timeToFirstReview": {
      "sampleCount": 1,
      "totalMs": 500,
      "averageMs": 500,
      "minimumMs": 500,
      "maximumMs": 500
    }
  },
  "repositories": []
}
```

Count arrays in a real report include every stable key, including zeros. The
shortened arrays above only illustrate field names.
