# Current Focus

Git is authoritative. Durable history lives in WORKLOG and plan/done.

## Active state — 2026-10-14

Single writer; no delegation or Norn reviews per maintainer.
PR394 actually squash-merged c5bcabf6b1cdade2accf01b172b5511dc5fe6ce9 at
2026-10-14T08:26:17Z after all8 exact-head CI checks passed, including actual
macOS/Linux/Windows workflow and Windows reconciliation correction. Plan0074
reconciled into done only after merge. ADR0020 remains Accepted.
Plan0075 claimed on fix/opentui-hardening in /private/tmp/norn-current-repo-first.
Client now bounds byte framing before concat, strictly validates shared schema
and rejects/reaps malformed native peers and stalled handshakes. Existing
settings/auth/data/fix/thread results and nullable operation error were missing
from schema; added exact definitions and shared synthetic fixtures. No mutation
replay/provider retry/private credentials.
Local complete Rust776core+80stdio+6CLI+5onboarding passed; owned UI/Rust
journey and6 actual POSIX PTY cases pass, including resize/raw UI Ctrl+C.
Updated UI329 tests, root127 tests/tooling and schema fixtures pass.
Native suites cap test concurrency at four; exact-head3OS CI remains pending.
Next after verified hardening: #308 matched installable artifacts, then #309
Tauri/Ratatui removal. Do not publish releases or touch stored user data.
Preserve root untracked .norn.yaml and user stash.
