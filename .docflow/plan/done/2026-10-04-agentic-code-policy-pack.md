# Agentic Code Policy Pack

## Owning ADRs

- `../../adr/0005-agentic-policy-pack-prototype.md`

## Scope

Implement GitHub issue #43 by adding a public `agentic-code` policy pack
prototype that can be loaded from a local repository path. Include the manifest,
profile definitions, analyzer defaults, adaptation guidance, example repo
config, and sample markdown/JSON outputs.

Out of scope: hosted pack distribution, private/commercial pack content, and a
new static-analysis runtime.

## Exit Criteria

- ADR 0005 AC1: `examples/policy-packs/agentic-code/pack.yaml` exists.
- ADR 0005 AC2: the manifest contains 15-25 agentic-code policy declarations.
- ADR 0005 AC3: the manifest defines `agentic-fast`, `agentic-balanced`, and
  `agentic-strict` profiles.
- ADR 0005 AC4: example repository config and example markdown/JSON outputs
  exist beside the pack.
- ADR 0005 AC5: pack documentation explains safe adaptation without private
  context or credentials.
- ADR 0005 AC6: automated coverage loads the checked-in pack through the local
  pack loader.

## Dependencies

- `../../adr/0005-agentic-policy-pack-prototype.md`
- GitHub issue #43

## Shipped

- PR: `docs/ship-agentic-policy-pack` (GitHub issue #43).
- Pack manifest, profiles, examples, adaptation guidance, and loader coverage
  landed in commit `36353fb` and are verified by the pack-loader test in
  `src-tauri/src/repo_config.rs`.
- This change closes the queue item, advances ADR 0005 to `Implemented`, and
  regenerates the ADR index.
