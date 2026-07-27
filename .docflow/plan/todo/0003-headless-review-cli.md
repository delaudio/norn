# Headless Review CLI

## Owning ADRs

- `../../adr/0007-headless-review-cli.md`
- `../../adr/0006-terminal-ui.md`

## Scope

Implement a non-interactive `lachesi review` command for local working-tree,
branch, and provider pull-request changes. Reuse native review configuration,
AI provider execution, evidence, and structured finding semantics; add stable
Markdown/JSON output, severity exit behavior, recursion protection, and a
repository-scoped Codex skill.

Out of scope: automatic finding remediation inside Lachesi, comment
publication, commit/push automation, and interactive review threads.

The maintainer later clarified that post-validation review must not rerun
analyzers on any automated or terminal launch surface. The implementation
therefore aligns headless, skill, TUI, and desktop GUI review defaults.

The maintainer explicitly authorised immediate implementation on 2026-07-24
despite lower-numbered historical queue items remaining open.

## Exit Criteria

- ADR 0007 AC1: `lachesi review` runs without starting the Tauri app and emits
  Markdown or JSON.
- ADR 0007 AC2: working-tree review covers staged, unstaged, and untracked text
  changes without repository writes.
- ADR 0007 AC3: branch and provider pull-request targets produce reviewable
  diffs.
- ADR 0007 AC4: effective prompt, profile, and policy pack behavior is shared;
  analyzers are skipped by default and available through `--run-analyzers`.
- ADR 0007 AC5: reviewer execution is read-only and exports
  `LACHESI_REVIEW_CHILD=1`.
- ADR 0007 AC6-7: structured output and documented exit codes have focused
  automated coverage.
- ADR 0007 AC8: `integrations/codex/lachesi-review/SKILL.md` is present,
  installable in the user skill directory, and validated.
- The repository verification gate passes: `pnpm run typecheck`,
  `pnpm run test`, and `archgate check`.
- The installed CLI is exercised against
  `~/dev/compri/procurement-frontend`.

## Dependencies

- `../../adr/0007-headless-review-cli.md`
- `../../adr/0006-terminal-ui.md`
- `../../adr/0005-agentic-policy-pack-prototype.md`
- `../../../docs/specs/0006-cli-headless-review.md`
