# Terminal UI Foundation

## Owning ADRs

- `../../adr/0006-terminal-ui.md`

## Scope

Implement the first foundation for the terminal UI roadmap:

- extract shared native review APIs that can be called by both Tauri commands
  and non-Tauri callers;
- keep Tauri command names and browser mock IPC contracts stable;
- add the initial `lachesi-tui` Rust entrypoint and ratatui rendering skeleton;
- render configured repositories, open pull requests, selected pull request
  metadata, comments, and unified diff loading/error states;
- add the terminal lifecycle guard needed to restore raw mode on exit and panic;
- start TUI AI review without rerunning repository analyzers already completed
  by the development flow;
- add task runner/script entrypoints while preserving `Makefile`/`justfile`
  recipe parity.

The initial foundation scope excluded staged comment authoring. That capability
landed on the same long-running plan before closeout. Separate repository
extraction and rebuilding a full git TUI remain out of scope.

## Exit Criteria

- ADR 0006 AC1: a separate Rust TUI entrypoint or workspace crate exists in
  this repository.
- ADR 0006 AC2: read-only provider/review flows reuse existing Rust native
  config, credential, provider, local repository, and review modules.
- ADR 0006 AC3: existing Tauri command names and mock IPC contracts remain
  stable unless intentionally updated in the same change.
- ADR 0006 AC4: the TUI can render configured repositories, open pull requests,
  selected pull request details, comments, and unified diff states.
- ADR 0006 AC5: review comments remain local drafts until explicit batch
  publication.
- ADR 0006 AC6: focused terminal render/layout tests run without a real
  terminal session.
- ADR 0006 AC7: native provider diff viewing does not check out branches, and
  terminal state is restored on exit, interruption, and panic.
- ADR 0006 AC8: TUI AI review skips duplicate local evidence analyzers.

## Dependencies

- `../../adr/0006-terminal-ui.md`
- GitHub issues #80, #81, and #82
- `../todo/0001-agentic-code-policy-pack.md` remains queued; the
  maintainer explicitly prioritized terminal work ahead of it.

Shipped at HEAD `fa13281d2b2f6f65a761f7c14f8643a4efdb12d5` through PR #146.
