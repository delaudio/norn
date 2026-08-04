# Conventions

## Project

Project name: Norn.

Norn is an open-source, local-first review workspace for pull requests
across Bitbucket Cloud and GitHub.

Artefact root: `.docflow/` - `adr/`, `plan/`, `INDEX.md`, and this file live
under this root; `AGENTS.md` and `CLAUDE.md` stay at the repository root.
Every lifecycle skill resolves paths against this root.

Discovery: the `.docflow/` directory at the repository root is itself the
artefact root. Do not add a root `.docflow` pointer file while this directory
is the artefact root.

Assessment depth: `guided` - the depth chosen at bootstrap. Skill assessments
pre-select it as the recommended depth; the selector still appears, so the
record steers the recommendation and is never applied silently.

Language: English. Keep spelling consistent with nearby project text.

## Existing Governance

This repository also uses Archgate. `.archgate/adrs/` contains the enforced
architecture rules and `.archgate/adrs/*.rules.ts` files contain automated
checks. Docflow records the documentation-led queue and ADR catalogue; it does
not replace Archgate enforcement.

The pre-bootstrap `docs/adr/` records were migrated into this catalogue during
the initial docflow retrofit. The old `docs/adr/` tree should not be recreated.

## ADR Files

ADR filenames use `NNNN-kebab-case-slug.md`, zero-padded to 4 digits, with
contiguous numbering and no reserved gaps.

The number is an integer; the four-digit zero-padding is a display convention
only. Tools sort ADRs numerically, not lexically. Widen the padding if the
catalogue ever approaches `9999`.

Each ADR describes one decision. If a decision splits, supersede the original
ADR and create new ADRs rather than expanding scope inside a single document.

Status lifecycle: `Proposed -> Accepted -> Implemented -> (Superseded |
Deprecated)`.

| Status | Meaning |
|---|---|
| Proposed | Draft. Decision authored but not yet approved. |
| Accepted | Decision approved; implementation authorised. Work item lives in `plan/todo/`. |
| Implemented | Code shipped per the completion event. Work item lives in `plan/done/`. ADR is the authoritative spec the running system matches. |
| Superseded | Replaced by another ADR. The successor is named in `superseded-by:` metadata. |
| Deprecated | Was real; the world moved on; no successor. |

Terminal states (`Superseded` / `Deprecated`) are reachable from any prior
state.

The first persisted status is `Proposed`. There is no separate `Draft` state
and no `brainstorming/` or `drafts/` folder.

Cross-references link by relative path to `adr/NNNN-*.md` when written from the
artefact root, or by relative path from the referring file.

## ADR Shapes

This project uses a single ADR shape. New ADRs use `adr/0000-template.md` and
contain these sections in order: Context, Capability statement, User stories /
scenarios, Acceptance criteria, Out of scope, Open questions, References,
Revision History, Approvals.

Legacy records migrated during bootstrap may preserve their original
technology-decision wording where needed, but every substantive later revision
should move them toward the canonical shape above.

## ADR Privacy

ADRs are internal artefacts. ADR numbers, ADR titles, and the existence of the
ADR catalogue must never appear in any string the product emits to users: UI
copy, API response bodies, error messages, customer-visible log lines, public
documentation, release notes, marketing copy, or support communications.

Norn has one product-specific exception: it may document and implement a
generic `.norn.yaml` policy-source type named `adr`, because reviewing
external repositories that keep architecture records is part of the product
model. The legacy `.lachesi.yaml` name remains a fallback-only compatibility
input. That exception does not allow user-facing references to this
repository's internal catalogue, internal ADR numbers, or internal ADR titles.

Allowed references:

- Inline code comments tying a non-obvious choice to its ADR.
- Commit messages and PR descriptions.
- Internal documents: `AGENTS.md`, `.docflow/INDEX.md`, the `plan/` queue,
  `_agent/` files, and internal runbooks.

Rule of thumb: if a non-builder could ever read the string, the ADR reference
comes out. Refer to the behaviour by its product-level name instead.

## Multi-Agent Rules

A single agent owns this repo. The `_agent/` directory tracks live state and
history; no LOCKS discipline is in use.

## Plan Folder

Pending and shipped work live under `.docflow/plan/`:

- `plan/todo/NNNN-<slug>.md` - pending work, lower numbers run first. Each file
  names the owning ADRs, scope, and exit criteria.
- `plan/done/<YYYY-MM-DD>-<slug>.md` - shipped work, chronological. A `git mv`
  from `todo/` to `done/` is the normal completion event.

The completion event is: pull request merged to `main` with the local gate
passing: `pnpm run typecheck`, `pnpm run test`, and `archgate check`.

When a `plan/todo/` item ships, the file moves to `plan/done/` and the owning
ADRs advance from `Accepted` to `Implemented`. `INDEX.md` is regenerated to
match.

## Concurrency Guardrails

ADR and `plan/todo` numbers are contiguous and assigned at authoring time, so
concurrent branches can pick the same next number. These guardrails keep
numbering collision-free without changing the identity scheme; the number stays
the stable cross-reference key, immutable once merged:

- G1 - decide before do. Prefer to merge an ADR and its plan items to `main`
  before implementation work begins, so work branches start from a `main` that
  already carries the numbered ADR.
- G2 - check before merge. Before integrating, sync onto the current `main` and
  run the docflow audit. If your ADR number or `plan/todo` slot now clashes
  with what landed on `main`, renumber locally before integrating.
- G3 - gate backstop. Integration is single-threaded; it rejects a duplicate
  number as the last line of defence, and the later author renumbers.
- G4 - claim before do. Before implementing a queued item, claim it by opening
  a draft PR referencing the item or recording ownership in the current focus
  file. G1-G3 protect the number; G4 protects the work assignment.

## Git Contract

- Conventional Commits are required.
- Commits touching ADRs require a `Rationale:` footer.
- Signed commits are expected.
- ADR revision tags such as `adr-NNNN-rN` are not used.
- Agent commits do not include `Co-Authored-By` trailers unless requested.
- PR-based integration uses squash merge by default.
