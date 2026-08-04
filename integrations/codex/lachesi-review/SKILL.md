---
name: lachesi-review
description: Deprecated compatibility alias for the Norn code-review skill. Use only when existing agent instructions still invoke $lachesi-review during the bounded migration window.
---

# Deprecated Lachesi Review Alias

This skill preserves existing `$lachesi-review` invocations during the bounded
compatibility window. Norn is the canonical identity.

If `NORN_REVIEW_CHILD=1` or `LACHESI_REVIEW_CHILD=1`, stop immediately: the
current process is already a reviewer child.

Run Norn as an independent, read-only reviewer after the repository's normal
validation commands pass. Before every push, review the exact changes that
will be published. Use `--scope working-tree` for uncommitted changes and
`--scope branch` for committed changes; use `--pr <id>` only when the user
names a pull request.

Prefer `$HOME/.local/bin/norn` when executable, then `norn` from `PATH`. During
the compatibility window only, fall back to `$HOME/.local/bin/lachesi` and
then `lachesi`. If none is available, report the setup failure and do not
substitute an ad hoc review.

For a committed branch whose validation already passed, run:

```bash
"$HOME/.local/bin/norn" review --repo-path . --scope branch --format json \
  --fail-on-findings
```

For uncommitted task changes, replace the scope with `working-tree`. Do not
pass `--run-analyzers` in this post-task workflow. Pass a provider, model,
profile, effort, base, or PR only when repository guidance or the user selects
it.

Exit code `0` means the gate passed. Exit code `1` means configured findings
were returned: fix high-confidence in-scope findings, rerun affected tests,
and run one bounded review again. Exit code `2` or greater is a setup,
configuration, repository, analyzer, provider, or runtime failure; report it
and do not treat it as a code finding.

Never publish provider comments, commit, push, or broaden permissions merely
because this review skill was invoked; those actions require the surrounding
task to authorize them.
