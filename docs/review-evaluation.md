# Review quality evaluation

`norn evaluate` evaluates a versioned, sanitized corpus. Offline mode reads
checked-in review snapshots and expected results. `--live` reruns each case through
the current local review pipeline and records provider/model/config execution metadata
and raw artifacts (payload redacted) for inspection.

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
norn evaluate --output /tmp/review-evaluation.json
```

Use `--corpus` and `--baseline` to evaluate a proposed new corpus version.
Corpus and baseline versions must match. Raising or lowering thresholds requires
an intentional baseline change in review; the runner never changes production
prompts, policies, or model configuration.

`--live` is opt-in and requires `--allow-provider-diff` because it sends corpus
diffs to configured providers. It accepts either:

```sh
norn evaluate --live --allow-provider-diff --prompt-profile minimal
# or
norn evaluate --live --allow-provider-diff --minimal
```

The default prompt profile mirrors the regular review pipeline, while `minimal`
builds a tight JSON-only prompt to isolate model effects in a controlled comparison.

Offline scoring is deterministic and useful for stable regression checks, but it is
not itself a guarantee of current live review quality.

Fixtures must remain sanitized and reviewable. Do not add customer code,
credentials, proprietary identifiers, or live provider data.
