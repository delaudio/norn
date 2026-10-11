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

The maintainer explicitly authorized finishing every remaining epic #294 task
through verified PRs and squash merges, without Norn reviews. Read CURRENT_FOCUS
for the current claimed branch and WORKLOG for actual merge proof before picking
a pending file by number. Older migration plans 0022–0030 contain already merged
work and require evidence reconciliation in #306, rather than duplicate code.
#357–#364 are bounded P0 retained-parity deliveries; #307–#309 follow them. No
private credential/profile access or live provider mutation is an automated test
requirement. Inherited CLI credential status now uses an owned child mock keyring.
A failed gate blocks integration while it is diagnosed/repaired; a passing rerun
does not erase the observed publication-fixture intermittency recorded in #307.
Native recovery/installed-platform proof must not be replaced by mocks/compile
checks. ADR 0020 remains Accepted until the full cutover, and the goal remains
active until every required issue and the epic are actually completed.
