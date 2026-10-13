# Current Repository TUI Baseline

## Owning ADRs

- `../../adr/0020-opentui-rust-stdio-workspace.md`
- `../../adr/0006-terminal-ui.md` (historical behavior reference)

## Scope

Maintainer direction on 2026-10-13 supersedes desktop parity requirements:
"TUI come riferimento; ritira la parità desktop". Restore the terminal's
current-repository PR entry point. Retire mandatory replication of desktop-only
workflows, keeping existing useful components and Rust services. Preserve
stored data and prohibit destructive cleanup. This urgent correction takes
precedence over optional desktop follow-ups in plans 0072/0073; it does not
claim or repeat their already merged implementations.

## Corrected behavioral scope (2026-10-13 feedback)

Current-repository filtering alone is insufficient. Restore the Ratatui main
screen: filter panel (Open/Draft/Merged/Local), PR/local-target list, AI review
panel and modal Settings. Preserve `s` settings, `a` explicit AI review,
`Enter` load selection, and `g` diff choice (`g` native diff / `b` authenticated
browser), with return from diff to the original selection/filter. Load and show the
selected PR description independently of snapshot/diff verification; failures
must not hide metadata. Fence late detail responses by full provider scope. Reuse backend
services, not the desktop-style standalone PR browser. Add native keyboard,
modal/input-fencing, narrow/wide frame and default-entry lifecycle evidence.

## Exit Criteria

1. Default production startup resolves the backend's launch repository using
   read-only Git identity and opens its PR list in the terminal home. It does not load all
   configured targets/history or capture a working tree automatically.
2. Explicit `--workspace` retains multi-repository access; `--current-repo`
   selects the default mode. Conflicting/unknown arguments fail clearly.
3. Failed repository resolution produces an actionable error, without guessing
   a provider or falling back silently to global targets. Settings reload and
   review completion preserve the launch scope; local capture stays explicit.
4. Terminal review failures retrieve and display the backend's actual bounded
   error, including when the final event and status have equal sequences.
5. Tests cover both providers, an unconfigured launch repository, no implicit
   capture/review, error handling, explicit workspace mode and scope retention.
6. ADR revision, migration matrix and epic/follow-up issue scope reflect the
   maintainer's retirement decision. Desktop-only parity is not a cutover gate;
   data preservation, Rust/headless contracts and terminal quality remain gates.
7. Root typecheck/tests/Archgate and UI typecheck/tests pass before integration.

## Dependencies

Existing native provider reads/snapshots and read-only `local.options` IPC.
Packaging/install and final Tauri/Ratatui deletion remain #308/#309.
