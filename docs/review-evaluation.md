# Review quality evaluation

`lachesi evaluate` evaluates a versioned, sanitized corpus without contacting a
provider, an AI model, or a customer repository. It reads checked-in review
snapshots and expected results, emits JSON, and exits with status `1` if the
explicit baseline is not met.

```sh
pnpm run evaluate
# or
make evaluate
```

The default corpus is `fixtures/review-evaluation/v1/corpus.json`; its matching
baseline is `fixtures/review-evaluation/v1/baseline.json`. The corpus includes
sanitized logic, security, persistence, concurrency, API-contract, frontend,
and Rust diffs plus a clean diff where the correct result is no finding.

Each case records the provider, model, configuration version, and review
duration that produced its observed findings. The runner classifies observations
as expected, optional, unexpected, or missed, then reports precision-oriented
metrics, false positives, missed expected findings, anchor accuracy, and total
and average duration.

To write a result artifact for CI or comparison:

```sh
lachesi evaluate --output /tmp/review-evaluation.json
```

Use `--corpus` and `--baseline` to evaluate a proposed new corpus version.
Corpus and baseline versions must match. Raising or lowering thresholds requires
an intentional baseline change in review; the runner never changes production
prompts, policies, or model configuration.

Fixtures must remain sanitized and reviewable. Do not add customer code,
credentials, proprietary identifiers, or live provider data.
