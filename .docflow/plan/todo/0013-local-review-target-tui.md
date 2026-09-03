# Local Review Target in the Terminal UI

## Owning ADRs

- `../../adr/0017-review-local-changes-before-publication.md`

## Scope

Implement the shared native snapshot for unpublished local work and make it a
first-class terminal UI target. The snapshot combines commits ahead of the
current upstream with staged, unstaged, and eligible untracked changes without
double counting, while preserving bounded sensitive-file and binary handling.

Add Local to the TUI filter, keyboard cycle, and mouse targets. Render local
repository and branch context through the native unified/split and authenticated
browser diff views. Support explicit refresh and AI review of the immutable
loaded snapshot while removing provider-only comment, approval, publication,
and branch-sync actions from local targets.

Out of scope: desktop UI integration, continuous filesystem watching, provider
publication, and repository mutations such as commit, push, stash, or discard.

## Exit Criteria

- ADR 0017 AC1-3: a tested native snapshot reports branch/upstream metadata,
  ahead commits, warnings, file metadata, and a bounded unified diff for both
  upstream and no-upstream repositories without modifying them.
- ADR 0017 AC4: Local participates in keyboard and mouse selection, includes
  configured usable local repositories, and refreshes on demand.
- ADR 0017 AC5: native unified/split and authenticated browser views render the
  local snapshot with accurate non-PR context.
- ADR 0017 AC6: AI review consumes the loaded immutable snapshot and no
  provider-only action is available for a local target.
- ADR 0017 AC8-9: bounds, stale-result fencing, fallback behavior, and existing
  provider PR workflows have regression coverage.
- `pnpm run typecheck`, `pnpm run test`, and `archgate check` pass.

## Dependencies

- `../../adr/0017-review-local-changes-before-publication.md`

