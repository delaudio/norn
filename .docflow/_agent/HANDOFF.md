# Handoff

Entry point for a fresh agent picking up this repo. Read these files in order
before any tool calls:

1. `AGENTS.md` - hard rules. Read in full.
2. `.docflow/CONVENTIONS.md` - authoring rules, ADR status semantics, and plan
   folder convention.
3. `.docflow/plan/README.md` - queue convention.
4. `.docflow/_agent/CURRENT_FOCUS.md` - live snapshot.
5. `.docflow/INDEX.md` - ADR catalogue.
6. Tail of `.docflow/_agent/WORKLOG.md` - confirms what landed.
7. The next queue item at `.docflow/plan/todo/NNNN-*.md`, and the ADRs it
   names, before implementing.

## Stop conditions

Stop and surface the issue rather than guessing if:

- The verify gate fails.
- The queue is empty.
- A `plan/todo/` item references an ADR whose status is not Accepted.
- An ADR's acceptance criteria are ambiguous or untestable.
- Two same-priority plan items target the same files.

## Active migration mandate

The maintainer authorized migration epic #294 through verified PRs and squash
merges, without Norn reviews. The terminal workflow is the reference; mandatory
desktop-only parity was retired on 2026-10-13. Do not revive that work from old
queue descriptions. Current-repository terminal home shipped in #394.

The latest bounded default-build task shipped in #397: empty Cargo defaults,
OpenTUI plus Rust CLI/backend, browser diff preserved, explicit legacy adapters
and separate CI. Read its plan/done receipt and CURRENT_FOCUS before resuming.
#396 remains a paused draft; its branch and owned followup stash are preserved.
Do not implement that work concurrently or merge it without rebasing onto main
and repairing its remaining Windows fixture failures.

Older plans 0022–0030 contain merged work that needs evidence reconciliation,
not duplicate implementation. Full packaging/upgrade proof and final source
removal remain #308/#309 work. ADR0020 stays Accepted until full cutover. Native
installed-platform proof must not be replaced by mocks or compile checks.
Never read the user's untracked root config or probe their credentials during
tests; use owned fixtures. Do not publish releases as part of this work.
