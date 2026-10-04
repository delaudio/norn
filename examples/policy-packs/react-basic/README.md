# React Review Basics

A small public example pack for reviewing React changes. It demonstrates
prompt rules, a path rule scoped to components, and analyzer defaults.

## Install Locally

```yaml
# .norn.yaml
version: 0.1

policy:
  packs:
    - ./examples/policy-packs/react-basic
```

Use an absolute path or a checked-in relative path your team controls when the
pack lives outside the reviewed repository.

## What It Covers

- `react.effect-cleanup`: effects that leak subscriptions or timers.
- `react.stable-list-keys`: unstable list keys.
- `react.state-ownership`: state lifted without a workflow need.
- `react.hooks-rules`: conditional or misplaced hooks.
- `react.presentational-boundary`: orchestration leaking into components.

Analyzer defaults enable `typecheck` and `tests` and leave `lint` optional.

## Adapting It

Copy `pack.yaml` into a private repository or an internal bundle and keep the
rule ids stable. Narrow or remove rules that do not match your component
conventions instead of suppressions that hide real findings. Keep customer
names, private paths, credentials, and model transcripts out of committed
rules.

## Files

- `pack.yaml` - loadable policy pack manifest.
