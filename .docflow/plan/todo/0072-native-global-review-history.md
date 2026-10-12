# Native Global Review History

## Owning ADRs

- `../../adr/0020-opentui-rust-stdio-workspace.md`
- `../../adr/0007-headless-review-cli.md`

## Scope

Port the desktop's global review history into the OpenTUI workspace. Add a
bounded, paginated `review.jobs` method returning stored review jobs with
actual status, timestamps, trigger, repository/PR/thread identity and
cancellation/error information, reusing `review_storage::list_recent_review_jobs`.
Jobs, structured runs and conversations stay distinct; a job id never becomes a
run id. This slice delivers the global job list and a terminal view; analytics
(overview/closed-PR/effectiveness) and legacy provider-less reconciliation
remain follow-ups within #361.

GitHub issue: #361 (epic #294 step 12/15).

## Exit Criteria

- `review.jobs` returns `{ jobs, offset }` bounded by a limit, newest first,
  with status/trigger/identity/timestamps/error; no fake statuses.
- The shell can open a bounded global history view listing the jobs.
- `cargo clippy -D warnings`, `cargo test`, `pnpm run lint/typecheck/test`, the
  `ui` job and `archgate check` pass.

## Dependencies

- `../../adr/0020-opentui-rust-stdio-workspace.md`
- `../../adr/0007-headless-review-cli.md`
