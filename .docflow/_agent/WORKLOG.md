# Agent Worklog

Append one row per commit. Newest at the bottom.

| Date | Commit | Branch | Item | Notes |
|------|--------|--------|------|-------|
| 2026-07-23 | `8aee0c3` | `codex/tui-roadmap` | `.docflow/plan/todo/0002-terminal-ui-foundation.md` | Queued terminal UI ADR/plan and initial TUI binary/task-runner skeleton. |
| 2026-07-23 | `816522e` | `codex/tui-roadmap` | `.docflow/plan/todo/0002-terminal-ui-foundation.md` | Extracted shared native review read APIs for non-Tauri callers. |
| 2026-07-23 | `9ec17e0` | `codex/tui-roadmap` | `.docflow/plan/todo/0002-terminal-ui-foundation.md` | Added TUI repository/PR loading, detail navigation, comments, and diff summary rendering. |
| 2026-07-23 | `6feb7d5` | `codex/tui-roadmap` | `.docflow/plan/todo/0002-terminal-ui-foundation.md` | Added lazygit launch from the TUI with terminal suspension and error handling. |
| 2026-07-23 | `802cef6` | `codex/tui-roadmap` | `.docflow/plan/todo/0002-terminal-ui-foundation.md` | Added session-local draft comments and explicit batch publish in the TUI. |
| 2026-07-23 | `6a07e25` | `codex/tui-roadmap` | `.docflow/plan/todo/0002-terminal-ui-foundation.md` | Hardened temporary repository fixtures for parallel Rust test runs. |
| 2026-07-23 | `2eedb4b` | `codex/tui-roadmap` | `.docflow/plan/todo/0002-terminal-ui-foundation.md` | Added mouse selection, GitHub-capable TUI loading, and explicit AI review trigger support. |
| 2026-07-23 | `b872d78` | `codex/tui-roadmap` | `.docflow/plan/todo/0002-terminal-ui-foundation.md` | Added `.lachesi/` policy-folder loading and terminal AI review output display. |
| 2026-07-23 | `8def318` | `codex/tui-roadmap` | `.docflow/plan/todo/0002-terminal-ui-foundation.md` | Changed repository-owned prompts to replace the built-in AI review prompt. |
| 2026-07-23 | `128166c` | `codex/tui-roadmap` | `.docflow/plan/todo/0002-terminal-ui-foundation.md` | Split PR and AI review panes, added scroll support, and introduced TUI accent styling. |
| 2026-07-23 | `83fa971` | `codex/tui-roadmap` | `.docflow/plan/todo/0002-terminal-ui-foundation.md` | Added PR-list AI review markers, narrowed the repository pane, and made wrapped AI review output scroll vertically. |
| 2026-07-23 | `458d2d3` | `codex/tui-roadmap` | `.docflow/plan/todo/0002-terminal-ui-foundation.md` | Added terminal PR-list running markers for active AI reviews and OSC52 clipboard copy for full saved review output. |
| 2026-07-23 | `1de1eba` | `codex/tui-roadmap` | `.docflow/plan/todo/0002-terminal-ui-foundation.md` | Allowed AI reviews to run in parallel across different PRs while retaining the per-PR duplicate-run guard. |
| 2026-07-23 | `cbd4719` | `codex/tui-roadmap` | `.docflow/plan/todo/0002-terminal-ui-foundation.md` | Suppressed optional unavailable skill/tool notes in AI review output unless they affect confidence. |
| 2026-07-23 | `d9c1b0d` | `codex/tui-roadmap` | `.docflow/plan/todo/0002-terminal-ui-foundation.md` | Added terminal PR author metadata and open/draft/merged filters. |
| 2026-07-23 | `3f65b76` | `codex/tui-roadmap` | `.docflow/plan/todo/0002-terminal-ui-foundation.md` | Moved terminal PR filtering into a visible mouse-clickable panel below repositories. |
| 2026-07-23 | `4cf30c4` | `codex/tui-roadmap` | `.docflow/plan/todo/0002-terminal-ui-foundation.md` | Opened lazygit from the TUI on the selected pull request source branch. |
| 2026-07-24 | `364f5b8` | `codex/tui-roadmap` | `.docflow/plan/todo/0002-terminal-ui-foundation.md` | Let Git attempt lazygit branch checkout when local changes do not conflict. |
| 2026-07-24 | `5e50f8e` | `codex/tui-roadmap` | `.docflow/plan/todo/0002-terminal-ui-foundation.md` | Replaced external lazygit launch with a native terminal Git view backed by local repository commands. |
| 2026-07-24 | `f4291a6` | `codex/tui-roadmap` | `.docflow/plan/todo/0002-terminal-ui-foundation.md` | Changed the terminal side pane to show the provider pull request diff without checking out branches or touching local worktrees. |
| 2026-07-24 | `339381b` | `codex/tui-roadmap` | `.docflow/plan/todo/0002-terminal-ui-foundation.md` | Reworked terminal diff review into a dedicated full-page view with a file list and per-file diff content. |
| 2026-07-24 | `3a53457` | `codex/tui-roadmap` | `.docflow/plan/todo/0002-terminal-ui-foundation.md` | Added a terminal diff display mode toggle between unified and side-by-side file views. |
| 2026-07-24 | `5e0993f` | `codex/tui-roadmap` | `.docflow/plan/todo/0002-terminal-ui-foundation.md` | Rendered terminal PR diffs through the delta pager to match the local lazygit diff style. |
| 2026-07-24 | `31e70e3` | `main` | `.docflow/plan/todo/0002-terminal-ui-foundation.md` | Added release build and local `lac` install recipes for the terminal UI. |
| 2026-07-27 | `7b35c16` | `main` | `.docflow/plan/done/2026-07-27-headless-review-cli.md` | Shipped headless review through PR #115; typecheck, frontend/Rust tests, build, Tauri smoke, Archgate, and Norn review completed. |
| 2026-07-27 | `7f95a9a` | `agent/shared-service-boundary` | `.docflow/plan/todo/0004-shared-review-service-program.md` | Accepted the shared-service trust boundary for issue #89; typecheck, frontend tests, Archgate, docflow audit, and Norn review passed. |
| 2026-07-27 | `5659ba3` | `agent/review-event-contract` | `.docflow/plan/todo/0004-shared-review-service-program.md` | Added the provider-neutral pull-request review event contract for issue #90; Rust/frontend tests, typecheck, Archgate, and Norn review passed. |
| 2026-07-27 | `852c9c5` | `agent/review-event-contract` | `.docflow/plan/todo/0004-shared-review-service-program.md` | Separated delivery replay and stable review idempotency keys after Norn review of issue #90. |
| 2026-07-27 | `58f9bd4` | `agent/review-cursor-storage` | `.docflow/plan/todo/0004-shared-review-service-program.md` | Added tenant-scoped last-reviewed-head cursor persistence for issue #91 with v1 migration and stale-head concurrency coverage. |
| 2026-07-27 | `c3b8534` | `agent/review-cursor-storage` | `.docflow/plan/todo/0004-shared-review-service-program.md` | Hardened issue #91 cursor advancement with an immediate SQLite compare-and-swap and current-head gate after Norn review. |
| 2026-07-27 | `5664bd5` | `agent/incremental-review-scope` | `.docflow/plan/todo/0004-shared-review-service-program.md` | Added read-only incremental commit scope construction for issue #92 with structured Git metadata, setup errors, and repository fixtures. |
| 2026-07-27 | `fd2a2d3` | `agent/incremental-review-scope` | `.docflow/plan/todo/0004-shared-review-service-program.md` | Hardened issue #92 Git record pairing, non-UTF8 handling, and bounded patch streaming after Norn review. |
| 2026-07-27 | `32cc75f` | `agent/incremental-review-scope` | `.docflow/plan/todo/0004-shared-review-service-program.md` | Completed issue #92 hardening with ordered Git record pairing, bounded metadata reads, and deterministic color-free output. |
| 2026-07-27 | `4255131` | `agent/incremental-review-scope` | `.docflow/plan/todo/0004-shared-review-service-program.md` | Validated incremental commit IDs against repository SHA-1/SHA-256 formats with a legacy Git compatibility fallback. |
| 2026-07-27 | `2f421a6` | `agent/incremental-review-scope` | `.docflow/plan/todo/0004-shared-review-service-program.md` | Removed diff-order coupling by matching stats with rename-aware path keys and canonicalized commit object IDs. |
| 2026-07-27 | `5a2a2ec` | `agent/incremental-review-scope` | `.docflow/plan/todo/0004-shared-review-service-program.md` | Rejected unrelated commit ranges with a structured setup error so incremental scope cannot silently cross histories. |
| 2026-07-27 | `d7e25e3` | `agent/incremental-review-scope` | `.docflow/plan/todo/0004-shared-review-service-program.md` | Required direct commit objects and completed legacy Git object-format fallback after Norn review. |
| 2026-07-27 | `083cdd5` | `agent/reviewer-feedback-events` | `.docflow/plan/todo/0004-shared-review-service-program.md` | Added provider-neutral append-only reviewer feedback, deterministic dispositions, tenant-scoped SQLite persistence, and idempotent delivery for issue #93. |
| 2026-07-27 | `284f169` | `agent/reviewer-feedback-events` | `.docflow/plan/todo/0004-shared-review-service-program.md` | Hardened issue #93 with an atomic self-repairing v3 migration and canonical identifier validation after Norn review. |
| 2026-07-27 | `6134af3` | `agent/reviewer-feedback-events` | `.docflow/plan/todo/0004-shared-review-service-program.md` | Canonicalized persisted optional feedback, bounded event timestamps, and added contextual SQLite conversion errors after final review. |
| 2026-07-27 | `ff9bea6` | `agent/administrative-audit-events` | `.docflow/plan/todo/0004-shared-review-service-program.md` | Added redacted append-only administrative audit events, content-free idempotency receipts, streaming JSONL export, and controlled retention purge support for issue #94. |
| 2026-07-27 | `88b7872` | `agent/administrative-audit-events` | `.docflow/plan/todo/0004-shared-review-service-program.md` | Avoided cloning unbounded sensitive audit values before redaction after the pre-push Norn review. |
| 2026-07-27 | `bf70442` | `agent/administrative-audit-events` | `.docflow/plan/todo/0004-shared-review-service-program.md` | Preserved delivery idempotency across payload retention, canonicalized audit timestamps, and kept the public export path streaming after final review. |
| 2026-07-27 | `6dd5204` | `agent/provider-webhook-ingress` | `.docflow/plan/todo/0004-shared-review-service-program.md` | Added authenticated GitHub and Bitbucket pull-request webhook ingress with provider-neutral normalization, atomic durable acceptance, delivery deduplication, and fork-aware commit resolution for issue #95. |
| 2026-07-27 | `fbbd332` | `agent/provider-webhook-ingress` | `.docflow/plan/todo/0004-shared-review-service-program.md` | Resynchronized GitHub draft conversions and base-branch edits while ignoring metadata-only edits after the pre-push Norn review. |
| 2026-07-27 | `3a51207` | `agent/provider-webhook-ingress` | `.docflow/plan/todo/0004-shared-review-service-program.md` | Suppressed unchanged Bitbucket PR state before review enqueueing and added privacy-safe actor identity fallbacks for the bounded Lachesi rerun findings. |
| 2026-07-27 | `e136bc8` | `agent/headless-review-jobs` | `.docflow/plan/todo/0004-shared-review-service-program.md` | Added durable provider-triggered review jobs with cursor-aware scope, tenant-scoped deduplication, bounded claims, lease heartbeat and fencing, stale-event suppression, and atomic completion for issue #96. |
| 2026-07-27 | `b4d5a6a` | `agent/headless-review-jobs` | `.docflow/plan/todo/0004-shared-review-service-program.md` | Added provider-time freshness fencing and base-plus-head review identity so delayed webhooks cannot regress current work and base retargets trigger a new review. |
| 2026-07-27 | `35f914a` | `agent/headless-review-jobs` | `.docflow/plan/todo/0004-shared-review-service-program.md` | Preserved ambiguous same-timestamp revision events and bounded expired-lease recovery before terminal failure after final branch review. |
| 2026-07-27 | `5f3f42d` | `agent/headless-review-jobs` | `.docflow/plan/todo/0004-shared-review-service-program.md` | Made review cursors base-aware and isolated equal-timestamp revision candidates so ambiguous events cannot overwrite state or cancel one another. |
| 2026-07-27 | `e0421a7` | `agent/headless-review-jobs` | `.docflow/plan/todo/0004-shared-review-service-program.md` | Preserved legacy v1 event decoding, allowed current full-branch completion, and persisted ambiguity so conflicting timestamp peers cannot advance before later confirmation. |
| 2026-07-27 | `d1dedd8` | `agent/headless-review-jobs` | `.docflow/plan/todo/0004-shared-review-service-program.md` | Preserved completed execution results through transient heartbeat renewal errors while retaining finish-time lease fencing. |
| 2026-07-27 | `78ef4ff` | `agent/provider-inline-publication` | `.docflow/plan/todo/0004-shared-review-service-program.md` | Added explicit structured finding publication with tenant/head-scoped markers, durable fenced idempotency, stale-anchor rejection, and GitHub/Bitbucket inline adapters for issue #97. |
| 2026-07-27 | `594449d` | `agent/provider-inline-publication` | `.docflow/plan/todo/0004-shared-review-service-program.md` | Canonicalized publication identity, isolated dry-run state, made permission failures terminal, and required provider responses to preserve requested inline anchors after Norn review. |
| 2026-07-27 | `f435e04` | `agent/provider-inline-publication` | `.docflow/plan/todo/0004-shared-review-service-program.md` | Added post-write head fencing, compensating comment deletion, exact marker-and-anchor recovery, and canonical repository identity after final branch review. |
| 2026-07-27 | `0513e2b` | `agent/provider-inline-publication` | `.docflow/plan/todo/0004-shared-review-service-program.md` | Preserved Bitbucket inline ranges during marker recovery and removed newly created provider comments when durable completion fails after the final bounded review. |
| 2026-07-27 | `dfd1072` | `agent/provider-inline-publication` | `.docflow/plan/todo/0004-shared-review-service-program.md` | Routed GUI finding drafts through durable structured publication, pinned reviewed heads, recovered failed cleanup, encoded provider paths, and kept GitHub throttling retryable after branch review. |
| 2026-07-27 | `c5a36fe` | `agent/provider-inline-publication` | `.docflow/plan/todo/0004-shared-review-service-program.md` | Preserved canonical multi-line finding ranges and removed marker-only provider orphans before replacement writes after the bounded branch review. |
| 2026-07-27 | `27664bb` | `agent/provider-inline-publication` | `.docflow/plan/todo/0004-shared-review-service-program.md` | Persisted immutable reviewed heads across GUI, TUI, and headless runs and separated non-idempotent manual writes from structured finding tracking after branch review. |
| 2026-07-27 | `3a97965` | `agent/provider-inline-publication` | `.docflow/plan/todo/0004-shared-review-service-program.md` | Pinned direct menu-bar review starts to their loaded provider head and added a command-argument regression test after final branch review. |
| 2026-07-27 | `35ab941` | `agent/provider-inline-publication` | `.docflow/plan/todo/0004-shared-review-service-program.md` | Prevented untracked structured provider writes and aligned the browser mock with backend validation, stale-head fencing, and idempotent publication retries after final branch review. |
| 2026-07-27 | `70bbdb2` | `agent/provider-inline-publication` | `.docflow/plan/todo/0004-shared-review-service-program.md` | Restricted Bitbucket publication pagination to the exact HTTPS comments endpoint so provider credentials cannot follow an untrusted next-page URL. |
| 2026-07-27 | `8ea0a03` | `agent/provider-inline-publication` | `.docflow/plan/todo/0004-shared-review-service-program.md` | Required exact structured finding anchor matches and rejected oversized composed provider markdown before reserving publication state after final branch review. |
| 2026-07-27 | `d4856d1` | `agent/provider-inline-publication` | `.docflow/plan/todo/0004-shared-review-service-program.md` | Made structured Bitbucket comment creation at-most-once across transient errors and moved the publication IPC contract into the typed review service. |
| 2026-07-27 | `d17184f` | `agent/provider-inline-publication` | `.docflow/plan/todo/0004-shared-review-service-program.md` | Bound provider targets and reviewed heads to authoritative review runs, normalized structured IPC errors, and required returned tracking state to prove publication events were applied. |
| 2026-07-27 | `68ef92f` | `agent/provider-inline-publication` | `.docflow/plan/todo/0004-shared-review-service-program.md` | Restricted marker recovery to the authenticated provider author, kept GitHub abuse throttling retryable, and verified pull request snapshots around diff construction. |
| 2026-07-27 | `f949415` | `agent/provider-inline-publication` | `.docflow/plan/todo/0004-shared-review-service-program.md` | Built manual reviews from provider-verified snapshots, carried their immutable heads through execution, and rejected stale focused-line questions. |
| 2026-07-27 | `bb2427e` | `agent/provider-inline-publication` | `.docflow/plan/todo/0004-shared-review-service-program.md` | Rebuilt focused line-question hunks from the verified provider snapshot so unchanged anchors cannot carry stale surrounding context. |
| 2026-07-27 | `5694212` | `agent/provider-inline-publication` | `.docflow/plan/todo/0004-shared-review-service-program.md` | Required stable provider head, base, and branches around native diff loading for both headless PR and terminal review starts. |
| 2026-07-27 | `8badb79` | `agent/provider-inline-publication` | `.docflow/plan/todo/0004-shared-review-service-program.md` | Required both provider head and base commit identities in frontend review snapshots to fail closed when drift cannot be detected. |
| 2026-07-27 | `33e7281` | `agent/provider-inline-publication` | `.docflow/plan/todo/0004-shared-review-service-program.md` | Fenced structured publication by the complete reviewed base-plus-head revision and post-checked both newly created and recovered provider comments. |
| 2026-07-28 | `fd04644` | `agent/provider-inline-publication` | `.docflow/plan/todo/0004-shared-review-service-program.md` | Preserved migrated v6 publication idempotency through legacy marker recovery and made revision validation errors identify the rejected commit field. |
| 2026-07-28 | `f2f7381` | `agent/provider-inline-publication` | `.docflow/plan/todo/0004-shared-review-service-program.md` | Pinned finding staging to a freshly verified reviewed diff and consolidated the unreleased publication schema into base-aware migration v6 without ambiguous legacy rows. |
| 2026-07-28 | `3ec085b` | `agent/provider-inline-publication` | `.docflow/plan/todo/0004-shared-review-service-program.md` | Matched GitHub old-side structured comments through original line fields with current-line fallback and regression coverage. |
| 2026-07-28 | `93133a8` | `agent/provider-inline-publication` | `.docflow/plan/todo/0004-shared-review-service-program.md` | Preserved provider comments for idempotent durable-state recovery, serialized typed publication errors across Tauri IPC, and kept browser mock review revisions aligned with native runs. |
| 2026-07-28 | `97c136e` | `agent/provider-inline-publication` | `.docflow/plan/todo/0004-shared-review-service-program.md` | Refreshed provider snapshots before structured publication and passed current active review references directly into review payloads after Lachesi branch review. |
| 2026-07-28 | `0ce2a87` | `agent/provider-inline-publication` | `.docflow/plan/todo/0004-shared-review-service-program.md` | Preserved large and opaque provider comment identifiers across Rust, Tauri IPC, GUI, TUI, browser mocks, and persisted reply drafts after Lachesi branch review. |
| 2026-07-28 | `ad06120` | `agent/reconcile-provider-comments` | `.docflow/plan/todo/0004-shared-review-service-program.md` | Added provider-neutral, retry-safe finding reconciliation with stable lineage, in-place update/resolve behavior, legacy marker compatibility, provider adapters, and auditable partial-failure summaries for issue #98; 282 Rust tests, 78 frontend tests, typecheck, build, Clippy, and 17 Archgate checks passed. |
| 2026-07-28 | `5874a60` | `agent/reconcile-provider-comments` | `.docflow/plan/todo/0004-shared-review-service-program.md` | Wired reconciliation through Tauri IPC, typed frontend service, GUI and browser mock; verified GitHub PR ownership, allowed historical findings to reconcile, and surfaced missing tracked comments after Norn review; 285 Rust tests, 80 frontend tests, typecheck, build, Clippy, and 17 Archgate checks passed. |
| 2026-07-28 | `6c4df60` | `agent/reconcile-provider-comments` | `.docflow/plan/todo/0004-shared-review-service-program.md` | Reconciled explicitly staged finding batches at PR-review scope, resolved only history absent from the active review, preserved current unselected findings, and reported uncertain provider mutations after the bounded Lachesi rerun; 286 Rust tests, 82 frontend tests, typecheck, build, Clippy, and 17 Archgate checks passed. |
| 2026-07-28 | `bfa1f1e` | `agent/reconcile-provider-comments` | `.docflow/plan/todo/0004-shared-review-service-program.md` | Kept reconciliation dry-run free of provider writes and preserved successful provider comments and local publication events across partial batch failures after final Norn review; 289 Rust tests, 83 frontend tests, typecheck, build, Clippy, and 17 Archgate checks passed. |
| 2026-07-28 | `2030561` | `agent/reconcile-provider-comments` | `.docflow/plan/todo/0004-shared-review-service-program.md` | Reconciled persisted drafts across review runs by reviewed revision, isolated stale draft failures, resolved tracked comments without provider anchor metadata, and bounded provider work after branch review; 290 Rust tests, 84 frontend tests, typecheck, build, Clippy, and 17 Archgate checks passed. |
| 2026-07-28 | `f729532` | `agent/reconcile-provider-comments` | `.docflow/plan/todo/0004-shared-review-service-program.md` | Preserved stale-revision classification when rollback fails and cached provider clients and authenticated identity across reconciliation actions after the bounded branch review; 290 Rust tests, 84 frontend tests, typecheck, build, Clippy, and 17 Archgate checks passed. |
| 2026-07-28 | `81516bc` | `agent/reconcile-provider-comments` | `.docflow/plan/todo/0004-shared-review-service-program.md` | Resolved old anchors before replacement publication, restricted reconciliation tracking to inline comments, and required durable target/fingerprint/comment ownership for legacy markers after branch review; 292 Rust tests, 85 frontend tests, typecheck, build, Clippy, and 17 Archgate checks passed. |
| 2026-07-28 | `aa2bdd3` | `agent/signed-organization-policy` | `.docflow/plan/todo/0004-shared-review-service-program.md` | Added signed, versioned organization policy with deterministic precedence, verified bounded cache fallback and recovery, fail-closed trust boundaries, execution-time enforcement, audit metadata, and GUI/TUI/headless integration for issue #99; 322 Rust tests, 85 frontend tests, typecheck, build, and 17 Archgate checks passed. |
| 2026-07-28 | `f5ef86d` | `agent/signed-organization-policy` | `.docflow/plan/todo/0004-shared-review-service-program.md` | Closed signed-policy trust-boundary findings across enforced profiles, analyzer provenance, cache finalization, local overrides, prompt enforcement, and repository config loading; 336 Rust tests, 85 frontend tests, typecheck, build, 17 Archgate checks, and the final Lachesi branch review passed with no findings. |
| 2026-07-28 | `af30eb3` | `agent/tui-image-diff` | `.docflow/plan/todo/0005-terminal-image-diffs.md` | Accepted bounded PNG, JPEG, GIF, and WebP rendering through Kitty, Sixel, and iTerm2 with deterministic metadata fallback for issue #127. |
| 2026-07-28 | `9b7dc23` | `agent/tui-image-diff` | `.docflow/plan/todo/0005-terminal-image-diffs.md` | Added bounded provider image reads, raster validation and decoding, terminal protocol rendering, base/changed toggling, metadata fallbacks, documentation, and non-interactive coverage for issue #127; 349 Rust tests, 85 frontend tests, typecheck, build, and 17 Archgate checks passed. |
| 2026-07-28 | `a49bd9e` | `agent/tui-image-diff` | `.docflow/plan/todo/0005-terminal-image-diffs.md` | Handled Git-quoted image paths and cross-type renames after Norn review, then removed the residual eager preview fetch so provider image reads occur only in the active diff view; 350 Rust tests and 17 Archgate checks passed before the focused TUI gate. |
| 2026-07-28 | `f50e5c8` | `main` | `.docflow/plan/done/2026-07-28-terminal-image-diffs.md` | Shipped terminal image diff rendering through PR #129 and closed issue #127; 350 Rust tests, 85 frontend tests, typecheck, build, and 17 Archgate checks passed before merge. |
| 2026-07-28 | `e2cf2f6` | `agent/review-effectiveness-metrics` | `.docflow/plan/todo/0004-shared-review-service-program.md` | Added deterministic tenant-scoped review effectiveness aggregation, stable JSON and human CLI reports, documented feedback denominators and latency semantics, storage migration, and zero/partial/multi-repository fixtures for issue #100; 359 Rust tests, 85 frontend tests, typecheck, build, and 17 Archgate checks passed. |
| 2026-07-28 | `b9aa31f` | `agent/review-effectiveness-metrics` | `.docflow/plan/todo/0004-shared-review-service-program.md` | Made review-store identities tenant-scoped, isolated local load/delete/cleanup paths, skipped id-less legacy runs before modern decoding, scoped validation to requested filters, and rejected duplicate finding fingerprints after Norn review; 360 Rust tests, 85 frontend tests, typecheck, build, and 17 Archgate checks passed. |
| 2026-07-28 | `bbc1734` | `agent/review-effectiveness-metrics` | `.docflow/plan/todo/0004-shared-review-service-program.md` | Applied provider/workspace/repository filters before stored-run and feedback decoding and ignored valid idle/running runs after the bounded Lachesi rerun; storage-level malformed-data fixtures, 361 Rust tests, 85 frontend tests, typecheck, and 17 Archgate checks passed. |
| 2026-07-28 | `ea64834` | `agent/review-effectiveness-dashboard` | `.docflow/plan/todo/0004-shared-review-service-program.md` | Added the review effectiveness dashboard with repository and rolling time filters, contract-backed summary and breakdown charts, explicit incomplete-feedback states, median first-review latency, Tauri and browser-mock parity, and accessible component coverage for issue #101; 363 Rust tests, 90 frontend tests, typecheck, build, visual desktop/mobile checks, and 17 Archgate checks passed. |
| 2026-07-28 | `26c733f` | `agent/review-effectiveness-dashboard` | `.docflow/plan/todo/0004-shared-review-service-program.md` | Returned empty browser-mock metrics for unmatched repository scopes and allowed Overview controls to wrap on narrow windows after the bounded Norn review; the 14 affected frontend tests and typecheck passed before the final rerun. |
| 2026-07-28 | `eb14789` | `agent/review-effectiveness-dashboard` | `.docflow/plan/todo/0004-shared-review-service-program.md` | Moved metrics IPC behind the typed frontend service, cleared disabled in-flight requests, and made seven-day browser metrics distinct after residual review findings; 96 frontend tests, build, visual range checks, Archgate, and the focused Lachesi working-tree gate passed with only two non-blocking low findings. |
| 2026-07-28 | `4631417` | `agent/team-role-authorization` | `.docflow/plan/todo/0004-shared-review-service-program.md` | Added provider-neutral team identity, explicit admin/member/viewer/service-account authorization, organization/team/repository scope enforcement, redacted denial audit, and IdP-free local mode for issue #102; 372 Rust tests, 96 frontend tests, typecheck, Clippy, and 17 Archgate checks passed. |
| 2026-07-28 | `16b958a` | `agent/team-role-authorization` | `.docflow/plan/todo/0004-shared-review-service-program.md` | Rejected ambiguous repository scope on organization operations, omitted it from organization denial audits, audited unknown service-account roles, and validated authorization timestamps against the shared audit bound after Norn review; 375 Rust tests and 17 Archgate checks passed. |
| 2026-07-28 | `66be183` | `agent/team-role-authorization` | `.docflow/plan/todo/0004-shared-review-service-program.md` | Preserved repository scope on forward-compatible unknown operations so they fail closed through an audited `unknown_operation` decision instead of bypassing denial audit; all 12 authorization tests passed. |
| 2026-07-28 | `43aecdf` | `agent/team-role-authorization` | `.docflow/plan/todo/0004-shared-review-service-program.md` | Enforced scope consistency before unknown-role/operation denial, omitted mismatched repository data from audit, and introduced repository-optional audit schema v2 while preserving the stable required-repository v1 contract; 377 Rust tests and 17 Archgate checks passed. |
| 2026-07-28 | `2d9a70e` | `agent/team-role-authorization` | `.docflow/plan/todo/0004-shared-review-service-program.md` | Separated the non-secret operation request from its trusted non-deserializable actor principal, rejected ambiguous team-only metrics, validated derived audit delivery ids, and restricted organization-scoped audit v2 to authorization denials after final branch review; 379 Rust tests and 17 Archgate checks passed. |
| 2026-07-28 | `d8a2f66` | `agent/team-role-authorization` | `.docflow/plan/todo/0004-shared-review-service-program.md` | Bound denial audit tenant and repository metadata to the authenticated principal, enforced denied outcomes for authorization audit events, and classified either missing repository scope component consistently after the pre-push review; 380 Rust tests and 17 Archgate checks passed. |
| 2026-07-28 | `f09aa7f` | `agent/team-role-authorization` | `.docflow/plan/todo/0004-shared-review-service-program.md` | Moved audit idempotency and correlation metadata behind a trusted non-deserializable ingress context, bounded correlation values before authorization, and made v1 repository presence fail during decoding after the pre-push review; 381 Rust tests and 17 Archgate checks passed. |
| 2026-07-28 | `bc14375` | `agent/team-role-authorization` | `.docflow/plan/todo/0004-shared-review-service-program.md` | Centralized audit wire-shape validation across serialization, deserialization, and storage so v1 always carries repository scope and authorization denials exist only as coherent v2 events after the pre-push review; 381 Rust tests and 17 Archgate checks passed. |
| 2026-07-28 | `2f16701` | `agent/team-role-authorization` | `.docflow/plan/todo/0004-shared-review-service-program.md` | Separated untrusted provider repository targets from trusted enrollment-resolved tenant/team ownership, denied missing or mismatched enrollment, and emitted repository audit metadata only for exact trusted matches after pre-push review; 383 Rust tests and 17 Archgate checks passed. |
| 2026-07-29 | `3a3f469` | `agent/generic-oidc-authentication` | `.docflow/plan/todo/0004-shared-review-service-program.md` | Added generic OIDC Authorization Code plus PKCE login, discovery/JWKS/claim validation, group-to-role mapping, opaque browser-bound sessions, and Codex runner isolation for issue #103; 396 Rust tests, 96 frontend tests, typecheck, 17 Archgate checks, and Norn review passed. |
| 2026-07-29 | `d8da28a` | `agent/self-hosted-service-packaging` | `.docflow/plan/todo/0004-shared-review-service-program.md` | Added the self-hosted service runtime, persistent-volume migration boundary, bounded HTTP health/readiness endpoints, offline smoke and restart coverage, Docker/Compose artifacts, and deployment documentation for issue #104; 404 Rust tests, 96 frontend tests, typecheck, and 17 Archgate checks passed. |
| 2026-07-29 | `a8b1e5e` | `agent/review-quality-evaluation-corpus` | `.docflow/plan/todo/0004-shared-review-service-program.md` | Added the versioned, sanitized offline review-quality corpus, JSON runner, explicit regression baseline, provenance and duration metrics, task-runner gate, and documentation for issue #105; 408 Rust tests, 96 frontend tests, typecheck, and 17 Archgate checks passed. |
| 2026-07-29 | `465bc99` | `agent/github-app-onboarding` | `.docflow/plan/todo/0004-shared-review-service-program.md` | Added least-privilege GitHub App onboarding contracts, explicit repository enrollment, webhook configuration port, durable identifier-only SQLite storage, lifecycle revocation, and administrator documentation for issue #106; 413 Rust tests, 96 frontend tests, typecheck, and 17 Archgate checks passed. |
| 2026-07-29 | `30357ba` | `agent/bitbucket-oauth-onboarding` | `.docflow/plan/todo/0004-shared-review-service-program.md` | Added Bitbucket Cloud OAuth state/callback validation, explicit repository enrollment, webhook validation port, durable identifier-only SQLite storage, revocation status, and minimal-scope documentation for issue #107; 416 Rust tests, 96 frontend tests, typecheck, and 17 Archgate checks passed. |
| 2026-07-29 | `a68dd66` | `agent/credential-broker` | `.docflow/plan/todo/0004-shared-review-service-program.md` | Added AES-256-GCM encrypted team credential storage with external master keys, opaque references, versioned rotation, durable revocation, authorized resolution, redacted error surfaces, SQLite persistence, and lifecycle documentation for issue #108; 419 Rust tests, 96 frontend tests, typecheck, and 17 Archgate checks passed. |
| 2026-07-29 | `f8e2978` | `agent/retention-deletion` | `.docflow/plan/todo/0004-shared-review-service-program.md` | Added deterministic bounded retry with jitter, durable retry scheduling, SQLite dead-letter state and migration, lease recovery through the same policy, idempotent delivery/head handling, and append-only manual-retry history for issue #109; 422 Rust tests, 96 frontend tests, typecheck, and 17 Archgate checks passed. |
| 2026-07-29 | `84682d3` | `agent/operational-telemetry` | `.docflow/plan/todo/0004-shared-review-service-program.md` | Added bounded provider-neutral service telemetry with opaque job correlation, redacted counters and histograms, webhook and durable-job instrumentation, Prometheus `/metrics`, self-hosted documentation, and synthetic failure coverage for issue #110; 426 Rust tests, 96 frontend tests, typecheck, and 17 Archgate checks passed. |
| 2026-07-29 | `4b1ffc0` | `agent/self-hosted-backup-restore` | `.docflow/plan/todo/0004-shared-review-service-program.md` | Added consistent SQLite snapshot backup, checksum manifest, integrity and schema validation, empty-target restore, self-hosted backup/restore commands, and ciphertext/key-dependency documentation for issue #111; 427 Rust tests, 96 frontend tests, typecheck, and 17 Archgate checks passed. |
| 2026-07-29 | `9cf5df7` | `agent/tenant-isolation-conformance` | `.docflow/plan/todo/0004-shared-review-service-program.md` | Added a two-organization conformance fixture with intentionally shared provider, repository, pull-request, delivery, finding, and credential identifiers; it proves durable jobs, publications, and credentials remain tenant-isolated for issue #113; 429 Rust tests, 96 frontend tests, typecheck, and 17 Archgate checks passed. |
| 2026-07-29 | `1e73bf5` | `agent/managed-service-slos` | `.docflow/plan/todo/0004-shared-review-service-program.md` | Defined the pilot managed-service operating model, selected-region data-location boundary, measurable SLOs, external-provider classification, backup/restore reference, ownership, status communication, and paid-support distinctions for issue #114; 96 frontend tests, typecheck, and 17 Archgate checks passed. |
| 2026-07-29 | `bd27a30` | `agent/retention-deletion-controls` | `.docflow/plan/todo/0004-shared-review-service-program.md` | Added repository-scoped dry-run and execution retention controls, durable success/failure audit records, transactional disconnected-repository deletion, documented defaults, and preservation of non-identifying metrics for issue #112; 428 Rust tests, 96 frontend tests, typecheck, and 17 Archgate checks passed. |
| 2026-07-30 | `1bfea2b` | `feat/tui-progressive-loading` | `.docflow/plan/todo/0002-terminal-ui-foundation.md` | Opened the TUI before repository/provider work, added independent loading and failure states for repositories, pull requests, details, comments, diffs, and AI review, discarded stale worker results, and preserved same-target data during refresh; 435 Rust tests, 96 frontend tests, typecheck, and 17 Archgate checks passed. |
| 2026-07-30 | `72cf541` | `feat/tui-progressive-loading` | `.docflow/plan/todo/0002-terminal-ui-foundation.md` | Compacted long diff paths from the start while preserving filenames, nearby directories, change counts, exact-fit boundaries, and Unicode grapheme clusters; 440 Rust tests, 17 Archgate checks, and the bounded Norn review passed with no findings. |
| 2026-07-30 | `37a6c37` | `feat/tui-progressive-loading` | `.docflow/plan/todo/0002-terminal-ui-foundation.md` | Cleared stale PR lists and markers on repository/filter changes, required the current list to be ready, deduplicated same-target resource loads, and kept switching to a different in-flight PR possible through request fencing; 444 Rust tests and 17 Archgate checks passed. |
| 2026-07-30 | `3bdf90f` | `feat/tui-progressive-loading` | `.docflow/plan/todo/0002-terminal-ui-foundation.md` | Blocked draft publication until the comment snapshot completes and applied asynchronous review-marker snapshots while preserving only PR IDs changed locally after each scan began; 446 Rust tests and 17 Archgate checks passed. |
| 2026-07-30 | `fa13281` | `main` | `.docflow/plan/done/2026-07-30-terminal-ui-foundation.md` | Shipped the terminal UI foundation through PR #146 with progressive loading, stale-result fencing, readable long paths, staged review workflows, native diff views, and analyzer-free AI review; 446 Rust tests, 96 frontend tests, typecheck, and 17 Archgate checks passed. |
| 2026-08-04 | `e149569` | `main` | `.docflow/plan/done/2026-07-31-side-by-side-terminal-image-diffs.md` | Recorded the already-shipped PR #148 completion event; current verification passed 459 Rust tests, 99 frontend tests, typecheck, evaluation, and 17 Archgate checks. |
| 2026-08-04 | `79a90ec` | `main` | `.docflow/plan/done/2026-08-04-norn-naming-and-compatibility.md` | Shipped the Norn naming and compatibility contract through PR #195 and closed issue #173; ADR 0011 remains Accepted while the dependent migration plans are still in progress. |
| 2026-08-05 | `98de442` | `codex/norn-onboarding-foundation` | `.docflow/plan/todo/0009-norn-onboarding-contract.md` | Moved onboarding migration queue items `0006`-`0008` to done and added ADR `0012` and spec `0011` for issue #186. |
| 2026-08-05 | `51546d4` | `codex/norn-onboarding-foundation` | `.docflow/plan/todo/0010-norn-readiness-probe.md` | Added ADR `0013`, spec `0012`, and plan for issue #187 (read-only readiness probe). |
| 2026-08-05 | `3887756` | `codex/norn-onboarding-foundation` | `.docflow/plan/done/2026-08-05-norn-onboarding-contract.md` | Closed issue #186 onboarding contract by adding final guided-init safety regression coverage, requiring `--yes` for guided init execution, and moving issue #186 from plan/todo to plan/done. |
| 2026-08-05 | `aa1c833` | `docs/readme-refresh` | `.docflow/_agent/CURRENT_FOCUS.md`, `src-tauri/src/readiness.rs` | Added legacy-name regression gating to `norn doctor` for issue #179 and added explicit repository tests for legacy artifact allowlisting; closed #179 from this slice. |
| 2026-08-31 | `65a86bc` | `codex/release-install-upgrade-hardening` | `.docflow/plan/todo/0005-codex-sandbox-permission-diagnostics.md`, `.docflow/plan/todo/0006-release-installation-upgrade-hardening.md` | Implemented issues #198-#204 with actionable sandbox diagnostics, reproducible signed release artifacts, verified Homebrew formula/cask lifecycle gates, durable local installation, and explicit GUI-only desktop versus command-distribution routing; frontend, Rust, tooling, Tauri build, Archgate, and bounded Norn gates completed. |
| 2026-08-31 | `643ff6f` | `main` | `.docflow/plan/done/2026-08-31-codex-sandbox-permission-diagnostics.md`, `.docflow/plan/done/2026-08-31-release-installation-upgrade-hardening.md` | Shipped issues #198-#204 through PR #205 after typecheck, 104 frontend tests plus tooling suites, Archgate 17/17, and pre-push Norn review; ADR 0014 advanced to Implemented. |
| 2026-08-31 | `24d9ce1` | `main` | `.docflow/plan/done/2026-08-31-cli-only-release-channel.md` | Shipped the command-only release channel through PR #208: CLI/TUI archives and the Homebrew formula no longer require Apple credentials, while desktop and cask delivery remain explicitly enabled, signed, notarized, and fail-closed; typecheck, 104 frontend tests plus tooling suites, Tauri IPC smoke, build, Archgate 17/17, and bounded Norn review completed. |
| 2026-09-01 | `7e9c759` | `main` | `.docflow/plan/done/2026-09-01-homebrew-formula-temporary-tap.md` | Shipped issue #213 through PR #214: Formula lifecycle validation now uses an isolated temporary tap, explicit candidate tags, and schema-checked readiness outcomes; typecheck, 104 frontend tests plus tooling suites, Archgate 17/17, and clean-runner Intel and Apple Silicon lifecycle run `33444357192` passed. |
| 2026-09-01 | `bd01a2f..60aa484` | `feat/terminal-onboarding-skills-settings` | `.docflow/plan/todo/0002-secure-terminal-credential-onboarding.md`, `.docflow/plan/todo/0003-managed-agent-review-skills.md`, `.docflow/plan/todo/0005-complete-tui-settings.md` | Accepted the managed Codex and Claude Code skill distribution decision and queued issues #216-#218 in implementation order; typecheck, 104 frontend tests plus tooling suites, Archgate 17/17, and bounded Norn review completed. |
| 2026-09-01 | `047779a..bba6dbf` | `feat/terminal-onboarding-skills-settings-impl` | `.docflow/plan/todo/0002-secure-terminal-credential-onboarding.md`, `.docflow/plan/todo/0003-managed-agent-review-skills.md`, `.docflow/plan/todo/0005-complete-tui-settings.md` | Implemented issues #216-#218 with keychain-backed terminal auth, versioned managed skills for Codex and Claude Code, responsive complete TUI settings, sanitized public documentation, and Formula lifecycle coverage; 541 Rust tests, 104 frontend tests plus tooling, Tauri IPC, Clippy, build, Archgate 17/17, temporary-home skill lifecycle, and bounded Norn review completed. |
| 2026-09-01 | `9179adc` | `main` | `.docflow/plan/done/2026-09-01-secure-terminal-credential-onboarding.md`, `.docflow/plan/done/2026-09-01-managed-agent-review-skills.md`, `.docflow/plan/done/2026-09-01-complete-tui-settings.md` | Shipped issues #216-#218 through PR #220; the required typecheck, 104 frontend tests plus tooling suites, Archgate 17/17, and a clean Norn branch review passed, and ADRs 0012 and 0015 advanced to Implemented. |
| 2026-09-01 | `7f62de2..68573de` | `fix/tui-settings-ux` | `.docflow/plan/todo/0005-refine-tui-settings-experience.md` | Refined the TUI settings surface into a bounded workspace overlay with grouped configuration, explicit GitHub and Bitbucket keychain flows, masked bracketed paste, predictable back/cancel behavior, narrow-terminal coverage, and sanitized public documentation; frontend and complete command-distribution Rust gates, Clippy, build, Archgate 17/17, focused input-limit regressions, and bounded Norn review completed. |
| 2026-09-01 | `c654019` | `main` | `.docflow/plan/done/2026-09-01-refine-tui-settings-experience.md` | Shipped the TUI settings and guided provider credential refinement through PR #224 after the required typecheck, 104 frontend tests plus tooling suites, Archgate 17/17, complete command-distribution Rust gate, Clippy, and clean pre-push Norn review passed. |
| 2026-09-01 | `2587dfa` | `fix/agent-review-permissions` | `.docflow/plan/todo/0006-agent-review-permission-handoff.md` | Added explicit local diff-sharing consent, proactive Codex and Claude Code host-permission guidance, stable restricted-host diagnostics, and bounded provider waits; typecheck, 104 frontend tests plus tooling suites, Archgate 17/17, Clippy, Tauri IPC, 101 focused Rust tests, fail-fast CLI probes, and bounded Norn review completed. |
| 2026-09-01 | `639ca00` | `fix/agent-review-permissions` | `.docflow/plan/todo/0006-agent-review-permission-handoff.md` | Reported known restricted Codex hosts before requesting diff-sharing consent after the pre-push review; the focused regression, runtime CLI probe, formatting, and Clippy passed before the bounded rerun. |
| 2026-09-01 | `8975b92` | `main` | `.docflow/plan/done/2026-09-01-agent-review-permission-handoff.md` | Shipped predictable Codex and Claude Code review permission handoff through PR #227; typecheck, 104 frontend tests plus tooling suites, Archgate 17/17, focused Rust coverage, runtime CLI probes, and bounded pre-push Norn review completed. |
| 2026-09-02 | `30c1df2` | `feat/tui-browser-diff-viewer` | `.docflow/plan/todo/0011-browser-diff-viewer.md` | Added the authenticated browser diff viewer, bound state to pull request identity, bounded failed provider population, blocked active SVG previews, repaired source-tree readiness exceptions, added general pull-request CI, and made onboarding E2E tests distribution-aware; typecheck, lint, frontend/tooling and Rust tests, command E2E, Clippy, formatting, and Archgate 17/17 passed. |
| 2026-09-02 | `29b0fd3` | `feat/tui-browser-diff-viewer` | `.docflow/plan/todo/0011-browser-diff-viewer.md` | Returned empty version-matched browser state responses before diff serialization and required Norn's canonical bundle identity for source compatibility exceptions; 577 Rust tests, Clippy, formatting, and Archgate 17/17 passed. |
| 2026-09-02 | `031f654` | `feat/tui-browser-diff-viewer` | `.docflow/plan/todo/0011-browser-diff-viewer.md` | Avoided cloning unchanged browser diff state, decoded Git-quoted paths, normalized provider file statuses before rendering, and added embedded-viewer regressions; build, lint, typecheck, 104 frontend tests plus tooling suites, 579 Rust tests, Clippy, Archgate 17/17, and a clean Norn review passed. |
| 2026-09-02 | `32c51e6` | `main` | `.docflow/plan/done/2026-09-02-browser-diff-viewer.md` | Shipped the authenticated browser diff viewer through PR #230 after typecheck, 104 frontend tests plus tooling suites, Archgate 17/17, the general CI gate, Homebrew contract gate, and bounded Norn review completed. |
| 2026-09-02 | `3497fc8` | `main` | `.docflow/plan/done/2026-09-02-browser-diff-shared-ui.md` | Shipped the shared React browser diff viewer through PR #236 after typecheck, 107 frontend tests plus tooling suites, build, Clippy, 26 focused Rust tests, Archgate 17/17, React Doctor 100/100, CI, and bounded Norn review passed. |
| 2026-09-02 | `1e08045` | `main` | `v0.2.9` | Published the signed v0.2.9 command release with Intel and Apple Silicon archives, verified Formula, clean install and upgrade lifecycle checks on both architectures, stable promotion, and public tap publication in workflow run `33686264305`; desktop artifacts remained intentionally disabled. |
| 2026-09-03 | `02a2b3e..1ddfed2` | `feat/local-review-tui` | `.docflow/plan/todo/0013-local-review-target-tui.md` | Added bounded immutable local-review snapshots and a first-class TUI Local target with native/browser diff views, explicit refresh, local AI review, provider-action restrictions, stale-result fencing, portable Git execution, snapshot-pinned image previews, and accurate line metadata; 609 Rust tests, 110 frontend tests plus tooling, Clippy, typecheck, lint, and Archgate 17/17 passed. |
| 2026-09-03 | `dc6a81f` | `feat/local-review-tui` | `.docflow/plan/todo/0013-local-review-target-tui.md` | Excluded upstream-only divergence through merge-base snapshots, surfaced ahead/behind state, rejected non-UTF-8 untracked paths before access, isolated persisted AI state by immutable snapshot identity, and aligned local image metadata layout after the final Norn findings; 613 Rust tests, 110 frontend tests plus tooling, Clippy, typecheck, lint, and Archgate 17/17 passed. |
| 2026-09-03 | `05b13e3` | `feat/local-review-tui` | `.docflow/plan/todo/0013-local-review-target-tui.md` | Added the full snapshot SHA-256 to browser state versioning and persisted AI-state validation while retaining the numeric ID only as a compatibility key; 615 Rust tests, 110 frontend tests plus tooling, Clippy, typecheck, lint, and Archgate 17/17 passed before the bounded Norn rerun. |
| 2026-09-03 | `3c92d87` | `feat/local-review-tui` | `.docflow/plan/todo/0013-local-review-target-tui.md` | Bound binary preview fingerprints into local snapshot identity, capped cumulative untracked-file reads and candidate scans, and restricted native old/new image previews to snapshot diffstat paths after Norn review; 618 Rust tests, 110 frontend tests plus tooling, Clippy, typecheck, lint, and Archgate 17/17 passed before the bounded rerun. |
| 2026-09-03 | `f7bde2a` | `feat/local-review-tui` | `.docflow/plan/todo/0013-local-review-target-tui.md` | Rejected untracked hard links through opened-handle metadata on Unix and Windows and fenced snapshot loading against concurrent branch or upstream changes after the bounded Norn review; 619 Rust tests, a standalone Windows API cross-check, Clippy, formatting, and Archgate 17/17 passed before the final rerun. |
| 2026-09-03 | `9a6b7aa` | `feat/local-review-tui` | `.docflow/plan/todo/0013-local-review-target-tui.md` | Replaced ambient untracked and preview reads with repository-root capabilities, bounded preview hashing by files and cumulative bytes without duplicate verification reads, and isolated local sessions and persistence by target kind plus full snapshot SHA-256; 623 Rust tests, 110 frontend tests plus tooling, all-target Clippy, typecheck, lint, Archgate 17/17, and an isolated Windows target check passed. |
| 2026-09-03 | `7836498..bf3f09d` | `feat/local-review-tui` | `.docflow/plan/todo/0013-local-review-target-tui.md` | Excluded untracked content pending explicit local per-file consent, made capability-backed Unix file opens non-blocking, revalidated bounded preview fingerprints before snapshot completion, and bound upstream SHA plus ahead/behind counts into snapshot identity after Norn review; 622 Rust tests passed with 2 ignored, alongside 110 frontend tests plus tooling, all-target Clippy, typecheck, lint, and Archgate 17/17. |
| 2026-09-04 | `3236736..849cf99` | `feat/local-review-tui` | `.docflow/plan/todo/0013-local-review-target-tui.md` | Routed local snapshot Git operations through a process-group-aware runner with bounded stdout, diagnostics, and duration; required preview fingerprints before filesystem access; and transactionally retained the newest 20 local snapshots per repository without touching PR or cross-repository stores; 626 Rust tests passed with 2 ignored, alongside 110 frontend tests plus tooling, all-target Clippy, typecheck, lint, and Archgate 17/17. |
| 2026-09-04 | `c28831e` | `feat/local-review-tui` | `.docflow/plan/todo/0013-local-review-target-tui.md` | Supervised Git exit and pipe closure under one deadline without blocking joins, contained descendants through Unix process groups and Windows Job Objects, and migrated local snapshot retention to transactionally monotonic generations; 627 Rust tests passed with 2 ignored, alongside 110 frontend tests plus tooling, all-target Clippy, typecheck, lint, Archgate 17/17, and an isolated Windows target compile check. |
| 2026-09-04 | `54bfdd0` | `feat/local-review-tui` | `.docflow/plan/todo/0013-local-review-target-tui.md` | Started Windows Git suspended before Job Object assignment, applied one cancellable deadline across each complete local snapshot, terminated superseded TUI loads, and restricted Local-mode navigation to usable configured repository paths; 631 Rust tests passed with 2 ignored, alongside 110 frontend tests plus tooling, all-target Clippy, typecheck, lint, Archgate 17/17, and an isolated Windows target compile check. |
| 2026-09-05 | `26b4ee6` | `feat/local-review-tui` | `.docflow/plan/todo/0013-local-review-target-tui.md` | Replaced preview reader workers with bounded Git object fingerprints and in-memory verification, fenced asynchronous repository eligibility by stable identity and generation, surfaced eligibility loading, stabilized deadline and HTTP regressions, and resolved ambient Windows Git lookup through canonical OS Known Folder paths; 642 Rust tests passed with 2 ignored, alongside 110 frontend tests plus tooling, all-target Clippy, typecheck, lint, Archgate 17/17, an isolated Windows compile check, and Norn review with no high-severity findings. |
| 2026-09-05 | `4cc7318` | `feat/local-review-tui` | `.docflow/plan/todo/0013-local-review-target-tui.md` | Resolved the release-blocking Norn findings by routing local review through the trusted Git resolver, fingerprinting previews from bounded bytes read through validated file handles, and aligning the TypeScript and mock snapshot contract with Rust's `previewOid`; 639 Rust library tests passed with 2 ignored, alongside 110 frontend tests plus tooling, typecheck, lint, and Archgate 17/17. |
| 2026-09-05 | `1025af3` | `feat/local-review-tui` | `.docflow/plan/todo/0013-local-review-target-tui.md` | Disabled executable and mutating Git configuration for local snapshots, sanitized inherited Git control variables, and added regressions proving repository-configured filesystem monitors and text conversion helpers cannot execute. |
| 2026-09-05 | `d74dda9` | `feat/local-review-tui` | `.docflow/plan/todo/0013-local-review-target-tui.md` | Removed direct working-tree reads from local image snapshotting: immutable previews now use bounded Git index object IDs and supervised `cat-file` reads, while unstaged image content remains excluded with an explicit warning. |
| 2026-09-06 | `e695660` | `feat/local-review-tui` | `.docflow/plan/todo/0013-local-review-target-tui.md` | Bound local repository eligibility checks to one cancellable five-second deadline with bounded Git output, cancelled superseded checks, and removed render-loop polling. |
| 2026-09-06 | `90b8812` | `feat/local-review-tui` | `.docflow/plan/todo/0013-local-review-target-tui.md` | Forced every local Git command onto the canonical configured worktree and ensured an empty Local repository eligibility result settles the TUI loading state; 643 Rust tests passed with 2 ignored, alongside 110 frontend tests plus tooling, all-target Clippy, typecheck, lint, and Archgate 17/17. |
| 2026-09-06 | `e592119` | `feat/local-review-tui` | `.docflow/plan/todo/0013-local-review-target-tui.md` | Supervised Git stdin writes under the shared deadline and process-tree cancellation, propagated eligibility failures instead of accepting partial lists, and resolved Git from validated Unix installation roots; 646 Rust tests passed with 2 ignored, alongside 110 frontend tests plus tooling, all-target Clippy, typecheck, lint, and Archgate 17/17. |
| 2026-09-06 | `eeef2e8` | `feat/local-review-tui` | `.docflow/plan/todo/0013-local-review-target-tui.md` | Restricted Unix Git discovery to exact validated installation entrypoints, persisted review target kinds without key-prefix inference, and protected active local review stores during bounded retention; 617 Rust tests passed with 2 ignored and 32 macOS-Keychain-dependent tests filtered, all affected regressions passed, alongside 110 frontend tests plus tooling, all-target Clippy, typecheck, lint, and Archgate 17/17. |
| 2026-09-06 | `1ebd3b0` | `feat/local-review-tui` | `.docflow/plan/todo/0013-local-review-target-tui.md` | Cleared previous local review state synchronously before repository loads and replaced complete untracked-path enumeration with a deadline-bound presence probe that terminates the contained Git process after first output; 621 Rust tests passed with 2 ignored and 32 macOS-Keychain-dependent tests filtered, alongside focused regressions, all-target Clippy, and Archgate 17/17. |
| 2026-09-06 | `63d7e31` | `feat/local-review-tui` | `.docflow/plan/todo/0013-local-review-target-tui.md` | Isolated snapshots from global and system Git configuration, discovered repository filter drivers without executing them, and neutralized clean, process, and required filter settings before working-tree inspection; clean and long-running process regressions, 622 Rust tests with 2 ignored and 32 macOS-Keychain-dependent tests filtered, all-target Clippy, formatting, and Archgate 17/17 passed. |
| 2026-09-06 | `93e4692` | `feat/local-review-tui` | `.docflow/plan/todo/0013-local-review-target-tui.md` | Preserved base-to-index and index-to-worktree patches as explicit staged and unstaged snapshot layers, included both in snapshot identity and TUI context, and invalidated canceled repository eligibility requests before leaving Local mode; 625 Rust tests passed with 2 ignored and 32 macOS-Keychain-dependent tests filtered, alongside 110 frontend tests plus tooling, all-target Clippy, typecheck, lint, and Archgate 17/17. |
| 2026-09-06 | `64b9f36` | `main` | `.docflow/plan/done/2026-09-06-local-review-target-tui.md` | Shipped the terminal-first Local review target through PR #247 after the required local gate and GitHub `verify` workflow passed; two final Norn attempts timed out at the configured provider boundary, and the maintainer explicitly authorised publication without that gate. ADR 0017 remains Accepted because its separately queued desktop acceptance criterion is not yet implemented. |
| 2026-09-28 | `e0dda49` | `feat/opencode-deepseek-provider` | `.docflow/plan/todo/0015-opencode-ai-review-provider.md` | Added the OpenCode DeepSeek review provider across config, headless CLI and setup, readiness probing, desktop settings, TUI, mock IPC, and docs; default-deny tool isolation, error-event rejection after partial output, and invalid model/variant rejection landed after a pre-push Norn review. Typecheck, lint, build, all-feature Clippy, the Rust suite, Archgate 17/17, and the bounded Norn rerun passed. |
| 2026-09-28 | `a3bc562` | `main` | `.docflow/plan/done/2026-09-28-opencode-ai-review-provider.md` | Shipped the OpenCode DeepSeek review provider through PR #264 as release `v0.3.1`; CI `verify`, both macOS architecture builds, release publication, formula smoke, stable finalization, and Homebrew tap publication succeeded while desktop signing remained intentionally disabled. ADR 0018 advanced to Implemented. |
| 2026-09-28 | `e8364ff` | `fix/readiness-legacy-scan-generated-outputs` | — | Stopped the readiness legacy-name scan from walking generated, gitignored `storybook-static/` and `.astro/` output, which had blocked the terminal UI preflight in the Norn source tree; covered both directories with a regression test and renamed the Storybook metadata to Norn. Preparing release `v0.3.2`. |
| 2026-09-29 | `80710e5` | `main` | — | Shipped the readiness legacy-name scan fix as release `v0.3.2` through PR #272; CI `verify`, both macOS architecture builds, release publication, formula smoke, stable finalization, and Homebrew tap publication succeeded. `norn doctor` on the source tree now reports zero `legacyNameNotAllowed` issues. |
| 2026-09-29 | `bd18cea` | `fix/opencode-isolated-tool-leak` | — | Fixed DeepSeek reviewed through OpenCode leaking raw tool-call markup into the review: keep the read-only tools registered so the provider uses OpenCode's native tool-call channel, deny reads outside the working directory and `lsp`, reject a line-anchored leaked-markup response, and retry an empty or leaked response once. Preparing release `v0.3.3`. |
| 2026-09-29 | `326fa13` | `main` | — | Shipped the OpenCode/DeepSeek review fix as release `v0.3.3` through PR #274; CI `verify`, both macOS architecture builds, release publication, formula smoke, stable finalization, and Homebrew tap publication succeeded. Two reviews with the installed binary and the `opencode`/`deepseek/deepseek-flash` configuration completed without leaked markup. |
| 2026-09-29 | `41e0398` | `fix/tui-opencode-cli-readiness` | — | Fixed the terminal UI readiness detector dropping OpenCode into the unknown-provider branch and always reporting the CLI Missing even though reviews worked; the detector now maps every AI provider including `opencode`. Preparing release `v0.3.4`. |
| 2026-09-29 | `6cf8264` | `main` | — | Shipped the terminal UI OpenCode readiness fix as release `v0.3.4` through PR #276; CI `verify`, both macOS architecture builds, release publication, formula smoke, stable finalization, and Homebrew tap publication succeeded. |
| 2026-10-09 | `2f78c6c` | `issue-295-opentui-architecture` | `.docflow/plan/todo/0016-opentui-rust-architecture.md` | Recorded the OpenTUI/Rust stdio architecture (ADR 0020, superseding ADR 0006), the migration contract with a full parity matrix and dispositions for every shipped terminal and desktop workflow, package boundaries, command/runtime contract, platform matrix, and reconciliation of #255/#253/#284. Epic #294 step 01/15, issue #295. |
| 2026-10-09 | `25f73d2` | `issue-296-engine-extraction` | `.docflow/plan/todo/0017-opentui-engine-extraction.md` | Gated the Tauri desktop adapter behind the optional `desktop` feature and the Ratatui terminal UI behind the optional `tui` feature, so a `--no-default-features` build compiles the engine and headless commands with no Tauri, Ratatui, crossterm, or ratatui-image in the active dependency tree; added `test:rust:core` and a dependency guard wired into CI and the release gate; kept every shipped distribution building. Epic #294 step 02/15, issue #296. |
| 2026-10-09 | `9fac324` | `issue-297-opentui-protocol` | `.docflow/plan/todo/0018-opentui-protocol.md` | Defined the versioned typed stdio protocol: JSON Schema single source with shared fixtures, generated TypeScript types plus schema copy, ajv runtime validation, and Rust envelope types, newline framing, versioned handshake, error taxonomy, and a per-operation `SequenceGuard`; added a `protocol:check` drift gate wired into CI and the release gate. Epic #294 step 03/15, issue #297. |
| 2026-10-09 | `c622f17` | `issue-298-opentui-backend` | `.docflow/plan/todo/0019-opentui-backend.md` | Added the persistent `norn-backend` stdio host: versioned handshake, typed dispatch, prioritized control channel plus bounded event channel, bounded frame reading, threaded cancellable operations, credential-free local Git diffs (tracked/deleted/staged/untracked), provider target resolution, bounded concurrency, and shutdown that terminates owned Git children; real subprocess integration tests. Epic #294 step 04/15, issue #298. |
| 2026-10-09 | `70d5c7a` | `issue-299-opentui-shell` | `.docflow/plan/todo/0020-opentui-shell.md` | Added the `ui/` OpenTUI React package: typed backend client with production subprocess and fake transports (bounded pending, timeouts, EOF/version handling, drained stderr), a generation- and operation-id-fenced shell store, an OpenTUI shell with focus/navigation/help/status views, and terminal restoration; plus CI `ui` job and task-runner recipes. Epic #294 step 05/15, issue #299. |
| 2026-10-09 | `b5d3155` | `issue-300-opentui-frames` | `.docflow/plan/todo/0021-opentui-frames.md` | Added native OpenTUI frame capture (spans, colors, attributes) into committed fixtures, Storybook frame previews and shell stories, and a golden-frame visual lane (`frames:check`) that fails with artifacts and never overwrites the baseline; wired into the CI `ui` job. Epic #294 step 06/15, issue #300. |
| 2026-10-11 | PR #333 head | `issue-303-review-reply-ipc` | `.docflow/plan/done/2026-10-11-opentui-review-reply.md` | Prepared reply-plan completion and fixed bracketed paste and failed-draft preservation after code inspection. The maintainer authorized sequential PR merges through epic #294 and explicitly excluded Norn reviews. Completion is effective only when PR #333 merges. |
| 2026-10-11 | PR #334 head | `issue-303-finding-thread-navigation` | `.docflow/plan/done/2026-10-11-opentui-finding-thread-navigation.md` | Added bounded stored conversation IPC, thread/run selection, severity filters and file/severity grouping with stable finding selection; prepared completion effective on merge. No Norn review per maintainer instruction. |

| 2026-10-11 | default-agent | Prepared plan 0033 / PR #335: native review.fix preview, snapshot/index/binary-bound one-use approval, cancellation and partial-result inspection. Root gates and 17/17 architecture checks, 43 UI tests/17 frames, 602 core tests (2 ignored) and fake-agent subprocess coverage pass. Test-only keychain mock prevents OS credential access; production defaults unchanged. Delivery becomes shipped when PR #335 merges. |

| 2026-10-11 | default-agent | Plan 0034: prepared approved commit/push delivery after merged PR #335. Literal selected-file commits preserve prior and unrelated staging; push uses the exact approved ref and HEAD. 30 backend tests, 46 UI tests, 21 frames and root gates pass; 603 core tests pass (2 ignored) after normalizing inherited HTTP socket mode, with delayed-header regression coverage. No Norn review. |

| 2026-10-11 | default-agent | Plan 0035 prepared: typed approved sync/conflict operations, actual merge path verification and separate merge commit/push. Local gates: 34 backend subprocess tests, 48 UI tests, 24 frames, 603 core tests (2 ignored), root/Archgate and strict Clippy. Catalogue numbering/INDEX checks pass; baseline audit found ADR 0011 Accepted despite its historical done plan and an unresolved ADR 0018 link to the old ADR 0015 slug. Reconcile under final parity/documentation work. PR #336 still awaiting Rust CI. |

| 2026-10-11 | default-agent | PR #336 merged at `350103f` with UI and Rust CI green. Plan 0035 is prepared in PR #337; all local gates pass and React Doctor reports no issues after panel extraction. No Norn review used. |

| 2026-10-11 | default-agent | Plan 0036 prepared: late start/reply/fix ownership, exact cancellation IDs, target/sequence fencing, coalesced gap/status recovery, mounted polling with unmount cleanup, final saved-run/finding refresh and native error/draft recovery. UI 56 tests and 26 frames pass; root/Archgate gates pass. React Doctor await-in-loop diagnostic is intentional serialization, covered by burst bounds; no suppression. |

| 2026-10-11 | default-agent | PR #337 merged at `2f9b58e`, all CI green. Plan 0036 is prepared in PR #338 with 56 UI tests/26 frames and root/Archgate gates. Next: settings/onboarding and local user-data recovery for #305. |

| 2026-10-11 | default-agent | Plan 0037 prepared: Rust-owned versioned atomic settings, separate masked credential writes/status/clear, bounded schemas and terminal forms. Root 121 tests/tooling, 62 UI tests/32 frames, 609 core tests (2 ignored), 35 real backend subprocess tests and strict all-feature Clippy pass; no real credential access. #305 remains open for desktop-local recovery and final parity. PR #338 merged at `3168dcd`. No Norn review. |

| 2026-10-11 | default-agent | PR #339 merged at `76ba213`, UI/verify CI green. Plan 0038 prepared: allowlisted desktop export, typed Rust user-data records, version/provider-fenced preview and additions-only atomic import with verified backup, conflict/rejection counts and idempotence; terminal controls and headless data CLI. Local gates: root 123 tests/tooling, Archgate 17/17 after a governed read-only export exception; 67 UI tests/36 frames; 36 backend subprocess tests; 7 isolated import tests and strict Clippy. No Norn review. Consumption and archived platform recovery verification remain final #305/#306 gates. |

| 2026-10-11 | default-agent | Final plan 0038 evidence: 616 core tests pass (2 ignored), no-GUI guard, 36 backend tests, strict Clippy, 67 UI tests/36 frames and production desktop/browser assets build. React Doctor reports no issues in both projects after status extraction. The export bridge source is pinned at `34a3c2a2b56b698ef8cf9978f0b1e0a7ba4402db` for already-upgraded recovery; installed platform recovery and consumption remain tracked before cutover. |

| 2026-10-11 | default-agent | PR #340 CI found the data CLI assertion running against the intentionally GUI-only desktop-bundle binary. Moved the real CLI round-trip/export/repeat/no-confirmation test to the existing command-distribution onboarding lane, while the real backend test remains in both configurations. No product routing change and no skipped coverage; temporary config/data only. |

| 2026-10-11 | default-agent | Plan 0039 prepared: changed-file old/new image sides including rename/delete/add, 192 KiB chunks within the 1 MiB frame, 2 MiB total/content-hash fencing and native dimension/format limits. Both sides render independently with narrow stacking and explicit block/browser fallback. Local evidence: root 123 tests/tooling and Archgate 17/17; 72 terminal tests/42 frames; 616 core tests (2 ignored) and no-GUI guard; 39 backend tests. Native decoder and Kitty/Sixel/tmux capability selection tests pass; physical-terminal display smoke and text/performance remain final gates, not claimed complete. PR #340 awaits corrected CI at `a63af00`; no Norn review. |

| 2026-10-11 | default-agent | PR #340 merged at `72440ea` with both CI gates green. Rebased plan 0039 onto the corrected main baseline, preserving the command-distribution CLI test and desktop-bundle backend test separation. Prepared image completion record with physical-terminal and text/performance gaps explicit; no Norn review. |

| 2026-10-11 | default-agent | PR #341 merged with UI/verify green. Plan 0040 prepared: viewport/overscan React rows with native word-wrap geometry and cumulative source anchors, complete scroll extent and offscreen horizontal width, frame/resize lifecycle cleanup. Root 123 tests/tooling/Archgate 17/17; 75 UI tests (409 assertions)/44 native frames. Expanded 10,000 rows keep 39 mounted rows; parent expanded renderer exhausted native TextBuffers, now rejected by the benchmark. Matched Ratatui and physical image-platform evidence remain final gates. No Norn review. |

| 2026-10-11 | default-agent | PR #342 merged at `2a2ed72`, both CI gates green. Plan 0041 prepared: same-input native Ratatui/OpenTUI rendering across six cases/two modes/three runs and 100 scroll samples, including a pinned real tracked-source diff; all budgets pass. Root 123 tests/tooling/Archgate 17/17, 75 terminal tests (explicit child env assertion)/44 frames, opt-in Rust release test, strict Clippy. Real read-only IPC uses explicit isolated environment and records ~26–30 ms round trips plus ACK/EOF null exit code; writer latency and EOF reaping remain #307, streaming/platform/installed gates remain open. No Norn review. |

| 2026-10-11 | default-agent | PR #343 merged at `ccd133d`, UI/verify green. Plan 0042 prepared: typed paginated immutable canonical records, per-record CAS/atomic mutations and cross-process storage locks; saved prompt editor, restored PR viewed marks and selected-repository policy resolution. Root 123 tests/tooling/Archgate 17/17; terminal suite plus six workspace tests and 48 frames; 623 core tests (2 ignored), no-GUI guard, 42 real backend subprocess tests and strict all-feature/all-target Clippy pass. Fake agents prove start and fresh-thread reply policy/override precedence and invalid-policy rejection; successful Claude resume intentionally retains its prior conversation. React Doctor reports no issues in the changed UI files after a Set lookup fix. Reference/draft publication/enrichment, theme policy, platform recovery and broader parity remain open. No Norn review. |

| 2026-10-11 | default-agent | Prepared plan 0042 completion record at implementation `7d492d3`; pre-completion root gate exits 0 (typecheck, 123 tests/tooling, Archgate 17/17). GitHub #303 is already closed (2026-10-11 11:30 UTC); broader recovery/streaming/fix parity stays with #306/#307 instead of reopening it silently. Main unchanged at `ccd133d`; no numbering collisions. Final completion gate and PR CI/merge remain required. |

| 2026-10-11 | default-agent | PR #344 merged at `5eb33fd`, UI/verify green. Plan 0043 prepared: explicit literal diff search with metadata/source navigation and paste fencing; bounded target/run/file/content/mode scroll positions; mixed metadata/two-column wrap geometry and listener ownership. Native tests now update one React container instead of recreating roots per draw. UI 87 tests/493 assertions and 52 native frames pass. Three separate synthetic stream runs (10,000 events + 10,000-line diff in each mode) pass 20 ms p95/64 rows/100 logs/512 MiB budgets with wrong-target fencing; raw artifacts committed. Syntax/clipboard and physical image smoke remain #302; no Norn review. |

| 2026-10-11 | default-agent | Plan 0043 pre-completion root gate: typecheck, 123 tests/tooling, lint/protocol and Archgate 17/17 pass. React Doctor reports one native `wrapMode` test-property DOM-rule false positive (installed types and real rendering cover it); no suppression. Functional/image/platform remaining gates stay explicit. |

| 2026-10-11 | default-agent | Prepared plan 0043 delivery at implementation `2d0cea7`; pre-completion root gate exits 0 (typecheck, 123 tests/tooling, Archgate 17/17). Main is PR #344 merge `5eb33fd`; ADR/plan numbering and pending-owner links checked. Final completion gate and PR CI/merge remain required. |


| 2026-10-11 | default-agent | PR #345 merged at `565d081`, UI/verify green. Plan 0044 prepared at implementation `94c4ebc`: bounded source-preserving Refractor syntax in an owned subprocess (replacing a reproduced Bun 1.3.8 WebWorker host crash), explicit exact run/provider-fenced review-output copy with OSC52 emission semantics and fake clipboard captures, isolated physical image demo/checklist. Root 123 tests/tooling/Archgate 17/17; UI 98 tests/555 assertions/56 native frames; core 623 tests (2 ignored), 43 backend subprocess tests and strict Clippy pass. React Doctor finds no issues in changed UI files. Synthetic 10k-row/10k-event recheck passes all budgets. Physical image and installed worker gates remain explicit; no Norn review. |


| 2026-10-11 | default-agent | PR #346 merged at `066174e`, UI/verify green. Plan 0045 prepared at `8c7974c`: status-only PATH/tool/root/build readiness, first-run guidance and generation-fenced coalesced settings refresh. Root 123 tests/tooling/Archgate 17/17; terminal 102 tests/574 assertions/59 native frames; core 625 tests (2 ignored), 44 backend IPC tests and strict Clippy pass. Existing CLI doctor regression queried the real keychain and blocked; interrupted that owned process, replaced the doctor path with a mock-keyring/empty-environment/synthetic-tool test child, asserted synthetic versions and retained actual CLI help/init/data routing. Corrected five-test CLI lane passes. React Doctor finds no issues in changed UI files. No provider publication or Norn review; remaining recovery/consumption/physical/installed gates stay open. |

| 2026-10-11 | default-agent | PR #347 merged at `58247d3`, UI/verify green. Plan 0046 prepared at `a134f5d`: actual owned iTerm2 3.6.11/Sixel synthetic screenshots at wide/narrow/wide plus explicit quit exit 0 and identical terminal modes. Fixed one-cell images and automatic blank Kitty placements by filling panes and preferring observed Sixel on iTerm. All seven owned windows closed; existing sessions untouched. Root 123 tests/tooling/Archgate 17/17; UI 104 tests/611 assertions and 59 frames pass, lint JSON newline corrected. Retina scaling limitation stays explicit for hardening, with no pixel-perfect/other-platform claim. No Norn review or real backend/provider/agent/keychain/data access. |

| 2026-10-11 | default-agent | Plan 0047 prepared at `c128831`: explicit imported-theme preview/staging followed by existing settings CAS save, and unsaved settings retained across the data overlay. Root 123 tests/tooling/lint/Archgate 17/17; UI 109 tests/648 assertions/61 frames pass. React Doctor's 12 native flexShrink unknown-DOM warnings are verified against installed OpenTUI types and native tests, not suppressed. Complete archived exporter merge `72440ea` compiled with production frontend/custom protocol locally; two isolated copy tests and six archived DOM fixtures pass, no desktop/profile/keychain accessed. Added Linux/macOS/Windows archive build/copy CI, pending; integrate after #348. Draft/reference consumption and native recovery/cutover gates stay open. No Norn review. |

| 2026-10-11 | default-agent | PR #349 CI correction: archive desktop compiles on all three OSes; macOS/Linux copy tests pass. Windows GUI-linked unit binary fails before tests with STATUS_ENTRYPOINT_NOT_FOUND, root cause unclaimed. Narrowed only the pure directory-copy regression to engine-only unit dependencies while retaining full embedded desktop compilation. Contract-only tooling eagerly imported TypeScript without installed dependencies; moved transpiler import into the explicitly invoked source verifier. No native profile/keychain access or Norn review. |

| 2026-10-11 | default-agent | PR #349 merged at `da0b87f`, all UI/verify/contract and three-platform archived desktop compile/engine-copy gates green. Plan 0048 prepared at `e9930a1`: scoped canonical reference list/form with explicit CAS save/removal, imported metadata preservation, paste/conflict/disposal fencing, wrapped-field frame ownership and same-PR refresh retention; Rust bounded reference context in new/fresh-session payloads, provider/PR isolation and resumed-conversation preservation. Root 123 tests/tooling/lint/Archgate 17/17; UI 115 tests/691 assertions/67 frames; core 627 (2 ignored), no-GUI guard, 46 backend subprocess tests and strict Clippy pass. React Doctor native flexShrink/old giant Shell warnings remain verified/unsuppressed; Set lookup fixes the new performance concern. No real provider/keychain/agent access, live publication or Norn review. #304 draft consumption and #306 enrichment/recovery/parity remain. |

| 2026-10-11 | default-agent | PR #350 merged at `bf7c15c`, UI/verify green. Plan 0049 implementation prepared: imported draft editor preserves identity/timestamps/source/finding/revision/parent metadata, explicit body/anchor/mode edits and local CAS save/removal, provider/PR fences, paste limits, same-PR refresh retention and native wrapped-field follow/cleanup. Root typecheck/123 tests/tooling/lint/Archgate 17/17, UI typecheck/123 tests/747 assertions/73 native frames, 47 real backend subprocess tests and strict all-feature/all-target Clippy pass. React component complexity/effect dependency findings corrected; final rebased delta reports only eight native flexShrink unknown-DOM warnings, verified with installed types/native input/frames and unsuppressed. No publication/real provider/agent/keychain or Norn review. Authoritative finding staging/approved publication/reconciliation remain #304; final recovery/parity remain #305/#306. |

| 2026-10-11 | default-agent | Pre-integration docflow relative-link audit found the pre-existing ADR 0018 reference to the old ADR 0015 filename. Corrected its slug editorially without changing metadata, decision scope, status or approvals. |

| 2026-10-11 | default-agent | Plan 0049 prepared delivery footer at `c4e3fff`; final completion root typecheck, 123 tests/tooling and Archgate 17/17 exit 0. Conditional on PR merge, not shipped yet. Based on main `bf7c15c`; numbering/status/relative links and next plan slot audited with no collisions after the editorial link repair. `.norn.yaml` remains unrelated and untouched. |

| 2026-10-11 | default-agent | Opened PR #351 for verified local draft editor; independent plan 0050 claimed for authoritative finding-to-draft staging. Integrate sequentially after #351 CI/merge. No publication, provider/keychain access or Norn review. |

| 2026-10-11 | default-agent | PR #351 merged at `fc9837e`, UI/verify green. Plan 0050 implementation: Rust prepares exact persisted finding metadata and mode, read-only preview and explicit hash/version-bound canonical staging; duplicate staging retains edited drafts. Native preview/paste/cancel/same-pinned-target refresh, CRLF storage/display, strict DTOs and late-state fences covered. Root 123 tests/tooling/lint/Archgate 17/17; UI 129 tests/800 assertions/81 frames; core 629 tests (2 ignored), no-GUI guard; corrected full 48 backend subprocess tests pass. Initial fixture-root change broke reference context in an existing reply test; reverted shared fixture and isolated only the new restart fixture, then reran the full lane successfully. React Doctor reports 17 native flexShrink DOM-rule warnings verified with installed types and native input/frames, unsuppressed. Final path validator Clippy simplification corrected without changing rejection rules. No provider publication, real agent/keychain access or Norn review. #304 publication/thread/feedback/reconciliation and #305 final recovery stay open. |

| 2026-10-11 | default-agent | Plan 0050 delivery prepared at `c6eac6c`, based on main `fc9837e`. Final root gate exits 0; 123 tests/tooling and Archgate 17/17. Conditional merge footer, no premature shipment/issue closure. Initial audit script incorrectly expected filename links in the number-only INDEX; corrected to actual INDEX membership format and audit passes within stated scope. |

| 2026-10-11 | default-agent | Opened PR #352 for verified finding staging; independent plan 0051 claimed for the durable approved-publication journal. Integrate only after #352 CI/merge. Remote adapters and UI approval follow; no live publication or Norn review. |

| 2026-10-11 | default-agent | Plan 0051 initial eight journal tests pass on actual-migrated owned SQLite files. Full core run failed because sandbox denies loopback server bind, and the old v1 migration test expected versions only through 15 (poisoning its shared test lock afterward). Updated the explicit expected list to include schema 16 and reran with authorized loopback permissions. Added writing-before-provider and failed-reservation storage checks, for nine journal tests. Full corrected core/Clippy/stdio lanes pending; no live provider or Norn review. |

| 2026-10-11 | default-agent | Plan 0051 implementation verified: root typecheck/123 tests/tooling/lint/Archgate 17/17; corrected core 638 passed (2 ignored), no-GUI guard; 48 real backend subprocess tests and strict all-feature/all-target Clippy pass. Nine focused journal tests rerun after adding altered-remote-body reconciliation and writing-before-send assertions, all pass; final strict Clippy exit 0. Distinguishes verified postflight mismatch from unavailable postflight evidence, retaining remote ID in both. No UI changes, provider publication, keychain/agent access or Norn review. Integrate after pending #352 CI/merge; concrete adapters/IPC/UI/feedback remain. |

| 2026-10-11 | default-agent | PR #352 merged at `5f518b5`, UI/verify green (Rust verify 13m38s). Rebased plan 0051 on merged main, implementation `0b60c44`, prepared conditional delivery footer. Final completion root gate and PR CI/merge required; no premature issue closure/cutover. |

| 2026-10-11 | default-agent | Opened PR #353 for verified journal; plan 0052 claimed for completion of #304 provider adapters, typed approved batch publication, threads, canonical cleanup and atomic feedback/reconciliation. Sequential integration after #353 CI/merge. Reviewed official GitHub review-comment and Bitbucket pull-request comment documentation; GitHub file comments use subject_type=file and reply parameters do not authorize relocating the parent anchor. No live provider publication or Norn review. |

| 2026-10-11 | default-agent | PR #353 merged at `3385fe1`, UI/verify green (verify 13m56s). First merge invocation constructed an invalid expected SHA and GitHub rejected it without mutation; corrected to the full verified head SHA. Rebased plan 0052 claim onto merged main with autostash, preserving owned in-progress adapters and unrelated `.norn.yaml`. Nine owned-loopback adapter tests and three pure diff-position tests pass; later validation refinements require rerun. Adapter strict Clippy currently reports unused production paths until IPC integration (not suppressed; no completion claim). Full #304 typed/UI/feedback integration remains. |

- 2026-10-11 — Plan 0052 implemented approved provider publication, single-use scoped/CAS/source approvals, cancel/status/reconciliation, remote root replies and atomic finding feedback repair with conditional canonical cleanup. Root typecheck/123 tests+tooling/lint/Archgate 12, UI typecheck/136 tests/830 assertions/89 native frames, core 660+2 ignored, 49 stdio, five CLI/five onboarding, no-GUI guard and strict all-feature/all-target Clippy pass. Owned-provider/native-input/legacy migration/atomic rollback tests pass; initial fixture errors and corrections are recorded in `docs/opentui-migration/approved-publication.md`. Preparing PR, not yet merged; #304 remains open and ADR 0020 Accepted. No live publication, credential/agent access, Norn review or user .norn.yaml changes.

- 2026-10-11 — Committed plan 0052 implementation as `7b2ec31`; all final gates pass including full source-feedback/header validation. Prepared done footer effective only on PR merge; ADR 0020 stays Accepted. Final React Doctor has seven native `flexShrink` DOM false positives only (installed type and 89 actual frames prove native behavior); no suppressions. Main remains `3385fe1`; #304 is not yet closed.

- 2026-10-11 — Opened PR #354 at `bcdad2e`; UI CI passed in 25s, verify running. Fresh GitHub timeline shows #305 was already closed by `delaudio` at 16:16:54 UTC; previous live snapshots overstated its open status. Corrected active/completion evidence, keeping native runtime/recovery evidence and scope access as explicit #306/#307 cutover work. No reopening or completion claim for unobserved native recovery.

- 2026-10-11 — Final #304 inspection found colon-delimited finding associations could alias different run/finding tuples. Replaced transient batch acknowledgment keys with typed `FindingAssociation` equality/hash. Added an actual two-run publication regression: an external acknowledgment for the second finding cannot borrow the first finding’s approval. Six manager tests, full core 661+2 ignored / 49 stdio / five CLI / five onboarding and strict all-feature/all-target Clippy pass. This narrow correctness fix refreshes PR #354 CI before merge; no provider/credential/user-profile access.

## 2026-10-11 — publication shipped; canonical scope parity claimed

PR #354 squash-merged at `4a303f1042e1747c7479d645392640da0921aed9` after UI and full verify passed on `bf289a0c2b1a4252e26c3c81c17c181f59b90cdd`; #304 closed at 20:47:07 UTC. CI duration 14m04s. Plan 0052 completion is now effective. Claimed #355 / plan 0053 on the synchronized baseline for canonical imported PR scope reachability and real saved-run/provider identity. #306–#309 and epic #294 remain open; ADR 0020 remains Accepted. No Norn review, real provider publication, credential access or user-data removal. Preserve unrelated `.norn.yaml`.

## 2026-10-11 — canonical scope picker implementation and local evidence

Plan 0053 / #355 adds bounded immutable content-free scope inventory, imported/unconfigured PR selection, provider labels, real saved run IDs and provider-filtered legacy reads. Local tests: four inventory cases including 10,000 scopes/cache expiry/path bounds; two real stdio cases for restart and saved identity; UI 143 tests/864 assertions and 93 exact frames (four added, ten existing descriptions intentionally changed). Root typecheck/123 tests+tooling/lint/Archgate 12/12 and strict all-feature/all-target Clippy passed. Full core 665+2 ignored, 51 stdio, five CLI/five onboarding passed in one complete run; inherited localhost HTTP fixtures also failed intermittently in subsequent complete runs (pagination authentication and Bitbucket acknowledgment), including a four-thread attempt. Isolated pagination and the latest complete four-thread library run passed; diagnostic assertions now include request paths and cached author state, cause remains unproven and tracked under #307. No assertion or provider deadline weakened. React Doctor no changed-file issues. Implementation remains unmerged; full parity, provider-less legacy history, native recovery, hardening, packaging and cutover remain. No Norn review or real provider/credential/profile access.

HTTP-fixture diagnosis update: a subsequent failure recorded `requests=[("", "")]` and a cached author error. The serial fixture blocked on an idle accepted TCP connection and falsely counted it as HTTP. It now uses a sixteen-connection bounded worker set, ignores incomplete/empty requests, joins workers and sends a buffered response. Added an owned idle-first connection regression; production deadlines/assertions remain unchanged. All twelve HTTP adapter tests pass; final default-concurrency core library passes 666+2 ignored, complete stdio/CLI gate in progress. This supersedes the earlier unproven timing hypothesis; it does not claim unrelated lifecycle/hardening complete.

Plan 0053 completion prepared after implementation `e4264db`, effective only on verified PR merge. Final full default-concurrency Rust gate passed 666+2 ignored/51 stdio/five CLI/five onboarding; final strict all-feature/all-target Clippy passed. Catalogue/index/main slot checked; historical ADR 0011 coverage unchanged. #355 is still open before integration; remaining #306 parity and full cutover are not claimed complete.

- 2026-10-11: PR #356 merged at `2a7f0fe605d3012c2e2b9a0c03e1d15b60f7c574` after exact `92f931c` UI/verify success (verify 13m54s); #355 closed. Claimed plan 0054 / first bounded #357 native provider read delivery on synchronized main. Remaining terminal browse, pre-review pinned diff and explicit PR approval are separate #357 deliveries; no premature issue/ADR completion.

- 2026-10-11: Plan 0054 / #357 implements bounded native `provider.prs`, `provider.pr.detail`, `provider.user` and strict generated DTO/client identity/generation checks. Both providers preserve raw-page continuation, filters and actual approval metadata; Bitbucket UUID aliases remain encoded single segments. Final gate exits zero: root typecheck/123 tests+tooling/lint/Archgate 12/12, protocol, UI 148 tests/886 assertions/93 unchanged frames, Rust 672+2 ignored/52 stdio/six CLI/five onboarding/three bin tests, strict all-feature/all-target Clippy and no-GUI guard. Five HTTP tests plus UUID bounds and five transport tests added.
  Initial full core attempt exited 101 from inherited publication fixture with zero requests; diagnostic/final core passed, recurrence recorded in #307 rather than declared solved. New stdio assumptions corrected without production changes. An inherited real CLI `auth status` test was found querying the OS adapter; owned processes interrupted (cargo exit 143) and replaced with isolated child mock-keyring dispatch, actual executable help routing and environment-cleared temporary CLI commands. No returned private credential material was inspected. Subsequent complete isolated suite exits zero. `.norn.yaml` remains untouched; #357 terminal browsing/pinned diffs/approval and #306–#309 remain open.

- 2026-10-11: Plan 0054 completion prepared against verified `8f1a120`; all final local gates pass, exact-head CI and squash merge still required. No new plan collision on synchronized main; owner links/contiguous ADRs verified and ADR 0020 remains Accepted. Historical ADR 0011 coverage unchanged. #357/#306 not closed by the read bridge.

- 2026-10-12 (Rome): PR #365 merged at `2f2cd50d3314b09528f323f40778c0eb45cc4879` after exact `8e2e36d` UI/verify success (verify 12m46s); first #357 read bridge shipped. Claimed plan 0055 native PR browser on synchronized main. #357/#306 remain open for all required deliveries; no Norn reviews.

- 2026-10-12: prepared plan 0055 at feature `df7ab05`: F2 native PR browser,
  grouped provider/state/actual-author/query filters, independent explicit pages,
  bounded two-read scheduler with generation fences, metadata/reviewer detail,
  and workspace selection preserving actual saved runs. Twelve owned store/native
  input tests and twelve new native-cell stories; all local gates passed, UI
  160 tests/970 assertions/105 frames. Rapid batched Tab/arrow input stale closure
  fixed; reused Intl segmenter after React Doctor. Assessed native DOM-property
  false positives without suppression. Prepared done receipt is not effective
  until exact-head CI and squash merge. #357/#306 remain open; no Norn review,
  live provider action, private credential/profile access or release publication.

- 2026-10-12 (Rome): PR #366 merged at `dd7562d29b712df6576ed31582123c4a8005e523`
  after exact `8acede54cc6cc503872e16507f224d35e0591c98` UI/verify success (verify
  13m43s); plan 0055 shipped. Claimed plan 0056 immutable native provider snapshots
  on synchronized main. Prepared actual owned Git fixture for destination-tip /
  merge-base distinction; no provider call. #357 remains open for all remaining
  deliveries, and #306–#309/epic remain active. No Norn review.


## 2026-10-12 — plan 0056 immutable provider snapshots prepared

- Claimed plan 0056 after verified PR #366 merge at `dd7562d`.
- Added actual native GH/BB immutable merge-base/head snapshots, distinct
  destination pin, complete bounded patch inventory, scoped opaque cache and
  generated snapshot/files/diff IPC with strict TS page/hash/generation checks.
- Owned evidence covers fork/multi-page/301-file comparisons, Unicode/rename/
  copy/empty/binary/mode changes, stale metadata, malformed/truncated inventory,
  redirect/429/size/deadline errors, expiry/LRU/restart and byte/frame bounds.
- Full-suite failure diagnosed as inherited O_NONBLOCK on accepted macOS test
  sockets: delayed/fragmented regression failed before fix; restore blocking
  mode in both owned fixtures, preserving limits/deadlines and production code.
  Temporary diagnostics removed; regression and full uninstrumented suite pass.
- Root 123 tests plus tooling/typecheck/lint/protocol, UI 164 tests/994 asserts/
  105 unchanged frames, Rust 686 core plus two existing ignored/53 stdio/six CLI/
  five onboarding/three bin tests, strict all-feature/all-target Clippy/fmt,
  no-GUI dependency guard and Archgate 12/12 pass. ADR 0020 remains Accepted.
- Native terminal preview/review-start and approve/browser integration remain
  #357 follow-ons; no private credentials/data/live provider mutation used.
- Preparing PR and exact-head CI/squash merge; this entry is not a shipping claim.

- Prepared plan/done receipt for feature `09443bf`; effective only after exact-head
  CI and squash merge. No issue/ADR completion claimed by the prepared path.


## 2026-10-12 — PR #367 shipped; claim plan 0057 native provider preview

- PR #367 squash merged at `22b5b1a441593b2dce9423dd3006b0b143dabf2b` after
  exact `5718adb2016426a18acea17330a219d373cd75ec` UI and verify success;
  CI run 38183029202, verify 11m45s. Prepared plan 0056 receipt is effective.
- #357 remains open: immutable backend bridge shipped, native preview/review
  start and explicit approve/external browser still required.
- Claimed plan 0057 on `issue-357-native-provider-preview` before code. Native
  pinned provider raster/tree/blob acquisition preserves remote image previews
  without requiring a local clone; keep bounded cache and browser source fences.
- Read official GH trees/blobs/contents and BB commit-file metadata docs. Raw GH
  contents can dereference symlinks; use verified regular Git blob semantics.
- ADR 0020 remains Accepted; no cutover/profile/platform/install proof waived.


## 2026-10-12 — plan 0057 native provider preview, local gate complete

- Fresh PR selection consumes immutable provider snapshots before AI; saved
  runs remain selected until explicit F3, F5 refreshes, return restores findings
  and conversations. Scope/handle/generation/EOF fences prevent stale evidence;
  local-only reserved IDs never issue provider reads. Settings retain pins.
- Native fork image acquisition uses verified regular Git trees/blobs (GH) or
  exact commit-file metadata/raw (BB), bounded raster cache/chunks/hash/TTL and
  the shared two-provider-read gate. Pierre uses the same evidence; rename old
  path translation is exercised against actual cached before/after bytes.
- Browser launcher stdio is null, Unix child ownership/reaping bounded and
  asynchronous, Windows URL dispatch native without cmd interpolation. Native
  acknowledgment does not claim display success; owned tests never launch the
  user's browser or use private credentials/profile data.
- Local root 123 tests plus tooling/typecheck/lint/protocol pass; UI 172 tests/
  1057 assertions/115 frames, ten new and 105 unchanged; Rust 697 core with two
  existing ignored/53 stdio/six isolated CLI/five onboarding/three bin tests;
  strict all-feature/all-target Clippy/fmt/no-GUI, Archgate 12/12 and docflow
  metadata/INDEX/sections/links/main-number audit pass. Existing 0011/0013
  coverage and ARCH007 length advisories unchanged. React Doctor 64/100, three
  native OpenTUI property false positives confirmed from renderer props/types,
  no errors/complexity/suppression. No Norn code review.
- Fresh GitHub events correct previous stale tracking: maintainer closed #357
  at 2026-10-11 22:01:23Z, with no associated commit. Preserve closure; review
  start using exact evidence and explicit approval/external provider browsing
  still require #306 retained-parity verification. #358–#364/#306–#309/#294 open.
- Preparing feature commit and PR; this entry is not a merge/shipping claim.

- Prepared plan 0057 receipt for feature `8f7c5cbdc914098a3b109991056e20486b129543`; effective only
  after exact-head CI and verified squash merge. ADR/epic completion not claimed.

## 2026-10-12 — PR #368 shipped; claim plan 0058 review scope foundation

- PR #368 squash merged at `1f9b3d3e0d491c2321817e21bd6f1944194665df`
  after exact `af7f523dae68366f774400d4bddfb08dbfe25e0b` UI and verify success;
  CI 38186484683, verify 12m58s. Plan 0057 prepared receipt is now effective.
- Claimed 0058 on `issue-306-provider-review-scope` before code. Existing PR
  storage/session keys omit provider; establish explicit native ownership before
  cached evidence starts create new runs. Preserve legacy rows/contracts and
  resolve transactional publication against actual owning provenance.
- #357 maintainer closure remains preserved; retained work tracked by #306.
  ADR 0020/full epic/platform/profile/install completion not claimed.

## 2026-10-12 — plan 0058 provider ownership foundation, local gate complete

- Native provider/length-framed repository/PR keys isolate saved rows and runtime
  lookup/cancellation. Library exposes explicit native helpers; legacy CLI/Tauri
  storage/session formats remain unchanged. New preview starts stay unavailable.
- Native reads combine only proven legacy and owned native runs, reject foreign
  or duplicate ownership, and omit ambiguous/unproved legacy conversations.
  Native conversation updates do not duplicate legacy mutable run ownership.
- Backend discovery/output/history/findings/reply/fix evidence readers, finding
  staging and publication source signatures use scoped projections. Confirmed
  finding publication resolves exactly one owning row inside the write lock;
  full SQL/JSON scope and feedback provider checked. Legacy byte preservation,
  corrupt scope and two-provider stdio restart exercised in owned fixtures.
- Root typecheck/123 tests plus tooling/lint/protocol, UI typecheck/172 tests/
  1057 asserts/115 unchanged frames, Rust 704 core plus two existing ignored/
  54 stdio/six CLI/five onboarding/three bin tests, fmt/strict all-feature/all-
  target Clippy/no-GUI, Archgate 12/12 and docflow audit pass. Final targeted
  transaction/publication tests rerun after SQL scope predicate change. No React
  changes, no Norn review, real credentials/provider writes or profile reads.
- Two existing stdio stale-target expectations initially failed after projection;
  foreign explicit IDs now retain targetStale without exposing foreign payload.
- No #306/epic/ADR completion or exact-evidence review-start delivery claimed.

- Prepared plan 0058 receipt for feature `48cafd4e21e144dc6cbad1f049fc717866d560fb`;
  effective only after exact-head CI and verified squash merge. ADR Accepted.

- Pre-merge compatibility inspection found legacy stale-review cleanup could
  delete native rows because its keep keys omit provider. Limit that legacy
  cleanup to legacy keys. Owned regression covers matching/nonmatching/empty
  keep lists and both native providers. Complete Rust gate rerun: 705 core plus
  two existing ignored, 54 stdio, six CLI, five onboarding, three bin tests;
  strict all-feature/all-target Clippy passes. Replacing PR #369 head requires
  fresh exact-head CI; the prior running head cannot authorize merge.

- A further pre-merge seam check found the old inline reply could recover an
  ambiguous raw legacy thread despite projection removing it. Require proved
  conversation presence and exact legacy run/conversation/configured provider/
  clone before the old inline handoff. Native-owned/replaced context stays read
  accessible but cannot silently enter that legacy agent seam. Full qualified
  inline execution follows in 0059; no capability retirement is inferred.
- Owned real-engine stdio regression covers ambiguous legacy thread, native
  conversation replacing legacy, native-owned run and mismatched first provider
  configuration. Fake CLI marker proves no spawn; saved bytes remain identical.
  Existing prompt/reference/resume tests pass. The first regression attempt used
  simulated review.start (and could not prove handoff); corrected fixture pins
  real engine with owned fake CLI. Guard checks configured clone directly and
  never invokes clone autodiscovery during provenance preflight.

- Final handoff gate passes: 705 Rust core plus two existing ignored/55 stdio/
  six CLI/five onboarding/three bin tests, strict all-feature/all-target Clippy,
  fmt/no-GUI, Archgate12/12 and docflow audit. Frontend gate remains unchanged
  (root123, UI172/1057 assertions/115 frames), no React source edits.

## 2026-10-12 — claim isolated plan0059 while predecessor CI runs

- #369 final head `0e828f7c0eb6a02579a4450eb2fce974605a567e` is under
  CI38190456486; earlier6433 head gate passed but cannot authorize final merge.
- Claimed0059 before code in isolated `/private/tmp/norn-provider-inline-engine`
  on `issue-306-provider-inline-engine`. Same single agent; predecessor code/CI
  head remains fixed. Native context/profile/inline reply/fix integration reuses
  shared core. No fresh-preview activation until durable evidence integration.
- Sequential PR integration remains mandatory: verify/merge369 first, rebase
  isolated branch onto actual squash baseline, audit, then open next PR.

## 2026-10-12 — provider-qualified execution implementation and predecessor merge

- Verified #369 exact head `0e828f7` against successful UI/verify CI38190456486;
  squash merged at `68c69399cc879b29873846a65fd7c79ad42baee9`, 01:22:43Z.
- Implemented explicit native provider/clone context, isolated initial/reply
  execution, proved conversation inheritance without legacy run copies,
  bounded review.start profile handoff, mandatory analyzer checkout fences,
  distinct optional diff-base pins and scoped approved filesystem state.
- Owned real stdio tests cover native/legacy conversation inheritance,
  ambiguous provenance, explicit/default/unknown profiles on both providers,
  canonical clone selection and clean pinned mandatory evidence. Adding final
  concurrent cancellation proof before final gate/PR; no shipped claim yet.
- No Norn review, live provider writes, private data/profiles or public releases.
  Fresh-preview activation and the remaining parity/hardening/package/cutover
  tasks remain outstanding. Single agent, authorized sequential PR integration.

## 2026-10-12 — prepare provider-qualified inline engine PR receipt

- Implementation `4db90405f7d80692c54ea491facc97de32ec8110` rebased onto
  #369 squash `68c6939`; tree comparison is identical. Number0059 collision
  audit passed against fresh origin/main.
- Final local gate: root123+tooling/typecheck/lint/protocol; UI172 tests,
  1057 assertions, 115 unchanged frames; Rust706+two ignored/58 stdio/six CLI/
  five onboarding/three bin tests; strict Clippy/fmt/no-GUI/Archgate/audit.
- Added real simultaneous same-identity GitHub/Bitbucket sessions in separate
  clones: both fake agents start; cancel GitHub only, Bitbucket succeeds;
  legacy bytes preserved. Mandatory local analyzer runs only on clean pinned
  checkout. Scoped fix-state lookup preserves each provider's lifecycle.
- Prepared plan/done receipt is conditional on exact-head CI and verified PR
  squash merge; full ADR0020/#306/epic remain incomplete.

## 2026-10-12 — fix provider cancellation fixture CI identity

- #370 CI38192754060 passed UI but failed verify at the second owned clone's
  Git commit. Linux runner has no global identity; clone does not copy the
  original repository-local user.name/user.email. Production code was not the
  failing operation.
- The fixture now declares a local synthetic name/email in the cloned repo.
  Targeted independent-cancel test passes with GIT_CONFIG_GLOBAL=/dev/null and
  GIT_CONFIG_NOSYSTEM=1. Running the whole stdio suite in that same isolation
  and strict Clippy before push; exact new-head CI/merge still required.

- Follow-up verification completed: all 58 stdio tests pass with personal/system
  Git config disabled, strict all-feature/all-target Clippy passes. Existing
  production/local root/UI/core gates remain unchanged; new PR head CI required.

## 2026-10-12 — claim isolated durable provider evidence plan0060

- #370 fixed head `789bd0347eed8b56bdc6ef79ca9f50c1966dc5e2` is in
  CI38192754060. Local gates passed; no merge/shipped claim yet.
- Claimed0060 before code in `/private/tmp/norn-provider-evidence-start` on
  `issue-306-provider-evidence-start`, single agent. Next integration persists
  exact captured provider evidence and connects native fresh-preview start,
  saved/restarted readers and replies with optional explicit clone.
- Verify/merge #370, rebase onto actual squash and audit before next PR.
  No Norn review, provider writes, private credentials/profiles or release.

## 2026-10-12 — verified predecessor merge during durable evidence integration

- #370 final `bc7c2ca69c5b41de7012d4561805aba1878a389d` passed both
  CI38193561843 jobs. Verified squash merge at
  `a529463b523f7e8039068ccc59fe9243baba0e36`, 02:06:45Z. Earlier failure
  was owned second-clone Git identity; declared synthetic local identity and
  verified all58 stdio without global/system Git config before repush.
- Durable evidence work remains isolated/unmerged. SQLite schema18 separates
  exact raw patch from versioned bounded metadata, content-addresses full scope/
  provenance and pins, and follows its native owning row via cascading FK.
  Initial owned tests pass deduplication/restart/hash/cross-scope rejection and
  consistent backup/cascade cleanup. Connecting saved readers/replies and UI.
- No shipped/full parity claim; rebase/audit and all final gates still required.

## 2026-10-12 — implement durable provider evidence plan0060

- #370 verified merged at `a529463b523f7e8039068ccc59fe9243baba0e36`; isolated
  plan0060 rebased onto the actual squash baseline. Collision audit passes.
- Versioned/content-addressed SQLite exact capture, distinct approval/diff pins,
  explicit metadata recheck, native no-clone initial/reply, restored saved native
  readers and bounded UI source/profile/generation fences implemented.
- Owned fake-agent initial execution and real stdio restart/reply prove exact
  patch bytes for GitHub/Bitbucket without clone/current PR. Owned HTTP recheck
  rejects drift without recapture. Existing legacy runs remain byte-preserved.
- Updated migration assertion to schema18; first complete run's other storage
  failures were poisoned-lock fallout. Corrected full suite: 712 core plus two
  existing ignored/59 stdio/six CLI/five onboarding/three bin; strict all-feature/
  all-target Clippy/fmt, core all-target/no-GUI, root123+tooling/typecheck/lint/
  protocol/Archgate17, UI177/1088 assertions/typecheck/117 native frames pass.
- Native F6 profile input includes wide/narrow frames and paste/cancel/default
  keyboard tests. React Doctor reports no changed-file issues, score64 baseline.
- No Norn review, live provider writes, private profile/data, releases or full
  parity/ADR0020 completion. Next: exact-head CI and verified PR squash merge.

## 2026-10-12 — claim native external provider browser plan0061

- PR #371 exact fixed head78ca5d0 UI passes; verify still running, no merge claim.
- Claimed0061 before code in isolated `/private/tmp/norn-provider-external-browser`.
  Existing native opener reused; UI supplies full PR identity, never a URL.
  Bitbucket UUID aliases require bounded native metadata resolution.
- No Norn review/provider writes/private profiles/releases. Integrate only after
  verified #371 merge, actual squash rebase/audit, local gates and exact next CI.

## 2026-10-12 — implement native external provider browser plan0061

- #371 verified squash merge `3322ff050c8d7c6bc115c01c9341cb0594cb4730`,
  03:00:52Z after exact78ca5d0 CI38197325446 both passed. Plan0061 rebased onto
  actual squash. Main checkout synced to that baseline; user file untouched.
- Full-scope strict provider.browser bridge, canonical native URLs, bounded
  authenticated Bitbucket UUID aliases, existing launcher/read gates, shutdown
  fencing and native b input implemented. No UI URL/native shell interpolation.
- Owned HTTP/real stdio prove both-provider offline slug routes/UUID/path/origin,
  disabled launcher and responsive control. UI DTO/input/paste/late/dispose tests
  and two new disabled-opener frames pass.
- Local gate: root123+tooling/typecheck/lint/protocol; UI181/1117 assertions/
  typecheck/119frames; Rust713 core+two existing ignored/60stdio/six CLI/five
  onboarding/three bin, strict all-feature/all-target Clippy/fmt, all-target
  no-GUI, Archgate17/audit. React Doctor reports no changed-file issues,
  aggregate score62; no native prop rules suppressed.
- Remaining explicit PR approval/profile catalog/editor/full parity/recovery/
  platform/install/cutover unwaived. No Norn review/live provider writes/private
  data/releases. Exact next-head CI and verified PR squash merge still required.

## 2026-10-12 — claim native explicit PR approval plan0062

- #372 fixed648e253 UI passed; CI38198661868 verify running, no merge claim.
- Claimed0062 before code in `/private/tmp/norn-provider-approval`, single owner.
  Scope: retained human PR approval, read-only preview, explicit native actor/
  pin/scope confirmation, durable pre-POST journal and read-only uncertain-write
  recovery. No clone/AI prerequisite or mixing with staged comment publication.
- Rebase after verified predecessor squash; local gates/exact next CI/verified
  PR merge required. No Norn review/live provider writes/private profiles/releases.


## 2026-10-12 — Prepare explicit native human PR approval (plan0062)

- #372 verified merged84921b7 at03:29:22Z after exact648e253 CI38198661868 passed.
  Rebased plan0062 onto its actual squash; no numbering collision. Native actor/
  immutable repository/pin proof, single-use explicit confirmation, pre-POST
  FULL-sync journal and GET-only uncertainty recovery in implementationd503f9b.
- Root123+tooling/typecheck/lint/protocol/Archgate17; UI190/1,181assertions and127
  native frames; Rust725+2ignored/61stdio/6CLI/5onboarding/3bin; strict all-feature
  all-target Clippy/fmt/core all-target/no-GUI; metadata/links/INDEX/audit passed.
  Final owned journal6 includes consistent SQLite backup; final native input9
  passes64assertions both providers/wide/narrow/scroll/paste/repeated confirmation.
- ReactDoctor62: complexity corrected, remaining17 unknown-DOM-property warnings
  are native OpenTUI text props with type/renderer proof, not suppressed.
- Prepared plan/done receipt namesd503f9b; effective only after exact-head CI and
  verified merge. #306/epic remain open, ADR0020 Accepted; preserve maintainer
  closures. No Norn review/live provider writes/private profiles/public releases.


## 2026-10-12 — Claim native review profile catalog (plan0063)

- #373 is open at fixed2db1dcb; CI38202396806 UI passed, verify in progress.
  No shipped claim until actual merge. New isolated branch starts at that fixed
  predecessor and must rebase on the verified squash before its own PR.
- Port the desktop's actual read-only catalog/default selection, using exact
  native policy/target/path ownership. Do not invent a profile rewrite API:
  retained policy-file editing belongs to #359; canonical prompt editing exists.
- Claim written before code; owned fixtures only, no unrelated root .norn.yaml,
  private credentials/profiles, live provider writes, Norn reviews or releases.

- 2026-10-12: PR #373 fixed2db1dcb passed exact-head CI38202396806 and was verified squash-merged at 04:23:57 UTC as ac077cd48ff96d6a6248f1ee941dd7904e2d9ce5. Plan0062 completion is effective. Rebased the claimed plan0063 onto the actual squash. Implemented strict bounded native profile catalog, exact provider/local ownership, shared actual Rust policy validation/default and F6 selection with full-target/generation fencing, no raw config/commands/credentials. Native local override semantics and existing organization verified-cache maintenance are explicitly retained rather than redefined. Owned policy, signed-layer, real Git, stdio/fake-agent and native input/frame proofs; full gates pending final receipt. No Norn review, private profiles, live provider writes or releases.

- 2026-10-12: Plan0063 implementation8e57d2a local gates passed: root123+tooling/Archgate17, UI197/1225/nativeframes135, Rust731core+2existingignored/62stdio/6CLI/5onboarding/3binary, strict Clippy/fmt/noGUI/protocol/docflow audit. ReactDoctor64 with3 valid native OpenTUI text-property warnings; no suppression. Prepared done receipt remains contingent on exact-head required CI and verified PR squash merge. #306/epic/ADR0020 remain open/Accepted; explicit policy editor is #359 and actual local branch/base/layers are #358.

- 2026-10-12: While inventorying #358's exact local source proof, found that the reused legacy matches_remote helper compared a substring. Tightened plan0063 catalog ownership to native parsed provider/owner/repository equality, including local requests, and added owned suffix/foreign-host/nested-path negative proofs for both providers. PR#374 prior fixedbd9a511 checks are superseded; a new exact-head CI run is required before merge. No other behavior or legacy helper consumers changed.

- 2026-10-12: Final374 e0a1cce UI job114667814181 failed one existing F2 test: it asserted a diff after eight zero-duration render turns, before the asynchronous native snapshot patch/hash completed. All catalog/new Unicode tests passed. Added an owned20ms patch delay and bounded wait on actual diff-loading completion, preserving error/exact-content/ownership/no-write assertions rather than increasing arbitrary sleeps or rerunning away the failure. Required UI/full integration gates and a new final-head CI remain pending.

- 2026-10-12: Claimed plan0064 for #358 qualified local immutable snapshot IPC foundation before code, isolated issue-358-local-snapshot-ipc based on pending374 finalfixe0a1cce. Native local controls/engine/history/Pierre remain the next required #358 slice. Shipped baseline inventory starts from native Local/B mode collectors, distinct layers, actual canonical checkout and current-repo semantics; no arbitrary Local SHA/cwd filesystem authority. PriorCI38204902743 superseded; final exact-head CI required; verified merge/rebase required before next PR. No delegation/Norn review/private data/live provider writes/releases.

- 2026-10-12: PR#374 final3dd4fb0 passed both required CI38205709559 jobs; verified squash-merged a3b6e4dcdd62278fb3cd5e781df4f8597e80b5c6 at05:09:19UTC. Plan0063 is shipped; root main synced. Rebase0064 onto actual squash with owned WIP safely stashed; preserve all completed gates and remaining local/parity/cutover requirements.

- 2026-10-12: Plan0064 restored after rebase onto verified374 squash a3b6e4d; own stash source verified byte-for-byte, four source/DTO tests passed again, own stash removed while preserving pre-existing user stash. WIP implements strict empty-struct Current variant (Serde unit variants accepted extras despite deny_unknown_fields), configured exact parsed origin/canonical root, startup-scoped current remote fallback, provider/root-separated opaque identity, bounded ref pages and native same-deadline collector helpers. Capture/persistence/IPC/consumers are still pending; no #358 completion or PR yet.

- 2026-10-12: Plan0064 local gates passed on implemented immutable local foundation: 744core+2existingignored/64stdio/6CLI/5onboarding/3binary; root123+tooling/Archgate17; UI201/1254/nativeframes135, strict Clippy/fmt/noGUI/protocol/docflow audit. Owned12 local tests include identical clones, real9MiB patches, layers, quota/backup/cascade; actual immediate-cancel/EOF and two-provider restart stdio passed. Root fixture first exposed schema accepting traversal while native/client refused; schema now enforces safe relative paths and full root gate passed. ReactDoctor100, no findings in three changed TS files; no setup/suppression. Schema20 compatible backup/matched rollback remains #308. No claim of completed #358 UI/engine/history/Pierre or full cutover; next slice required.

- 2026-10-12: Implementationd7ba477 passed all local gates. Prepared plan0064 done receipt after verified predecessor374/main rebase and collision audit; it is ineffective until own final-head CI and verified squash merge. #358/#306/#294 remain open and ADR0020 Accepted. Next required slice connects immutable snapshots to actual local controls, native review payload/history/cancel/findings and Pierre.

- 2026-10-12: PR#375 fixed42dea41 passed both required CI38209164847 jobs (UI05:43:35UTC, verify05:57:36UTC); verified squash-merged5e4e1b293b7044c693b5f8f2e8308f59d810cebf at05:58:26UTC. Plan0064 is shipped and main synced. Claimed plan0065 before code on isolated actual-main branch issue-358-local-review-lifecycle: exact captured local native engine/payload/stream/cancel/history/findings/output; actual terminal controls and pinned preview/Pierre follow. #358/#306/#294 remain open; no full local or cutover claim.

## 2026-10-12 — plan0065 native qualified local lifecycle implementation

- Single writer, following actual#375 squash5e4e1b293b7044c693b5f8f2e8308f59d810cebf.
  Claim7622029 preceded code. No Norn review or live provider/agent/profile access.
- Qualified saved source/root/actual provider enters existing native inline engine;
  explicit layer payload, same-comparison canceled recheck, actual profile/policy,
  opaque local parent/high-bit ID and retained operation cancellation. Bounded SQL
  store reads; scoped run history/findings/output/conversation contracts and client.
- Eight real owned Git/fake-native-child stdio cases cover both providers, wrong
  configured default, roots/clones, layers/base, parser finding ambiguity, restart/
  missing checkout, run pagination, failure/oversize, cancel/EOF/preflight and local
  publication decode rejection. Canonical output is not truncated; resume IDs are
  omitted from bounded conversations and compact Claude progress envelopes.
- Observed failures: early-return read counter leak delayed EOF (RAII fixed);
  cancel test discarded a terminal event before its control response (collect both);
  compact result envelopes leaked session metadata (formatter+unit fixed);
  high-bit publication frames are rejected before dispatch, not correlated responses
  (test follows actual codec semantics); required-preflight test revealed provider
  clean-OID restriction incorrectly applied to local opaque snapshots (separated);
  strict Clippy large target enum (boxed captured context). Preserve failed logs.
- Final gates:745 core/72stdio/6CLI/5onboarding/3binary+2existingignored; noDefault
  allTargets, strict allFeatures/allTargets Clippy, fmt/noGUI; root123+tooling,
  lint0errors/3existinginfos, protocol, Archgate17, audit PASS with existing0011/0013
  coverage advisories; UI206tests/1277assertions/135 unchanged nativeframes;
  ReactDoctor100/no findings in2changedTSfiles. Exact-head CI and PR merge pending.
- This is backend only. #358/#306/#294 stay open and ADR0020 stays Accepted;
  actual local terminal controls/Pierre/image consumers/full workflow are next.

- Prepared plan0065 receipt after all local gates. Implementationd3c670bae36dd4e7317c95cfc41cf9bc9cda1f51;
  origin/main remains actual3755e4e1b2; no number collision. Receipt ineffective
  until exact-head required CI and actual squash merge. No child/epic closure.

- Fresh GitHub timeline correction: #306 was maintainer-closed at01:22:44UTC,
  no associated commit. Earlier open-state references were stale; preserve the
  closure without inventing child parity/cutover proof. PR#376 body removes an
  unintended GitHub closing reference to#358 caused by a negated closure sentence.
  Actual #358/#294 remain open; required child implementation continues.

## 2026-10-12 — verified#376 merge, plan0066 claim

- #376 actual squash4644a03739c78fbe7e687edbb7a03d54f8366c13 at07:12:32Z,
  final19610d53dc067148367371ee56dd346bfcab2a3f both requiredCI38214976426
  SUCCESS: UI06:56:27Z and verify07:10:53Z (14m51sjob). No auto-closure links;
  #358/#294 remain open, maintainer-closed#306 preserved. Root main ff synced;
  unrelated .norn.yaml and existing5f69c995stash preserved without reading.
- Plan0065 receipt effective only from this verified merge. Proof remains eight
  actual owned native child/stdio scenarios and full gates listed in its receipt.
  API does not establish actual UI/Pierre/local full workflow parity by itself.
- Plan0066 claimed before source edits in isolated branch/worktree on actual376.
  Existing shell gets exact local source/base/layer capture, snapshot consumers,
  native lifecycle/profiles and owned wide/narrow input/frames. Following pinned
  preview/Pierre/saved history/unsafe fallback/full workflow proof is still required.
  No new0066implementation or gate success claimed yet; no Norn review/delegation.

## 2026-10-12 — plan0066 local controls implementation prepared

- Existing Shell now explicitly captures backend Current/configured sources,
  listed working/branch bases and independent layers; keeps Rust full refs/runs
  authoritative for files/diffs/findings/history/conversations/profile/start/cancel.
  F4/source form and B/base cycling never launch an agent; pasted actions are
  ignored. Layer ambiguity asks before opening a patch. Qualified clipboard and
  source/nonce/generation/operation fences retain selected evidence/run identity.
- Native local.review.profiles uses live qualified source/root/provider proof and
  shared policy/default/errors. Local capture/options/recheck/profile transport
  deadlines45s exceed native Git30s; actual fixture subprocess31s response passed.
  Captured pins/owner/warnings/layer meanings/local unpublished context reach
  the shared native payload without untracked/excluded content.
- Local gates:745core/73stdio/6CLI/5onboarding/3binary+2existingignored;
  corealltargets/strict allfeaturesalltargetsClippy/fmt/noGUI; root123+tooling/
  typecheck/lint/protocol/Archgate17/audit; UI219/1346/147 native frames.
  Nine owned native scenarios and eight local Shell/input cases plus four
  aggregate/client cases and the slow production transport check passed.
- Doctor65 has9 no-unknown-property warnings solely on truncate, a supported
  OpenTUI TextBufferOptions property (local installed.d.ts and native frames).
  This custom renderer has no DOM property problem; no diagnostic suppression.
  Existing frame changes are only F4/footer; twelve new local/profile/error/loading
  frames added. Failures were retained and corrected: TS layer tuple/narrowing,
  nullable HEAD formatting, test Escape decoding delay, fixture diff routing and
  qualified snapshot-derived ID. No gate failure silently counted as success.
- #358/#294 remain open; maintainer-closed#306 preserved. Pinned image/Pierre,
  saved snapshots/global history/legacy fallback/full native UI+Rust workflow proof
  remain required. ADR0020 Accepted; no Norn review, delegation, private data,
  provider write, actual agent, release, or user stash mutation. PR/merge pending.

- Source db8bc372d4c03f2ad5bcfaacdb79e9653e36d761 committed after full gates.
  origin/main still4644a03; number audit passed with existing0011/0013 advisories.
  Prepared0066receipt is conditional on exact final-head CI and actual squash;
  no completed child/epic claim. Additional source tests did not alter Rust gates.

## 2026-10-12 — verified377merge, plan0067claim

- #377 verified squash63970f499ae1a16c66e02438ec7fdd59f26d4cb5 at08:12:58Z;
  both requiredCI38220689561 SUCCESS on finalc4278e1efe5ae1f4f0dffd5d6c0d6b3d54fdc1c8.
  No automatic issue closures; root main ff synced, user stash/untracked untouched.
  Plan0066receipt now effective. Owned gates:745core/73stdio/6CLI/5onboarding/
  3binary+2existingignored; root123+tooling/Archgate17; UI219/1346/147frames;
  typecheck/lint/protocol/audit/strictClippy/fmt/noGUI. Doctor65 false positives
  on supported custom-renderer truncate remain explicitly recorded, unsuppressed.
- Plan0067 claimed before code on actual377 for pinned local raster/Pierre/saved
  snapshot consumers, unsafe legacy fallback rejection and actual owned native
  UI+Rust full workflow proof. Draft prepared while377CI; no sibling source
  implementation before actual merge. #358/#294 remain open; preserve306closure
  and ADR0020 Accepted. Broader retained tasks are not silently inferred done.

## 2026-10-12 — plan0067 pinned local consumers, implementation gates

- Shared Rust raster validator; durable format2 independent layer/side captures
  with byte/metadata identity, stable collection and native stale recheck.
  Format1 identity and readonly access preserved; absent/unsupported/legacy/quota
  states never fetch mutable bytes. Bounded secure file handles reject symlink
  traversal; Windows opened-handle canonical fence awaits actual Windows CI.
- local.snapshot.image/browser/list use full source/provider/root/snapshot identity.
  Pierre receives exactly one saved layer, including old rename paths and missing
  sides. Durable catalog paging verifies ownership/hash/continuity and restores
  actual runs without checkout. F7 input/paste/modal fences and native wide/narrow
  saved and image frames. Public arbitrary legacy Local SHA routes reject before
  side effects; explicit configured provider working-tree contract remains.
- Native real OpenTUI input/production transport/Rust/owned fake-agent workflow
  passes on macOS: source/base/layers, pinned images, CLI, actual review/findings,
  explicit layer choice, cancel and restart into saved run after root replacement.
  Three-OS CI lane added; configuration is not runtime proof before exact-head CI.
  Found and fixed operation resync reopening layer selector and intercepting x;
  choice retained only within exact snapshot/run/finding/anchor. Regression passed.
- Local gates:753core+2existingignored/75stdio/6CLI/5onboarding/3binary;
  root123+tooling/typecheck/lint/build/protocol/Archgate17/audit/noGUI;
  UI228/1433/165frames/typecheck/Storybook build. Strict allfeatures/alltargets Clippy and fmt passed.
  Doctor65/11 warnings target supported OpenTUI flexShrink/truncate on saved panel;
  installed Renderable.d.ts/TextBuffer declarations and native frames establish
  custom-renderer DOM false positives; no suppression or dependency installation.
- Failures retained and corrected: renamed-image fixture had identical deleted
  bytes causing Git to select another valid rename source; unique fixture fixed.
  Root lint formatting fixed; sandbox loopback-denied full Rust tests repeated
  with approved local server access and passed. Invalid native policy blocks agent
  even with prompt override; qualified payload precedence test states this
  explicitly. Legacy private cwd adapter is still present, public access fenced.
- PR/source commit/own final-head CI/actual squash remain pending. #358/#294 open;
  maintainer305/306/357 closures preserved, ADR0020 Accepted. No user data/stash,
  credentials, provider write, actual agent, Norn review, delegation or release.

- Source e34758fc77d1fef309e11321b37ea233fa5a1d30 committed after applicable
  local gates. origin/main remains actual37763970f4; audit no0067collision.
  Prepared0067receipt is conditional on final-head CI/all3native lanes and actual
  squash. No completion/closure is inferred before that event.

- PR378 opened on receipt8479693672104cf6fcc3d2481e60d0e48a7a24ac.
  CI38232903789 UI passed; native Linux/macOS exposed harness readiness race:
  operation succeeded/history remained visible before run/finding/thread refresh.
  Now waits for all actual saved state, rather than skipping the assertion.
  Windows cleanup EACCES masked the scenario error: preserve original diagnostic,
  wait for actual backend exit after shutdown and bounded owned-root cleanup.
  Corrected native gate and UI typecheck pass locally; own new-head CI pending.

- New-head CI38233690131 Linux/macOS native workflows and UI passed.
  Windows exposed actual cancellation defect: review.kill_process always invoked
  /bin/kill, marking cancellation failed and leaving the owned fake agent alive.
  Add Windows API bounded process-tree inspection and terminate only the recorded
  agent/observed descendants, no shell/PATH/taskkill dependence. Already exited
  processes are handled explicitly. Unix behavior unchanged. Native Windows CI
  remains required; observed-tree cancellation does not claim JobObject-level
  future descendant/race guarantees (retained hardening).

## 2026-10-12 — verified378merge/358closure, plan0068claim

- PR378 actualsquash8cdd00768eba9b162dd08d6b15717188e77dd455 at10:39:13Z,
  finalb906071101ba65347b3105358a7fb992a215bb97. All five CI38234834238
  jobs SUCCESS; actualLinux/macOS/Windowsnativeworkflow andUIpassed.
  Generalverifyjob repeated on samehead after12m slowAzureUbuntuHTTPdownloads;
  preserved fourpassjobs, no bypass. Allfrontend/Rust/architecture gatespassed.
  Normalcancel completed beforeforceattempt409; retryonlyverify114767629843.
- Rootmain ff synced before nextcheckout/claim. Plan0067receipt effective;
  #358closed10:40:10Z afterfreshactualacceptancescopeaudit and fullproofupdate.
  Its broader localreply/fix/globalhistory are retainedfollowups, not added to
  actual358scope. Maintainer305/306/357closures preserved; ADR0020Accepted.
- Native Windows API cancellation passes actualowned gate; secureopenedhandle
  images/cancel/savedrunrestart verified onWindows. Observeddescendant cancellation
  is not a JobObject race guarantee. Shared globalhardening remains307.
- Plan0068claimed beforecode on actual378: boundednative readonlyrepository
  extraction/qualifiedIPC, then native tree/find/editor follow-up for359. Draft
  only while378CI; no nextsource before actualmerge. Singlewriter, no Nornreview,
  delegation, private data, actualagent, providerwrite, release or userstashchange.


## 2026-10-12 — plan0068 native repository reader implementation gates

- Extract shipped readonly DTO/parsers and native Git/filesystem readers behind
  full provider/workspace/repository/root identity. Desktop readonly commands
  delegate to bounded native services, rejecting ambiguous legacy provider
  selection while preserving their configured-path fallback. Branch/worktree
  and external-editor adapters remain for their retained follow-ups.
- Six strict repository.explore methods + generated/shared contract fixtures,
  two bounded workers, 20s aggregate deadlines, EOF/shutdown cancellation,
  secure shared file handles and production typed semantic client validation.
  Inventory continuation pins raw tracked paths/status/index/HEAD and filters;
  content/diff continuation pins complete UTF8 bytes. Binary/large/missing/
  deleted/unborn states are explicit. Blame retains full bounded author/commit
  text without the legacy delimiter ambiguity; find maps lowercase matching
  back to original UTF16 positions without a full per-character map.
- Owned10native tests cover state/rename/conflicts, literal/Unicode/control names,
  symlink confinement/internal links, limits/unborn/binary, cancellation,
  delimiter-containing messages,10k-line blame and provider/clone/version drift.
  Fourclienttests + realstdio all-six-method read/stale/control/shutdown test.
  Native workflow actual macOS PASS with all six methods including untracked
  no-index and binary; own exacthead Linux/Windows CI remains required.
- PASS local:763core+2existingignored,76stdio,6CLI,5onboarding,3binary;
  root123+tooling/typecheck/lint/build,Archgate17,audit/noGUI;
  UI232/1454,165unchangednativeframes,typecheck/Storybook; strictallfeatures
  Clippy/fmt. Doctor100/noissues on changed transport files, no project install.
- Initial owned test roots had macOS temp path aliases; fixtures now create
  under the canonical temp directory, matching the service's canonical-root
  authority. Stdio traversal test expected a correlated error although malformed
  frames are dropped by the established decoder: corrected proof explicitly
  rejects any such response and verifies the following valid control request.
  Initial lint/import/control-regex errors fixed without suppression; allgates
  green. No Norn review, private files, provider calls, credentials or realagents.
- Prepared source/receipt are not actual completion. Plan0068 only becomes
  shipped after own exact-final-head CI and actual squash to currentmain.
  #359 stays open until tree/find/editor UI proof; ADR0020 stays Accepted.

- Source commit `48271cfefd0beb9374dc71775fccf513f1846346` prepared; conditional plan0068 receipt effective only
  after exact-final-head CI and actual verified squash. Next slice not claimed.

- Automatic approval review rejected pre-merge git mv todo-to-done because it
  could imply premature completion despite conditional wording. No move or
  workaround performed. Safer flow: retain0068 in todo for the source PR, then
  reconcile actual merge proof and move only after verified integration.


## 2026-10-12 — verified379merge, plan0069claim

- PR379 actually MERGEDb15d80bf2c2f275e22c68a100fd1ca2124c4542d at11:48:19Z.
  Five CI38243490575 checks SUCCESS on766fa6c084f24079580eacf0fa91cf948f37838c;
  actual native Linux/macOS/Windows explorer reads +review/cancel/restart PASS.
  Source48271cf and pending-evidence766fa6c preserved; no retry/bypass.
- Reconcile0068 only now, after actual source completion; no pre-merge move or
  workaround of automatic rejection. Rootmain ff synced; privatefile/stash intact.
- Claim0069 before code in isolated issue-359-native-explorer-ui onactual379:
  paged native tree, bounded previews, complete blame/find presentation, escaped
  controls, generation/focus/return and explicit configured-path semantics. Editor
  lifecycle remains explicit follow-up before359closure. #359/#294 open;
  maintainer305/306/357closures preserved; ADR0020 stays Accepted untilcutover.

- Editorial baseline correction before code: configured_repo_path only trims
  explicitlocalPath; no clone-directory fallback exists there. Remove the draft's
  inferred discovery scope; retain exact configured source/path proof.

## 2026-10-12 — plan0069 native explorer source prepared

- Independent F8 ExplorerStore/panel over real qualified Rust readers; 32-row
  inventory, explicit filters/folders/status, pinned32KiB content/diff windows,
  viewport and horizontal navigation, complete blame detail keyboard scrolling,
  bundled native Markdown source/render toggle and direct UTF8-byte find jumps.
  Full context/generation fences, escaped display and inert/256-byte safe paste.
- Matched strict protocol now includes original byte match bounds alongside
  UTF16 positions; backend capabilities fence incompatible UI/backend pairs.
  No explorer mutation/editor/agent/provider publication authority introduced.
- Real owned OpenTUI/production stdio/Rust workflow covers F8/filter/file/modes/
  find/return preserving selection/focus/scroll. Added22native Storybook scenes
  at120/50cols; Markdown waits for actual heading/body rather than blank startup.
- Local763core+2existingignored/76stdio/6CLI/5onboarding/3binary PASS; allfeatures
  Clippy/fmt, root123+tooling/typecheck/lint/build, protocol/noGUI/Archgate17/audit
  PASS. UI240tests/187frames/Storybook/typecheck PASS. React Doctor changed UI
  reports no diagnostics, score66. Final exact-head3OS CI/actualmerge pending.
- Keep0069 in todo until actual source squash. #359/#294 remain open for editor
  lifecycle/finalparity and remainingepic. ADR0020 remains Accepted; preserve
  maintainerclosures/rootprivatefile/userstash. No Norn code review/delegation.

## 2026-10-12 — verified380merge, plan0070claim

- PR380 actually MERGEDd623231b16215a8d828764bfdfe48349c6d73254 at12:43:36Z.
  All5CI38249399379 SUCCESS on exact99b33bdbe73befa2b2ef25be30171f657cb8bea3;
  real3OS native F8/filter/file/content/diff/blame/find/return PASS. No retry/bypass.
- Reconcile0069 done only from actual source completion and update parity.
  Claim0070 editor/real terminal lifecycle before code on actual380main.
  Rust validated platform adapters and child ownership; native suspend/resume
  and owned POSIX PTY/Windows ConPTY proof, no protocol stdio as editor terminal.
- Rootmain ff synced, privatefile/userstash intact. #359/#294 remain open;
  maintainerclosures preserved, ADR0020 Accepted untilcutover. No delegated
  agent/Norn code review/privateprofile/live provider writes/releases.


## 2026-10-12 — 0070 native editor implementation, pending source merge

- Explicit F8 o/O actions use full provider/root/path context, 30-second one-use
  authorization and fresh file/integration checks. Native GUI priority/system
  adapters replace desktop shell interpolation; terminal child never consumes
  protocol pipes. Owned process groups/Windows jobs, status/cancel, foreground
  restore and graceful transport shutdown protect the terminal's single reader.
- Actual owned macOS PTY success/nonzero/Ctrl-C/backend-death tests pass with
  a compiled editor changing real modes. Native GUI IPC proves both clones,
  exact Unicode/option-like argv, stale file/root, symlink escape and token
  binding/replay. Windows ConPTY/witness and Linux native proof remain pending.
- Root discovery must precede --work-tree pinning; new owned reader test proves
  nested configured browsing and unchanged strict review-capture root policy.
- Local UI249/1578,193nativeframes/Storybook, root123+tooling, Rust765+2ignored/
  76stdio/6CLI/5onboarding/noGUI, Archgate17/audit/Clippy pass including final adapter-test
  rerun; Doctor17changed/untrackedfiles no diagnostics, score66.
- Keep0070 todo and #359/#294 open pending all3OS exact-head CI/actualmerge.


### PR381 first CI head3fe8a38 — native test dependency correction

- CI38262444809 Linux built the no-GUI binaries and passed the existing owned
  native workflow; the new adapter library test failed because the unconditional
  Tauri dev dependency activated GTK/WebKit on the native-only runner.
- Move Tauri mock support onto its existing optional desktop dependency and
  include dev edges in the no-GUI verifier. Desktop/all-feature tests retain
  the test feature; core/editor tests no longer activate Tauri at all.
- Local exact adapter tests and strict all-feature Clippy pass; cargo tree with
  normal/build/dev edges reports Tauri absent. Actual3OS terminal proof remains
  pending the corrected source head. No retry/bypass or premature completion.

- Remaining first-head results: Windows failed by compiling unrelated Unix-only
  backend integration tests; scope the native adapter test to --lib. macOS
  reached real terminal smoke but opening raced startup snapshot work. Keep
  status connecting through initial reads, show busy editor controls and wait
  for real availability instead of a one-second pause. Own startup regression,
  UI250/1580,195frames/Storybook and all4 macOS PTY cases pass after correction;
  retained desktop IPC4 smoke tests and full765core/noGUI-dev-edge gate pass.

- Local general-workflow diagnosis: the retained desktop smoke command built
  every default-feature binary and overwrote target/debug/norn with a GUI-only
  build. The owned workflow child48528 then launched the GUI instead of command
  routing; terminate only that verified child of48494. No profile contents were
  inspected. Restrict the desktop smoke command to --lib (same four tests), add
  a 60-second owned CLI probe limit, rebuild the matching no-default pair before
  rerunning. Make/just recipe names and forwarding remain unchanged.

- Final correction verification: desktop --lib lane retains IPC4 and no longer
  overwrites CLI artifacts; matched no-default pair/general native workflow
  passes. UI250/1580,195frames/Storybook, strict Clippy, core765+2ignored and
  dev-inclusive noGUI gate pass; Doctor18files no diagnostics/score66. Keep plan
  pending and obtain all5 checks on the corrected head before actual merge.

## 2026-10-12 — PR381 Windows protocol-pipe follow-up

- Exact c6e35f65d24e9324a48b4c6dd8281c24c6e9abdb CI38267198082
  passed UI and full native Linux/macOS workflows, including four real PTY
  handoff/restoration cases. Windows failed its existing slow-agent cancellation
  scenario before the editor steps; ConPTY is still unobserved.
- Windows exposed an unhandled asynchronous FileSink.write EPIPE during
  shutdown. Await write/flush/end in order, catch synchronous/asynchronous pipe
  closure, keep draining output and retain the five-second owned reap fallback.
  A real owned subprocess closes both protocol handles before exit7; regression
  test checks the actual reaped exit and idempotent close without unhandled
  rejection. Ordinary request writes handle asynchronous rejection too.
- Owned native workflow uses fresh statSync for the slow-agent receipt, awaits
  backend closes before fixture cleanup and includes operation logs in failure
  diagnostics. No claim about the original Windows cancellation cause yet;
  corrected-head CI must supply proof or a more precise failure.
- Local UI251/1582/47files, typecheck, full owned general workflow and all four
  production macOS PTY cases pass after transport changes. No Rust code changed.
  Keep0070 todo, #359 open and require all five actual-head checks before merge.

## 2026-10-12 — PR381 ConPTY fixture argument correction

- Exact ae8750f9f71470cfcd09bb14b4fe70ff830e2c14 CI38268459553
  passed UI and full Linux/macOS native editor proof. Windows passed its general
  review/cancellation/restart workflow, three adapter tests and full native GUI
  IPC authorization/expiry proof, then failed before console launch: PowerShell
  consumed a bare -- at the OwnedGit function call, exposing --owned ü.txt as
  a Git option. No production terminal result was observed on Windows.
- Pass all fixture Git arguments in a named string array, retaining the literal
  separator and hostile filename. This fixes owned fixture setup; it does not
  replace or weaken any console/input/mode/death assertion. No Rust/UI source
  change. New actual-head CI must prove ConPTY and all required jobs.

## 2026-10-12 — PR381 ConPTY supervisor console handles

- Exact0930d9921253d63836382076fe07c1266db27cb6 CI38269494158
  passed full Linux/macOS native lanes. Windows completed GUI IPC and reached
  its console driver, but the standalone witness's initial is_terminal assertion
  failed before production UI launch. This is not Windows editor proof.
- The owned supervisor now explicitly opens its attached CONIN$/CONOUT$, sets
  its standard handles and retains them through the UI scenario, keeping native
  is_terminal/GetConsoleMode and exact restoration assertions. The C# harness
  retains pseudoconsole creation handles until CreateProcess has attached the
  client, instead of releasing them before launch. No production change.
- Primary API references checked: Microsoft console-handles, SetStdHandle and
  creating-a-pseudoconsole-session documentation. The real Windows runner must
  prove the corrected harness and all four production lifecycle cases.
  https://learn.microsoft.com/en-us/windows/console/console-handles
  https://learn.microsoft.com/en-us/windows/console/setstdhandle
  https://learn.microsoft.com/en-us/windows/console/creating-a-pseudoconsole-session

## 2026-10-12 — PR381 bounded local-review preflight backpressure

- Exact25e0a3c845ea9773220893c54f5ef11d6ca8d3c1 CI38270637121
  passed UI/Linux/macOS. Windows general workflow intermittently failed before
  ConPTY. Added diagnostics now prove the actual operation error: "Two local
  evidence requests are already running." The original cancellation failure
  was not adequately fixed by a fresh receipt; preserve this observed cause.
- Local review's already-bounded worker now waits at most20s for the existing
  two-reader pool, checking operation cancellation/shutdown/manager-stop every
 10ms. It performs the same fresh snapshot recheck only after owning a permit.
  No third reader, automatic operation replay or early agent launch is added.
  RAII releases the permit on every path. Two concurrent regression tests prove
  waiting/capacity/release, cancellation (also with a free slot), and timeout
  without leaks. The owned slow-child check allows45s for bounded preflight but
  still requires a real running agent/start receipt before cancellation.
- Final local proof observed: core767passed+2existingignored, dev-inclusive
  noGUI/alltarget check, stdio76, strict allfeature/alltargetClippy, root/UI
  typecheck, lint, Archgate17, full owned review/cancel/restart workflow and
  four production macOS PTY cases. Initial full test attempts were denied
  loopback binds by the sandbox; approved owned-fixture reruns passed entirely.
- Windows corrected native console supervisor still unobserved. Final actual
  head CI and verified squash remain required;0070 stays todo, #359 stays open.

## 2026-10-12 — PR381 Windows processed input and mode guard

- Exact009077ff0528e228e345b48a8692535390e1faff CI38272600909
  passed UI and full Linux/macOS lanes. Windows passed native review/cancel,
  adapter and GUI IPC, then actual ConPTY success and nonzero-editor cases,
  including literal argv/input, reaping and exact native mode restoration.
  Ctrl-C case stayed at editor-ready; backend-death was not reached by the
  previous fail-fast runner. Do not claim four-case Windows acceptance.
- Rust terminal handoff explicitly enables ENABLE_PROCESSED_INPUT, captures
  both console modes and restores them in the existing native guard. A raw UI
  mode must not silently leave Ctrl-C as input data. Original mode values were
  not recorded in the failed run, so this is a correction to verify, not a
  proven diagnosis of that failure. The fake editor now requires processed
  input before intentionally changing its own mode.
- The test runner executes every owned case and aggregates failures; each case
  retains its own cleanup and120s deadline. Windows failures print owned mode/
  receipt/alive facts, and premature UI exit fails immediately except during
  the explicitly bounded post-exit editor-reap check. No assertion removed.
- Primary SetConsoleMode documentation checked; local strict allfeatureClippy,
  UI typecheck/lint/fmt and all four production macOS PTY cases pass. New exact
  head Windows Ctrl-C/backend-death proof and all five checks remain required.
  https://learn.microsoft.com/en-us/windows/console/setconsolemode

## 2026-10-12 — PR381 UI console interrupt ownership

- Exact010bf1fbc7a5e64c4cdddeaae61f5d35d933e75b CI38274290427 passes
  verify/UI/Linux/macOS. Windows native review, GUI IPC and ConPTY success/
  error pass. Ctrl-C kills the editor and UI (C000013A); backend death returns
  UI255 instead of the test's guessed1. Four-case acceptance still pending.
- Add a UI-only fixed kernel32 SetConsoleCtrlHandler binding and reassert its
  protection before/after renderer suspension. Restore/release once at UI
  exit, including cleanup failure. Rust engine communication remains stdio;
  no Rust library, shared engine memory or caller-controlled native symbol.
- Derive backend-death expected status from the actual owned native process
  and require the UI's exact exit-cause message. Failure facts include native
  before/after console modes; all four cases and exact restoration remain.
- Local253UItests/1588assertions/48files, root/UItypes, lint,195nativeframes,
  Storybook, Archgate17, audit and four production macOS PTY cases pass.
  Online Doctor failed npm401; offline registry lookup missed its cache.
  Existing cached CLI ran directly:25files, no diagnostics; score service
  unavailable. No credentials/config read or installation/setup changes.
- Keep0070 todo and359 open until final exact-head five-job proof and actual
  verified squash. Current Rust767core/76stdio/Clippy proof remains unchanged.

## 2026-10-12 — PR381 known native backend-death cause

- Exactc7360cbb510c307ff5076d03af29ee0078eb21c7 CI38279287792
  proves Windows ConPTY success/error/interrupt, including restored modes
  and editor reaping. UI/Linux/macOS jobs pass. Backend-death mismatch255
  remains; previous failure omitted expected/native status and transcript.
- Own parent/path verification precedes retained native-handle termination
  with explicit code73; GetExitCodeProcess must independently observe73.
  UI must exit73 and report that exact cause. This strengthens causal proof
  and removes reliance on .NET Kill's unspecified status/cache. Failed exit/
  cause checks now include the bounded production transcript. No assertion
  removed; exact console restoration/reaping/all-case execution stay required.
- Only owned test harness/docs change; production UI/Rust gates unchanged.
  Native Windows four-case proof still pending;0070 remains todo.

## 2026-10-12 — actual381 receipt and0071 repository-operations claim

- PR381 actually squashed 29f5e2a63efca38fccb0f332d812fde80f11354b at2026-10-12T17:03:36Z.
  Exacte883fc9f8a0f181d99283f56ffade41788816dda passed all five
  CI38280325908 checks. Windows independently observes native73 and UI73
  cause; all four native terminal cases pass on all3OS. No check bypass.
- Rootmain fast-forwarded; user stash and unrelated untracked config intact.
  Reconcile0070 done/parity only now; combined379/380/381 completes359.
- Claim0071 before code, isolated issue-360-repository-operations checkout
  on actual381main. Preserve shipped branch/status/checkout/fetch/pull/stash
  with bounded provider-aware native services and explicit one-use approval.
- Prior standalone owned smart-Git CGI proxy research proves both provider
  origin identities unchanged with real fetch/ff-pull through loopback only.
  This is research, not future native acceptance; fail-closed tests required.
- Plan/audit before implementation; no unrelated user config/profile read.
  ADR0020 Accepted,360/294 active; no Norn review or delegation.

## 2026-10-12 —0071 approved repository operations prepared

- Implemented full-provider/root-bound Rust Git operations and five stdio
  methods with read-only previews, keyed secret-config fingerprints, one-use
  30s approvals, bounded process supervision, responsive status/cancel and
  fresh state checks before writes and between fetch/ff-merge.
- Native explorer branch panel requires a separate approval key; paste is
  inert, writes fence navigation, late clone results are discarded and actual
  completion refreshes matching options without touching review drafts/prompts.
- Actual owned IPC proves both provider identities via fixed loopback-only
  smart Git CGI, branch semantics, native stash bytes, local commits/divergence,
  ignored/conflict protection, config drift, expiry/reuse/restart and cancel.
  Added actual post-checkout-hook drift detection; actual native proof PASS.
- Corrected write-path literal-pathspec contamination: stash had saved a
  hostile untracked filename but left it in the clone. Read previews retain
  literal semantics; approved Git writes use normal semantics. Rust/native
  regressions now preserve actual saved bytes and assert the clean result.
- Gates pass: root123/UI259/core772+2ignored/stdio76, typechecks/noGUI,
  strictallfeatureClippy,209frames/Storybook, Archgate/audit. Sandbox socket
  failures were rerun with owned loopback permission and all core tests pass.
  Doctor rendering-ref error fixed;2nonblocking maintainability warnings.
- Source commit/PR/3OS exact-head checks and actual merge still required.
 0071 stays todo;360/294 stay open; no Norn review/delegation/private data.

## 2026-10-12 — PR382 Windows borrow correction

- Source5a4ccaa opened PR382; first CI38292751534 UI passes, Windows build
  rejects the changed-content reader because Windows normalization produced
  an ownedString where the confined open helper requires &str. POSIX strict
  Clippy correctly required removing the original needless borrow there.
- Follow the existing cross-platform confined-reader pattern: keep normalized
  WindowsString alive and pass its as_str view; POSIX uses the original &str.
  No path confinement, hashes, operation semantics or assertions change.
- Actual owned post-checkout-hook drift proof and bounded stale branch paging
  pass. Plan0071/360 stay pending until all5 checks pass on the new exact head
  and an actual verified squash merge. No Norn review or delegation.

## 2026-10-12 — PR382 complete keyboard approval and actual component frames

- Correctedheadf2861a5 passes all5CI38293524038 jobs, including actual
  Windows/Linux/macOS native Git and terminal-editor workflows. No merge yet.
- Reproduced a50-column long-approval defect with native keyboard input:
  the target tail could not be read. Ctrl-D/U now scrolls details in preview,
  progress and recovery; it never approves/writes. Scope/action/selection
  changes remount the details viewport, restoring its initial position.
- Corrected the new frame scene builder's unusednode property to the renderer's
  actualview slot. Positive BRANCHES/target-tail assertions now prevent a
  default-shell image from masquerading as a repository component scene.
-211actualnativeframes include wide/narrow long-approval proof; targeted input
  covers complete target reading, inert paste and separate explicit approval.
  Final UI259/1620assertions, typechecks,211frames, Storybook, lint and
  Doctor pass (2maintainability warnings; no rendering-ref error).
- Keep0071/360todo and require all5checks on the final updated PR head before
  any merge. ADR0020Accepted; no delegation/Norn review/private profiles.

## 2026-10-12 - #360 reconciliation

- PR #382 squash-merged as `49a4867` after all five exact-head required jobs
  passed (verify, ui, native-workflow on macOS/Ubuntu/Windows). #360's typed
  approved repository operations and native branch panel are shipped.
- Moved plan `0071-native-repository-operations.md` to
  `plan/done/2026-10-12-native-repository-operations.md`; updated the parity
  matrix row to merged with the commit and CI evidence; kept no code change.
- #360 can now be closed with the merge evidence; epic #294 continues with #361.


## 2026-10-13 — Current-repository entry and TUI baseline

- Maintainer explicitly chooses the shipped TUI as baseline and retires desktop
  parity. Plan 0074 owns the urgent startup correction in an isolated worktree.
- Production startup now resolves read-only Git origin identity and opens only
  that repository's PRs; no global targets/history or automatic local capture.
  Explicit workspace mode remains available. Failed final events fetch actual
  backend errors even at an equal event/status sequence, without log duplicates.
- Revised ADR0020/migration contract and issues294/306/307/308/309/361/363;
 362/364 retired as not planned, not implemented. Existing data, recovery
 archive, stores and non-destructive import/export remain protected.
- Verification: root typecheck/tests and Archgate17 pass; UI285tests and
 typecheck pass;215actual native frames match, Storybook build passes; lint
 has3existing informational suggestions. Changed-file React Doctor reports no
 issues (score API unavailable). ADR/index/link/plan-collision audit passes.
- Source PR/CI/merge evidence remains pending;0074 stays todo. ADR0020 stays
 Accepted because packaging/cutover are unfinished. User's root .norn.yaml
 and stash are untouched; no Norn review, live providers or private profiles.

- Additional owned actual Rust stdio smoke resolves GitHub and Bitbucket current
  repositories with no configured entry and unborn HEAD, without reading an
  intentionally invalid owned review policy. No provider request or credentials.


## 2026-10-13 — Correct the actual terminal UX in PR394

- Maintainer rejected the standalone PR browser as the home screen. The same
 plan0074/PR394 now restores filter/PR-list/AI panels together, Settings as a
 modal, Enter load, a explicit AI review, s settings, and g choice followed
 by g native diff or b authenticated diff viewer. Filter, selection and focus
 survive diff/settings return. Local capture remains an explicit action.
- AI output/findings load by actual provider/repository/PR/run identity; partial
 output failures do not hide valid findings. Native paste cannot trigger home
 or diff-choice shortcuts. F8 resolves the launch repository without a PR.
- Fixed the default-entry regression observed by all three editor CI lanes on
 f648c6d: the standalone browser swallowed F8. Actual owned macOS production
 PTY success/failure/interrupt/backend-death now pass with exact terminal
 restoration and owned process cleanup. No test bypass or private profiles.
- Native tests cover both providers, wide/narrow panels, filters, local explicit
 capture, saved/partial AI output, capture retry, modal/paste fencing and
 native/browser diff identity. 221 real native Storybook frames match.
- React Doctor DOM-property reports are false positives for typed OpenTUI
 native text properties; no DOM is mounted. Maintainability warnings concern
 the keyboard/controller complexity and new home component size. No broad
 rule suppression or unrelated refactor. The narrowly documented Biome ARIA
 exception applies only to native terminal renderables with keyboard handlers.
- New source verification and exact-head CI remain required before merge;
 0074 stays todo and ADR0020 Accepted. Original installed norn stays unchanged.

- Isolation correction: NORN_CONFIG_DIR does not itself isolate the credential
 loader's OS keychain/default terminal-config paths. Owned real editor lifecycle
 fixtures now explicitly use --workspace to avoid startup provider requests.
 Default current-repo home/F8 is separately tested through the native renderer
 with fake transport. Do not claim config redirection alone prevents credential
 probing, or run a real provider-startup fixture on a private machine.

- Final local verification for the corrected home: UI301/0 failures (16 native
 home scenarios), root tests/typecheck, UI typecheck, lint (3 pre-existing
 informational suggestions), Archgate17,221golden native frames and Storybook
 build pass. Owned local-workspace PTY all4cases pass. No source changes after
 this gate; exact-head CI/merge remain pending.


## 2026-10-13 — Show PR description independently of snapshot verification

- User screenshot shows metadata missing when the provider snapshot fails.
  Terminal home now loads selected PR details through the existing bounded
  provider reader, without opening the standalone browser or starting a diff,
  capture or review. Description/loading/empty/error states remain scoped to
  the exact selected provider/repository/PR. Enter refreshes metadata separately.
- Existing detail queue/generation fencing is reused; late responses cannot
  replace the next selection. Selection resets review scrolling; Local hides
  previous provider descriptions. Snapshot errors do not hide descriptions.
- Native coverage: both providers at wide/narrow sizes, snapshot failure with
  description still visible, late PR response and Local switch. UI306tests pass;
  root tests/typecheck, UI typecheck, lint, Archgate17,221native golden frames
  and Storybook build pass. Changed-delta React Doctor reports no issues
  (score API unavailable). No real provider/credential/agent access in tests.
- Same PR394/plan0074 remains pending until the new head has verified CI and
  actually merges. The installed legacy launcher is untouched.


## 2026-10-13 — Current-repository review evidence and compact Settings

Plan0074 / PR394: maintainer reports a blocked behind generic provider snapshot
error and a desktop-style global Settings inventory. Preserve verified evidence;
resolve abbreviated Bitbucket metadata to full commits using authenticated
fixed-origin endpoints, validate prefix/repository, and recheck at capture and
review start. Align rename handling across diffstat/patch. Parse Git suppressed
empty context with exact hunk counts; preserve inline positions and native
split/anchor navigation. Failure diagnostics expose stage/line/count, never
patch content or credentials. No live provider/agent used; maintainer PR still
requires a matched-backend user trial to identify its exact previous failure.

Terminal Settings shows only selected agent/model/effort, diff/theme and current
provider credentials. Hide folder inventory, Jira/sync and other agent models;
keep hidden draft values and repository paths unchanged on save. Unknown launch
identity still gets compact Settings with GitHub/Bitbucket credential choice.
Owned native tests fence paste, verify modal contents and explicit a retry with
pinned evidence for both providers.

Validation: core library776 passed/2ignored with owned loopback fixture permission;
initial sandbox run rejected52 loopback binds, so it is not passing evidence.
UI310 suite passed plus two new a retry tests; root tests/typechecks,
UI typecheck, strict core Clippy, Archgate17, lint (three existing infos),
221native golden frames and Storybook build. Changed-delta React Doctor found no
issues (score API unavailable). APFS clone of build cache isolates backend
compilation from root workspace; matched backend built in this worktree.
Plan stays todo / ADR0020 Accepted until verified integration and final cutover.


## 2026-10-13 — Maintainer-directed terminal layout refinement

Plan0074 / PR394: use the horizontal button style inspected read-only in
~/dev/under/src/menu.ts and app.ts. Top Open/Draft/Merged/Local choices replace
the vertical filter column. Left AI output/logs/findings are independent from
the central PR list above PR description. Tab/Shift+Tab cycles filters, list,
description, review; j/k and PageUp/PageDown scroll the focused text pane.
Selection resets each scroll; diff return and settings input fences remain.
Explicit text wrapping and reserved scrollbar space prevent narrow-pane text
clipping. Preserve current-repository scope and Rust operation/evidence contracts.

ADR0020r4 and migration contract reflect the maintainer's presentation refinement,
without expanding desktop parity. Native geometry tests check horizontal menu,
left output, PR/description alignment at120/50columns for both providers;
independent scroll/selection reset and reverse focus pass. Two new actual native
Storybook frames show saved AI output alongside description, wide/compact.
Validation: root tests/typechecks, UI typecheck, Archgate17, lint (three existing
infos), React Doctor changed delta (13 DOM-property false positives for typed
native text wrapMode; no DOM exists and native wrap tests pass; score unavailable),223native
frames and Storybook build pass. No backend change or rebuild needed for layout.
Keep0074pending until verified actual merge; root working tree/user config untouched.


## 2026-10-13 — Right AI pane and themed native Markdown

Plan0074 / PR394: maintainer moves AI to the right; PR list/description occupy
central-left with existing separate scroll/focus. Shared TerminalMarkdown uses
OpenTUI's native renderer and SyntaxStyle from active palette: headings, strong,
emphasis, inline code, links, quotes, list styling. Review output, PR description
and finding summaries render natively; operational status remains plain text.
Theme replacement rebuilds/frees native style resources. As in the explorer,
remote fence-language labels cannot trigger grammar asset downloads; code body
is preserved and rendered plain. No change to stored source or provider/backend.

Native tests validate actual colored/bold spans, marker concealment, unknown
fences and dark-to-light change at100/32columns. Home geometry now requires
right AI pane, and fixtures await asynchronous cold Markdown paint. Wide/compact
saved-review Storybook frames contain actual native colored Markdown. ADR0020r5,
contract and plan reflect the maintainer's refinement. Validation:315UI tests,
root tests/typechecks, UI typecheck, Archgate17, lint,223native frames and
Storybook build pass. Changed React Doctor has no issues (score API unreachable).
Plan0074 remains pending until verified actual integration; installed launcher
and root/user files unchanged. No Norn review or live provider/agent invocation.


## 2026-10-13 — Multi-file browser diff regression

Plan0074 / browser viewer workflow: maintainer reports Pierre singular-patch
exception with75files. Previous tests mocked PatchDiff and missed its cardinality
contract. Use actual parsePatchFiles in strict mode, flatten complete patches
and pass one FileDiffMetadata per native Pierre FileDiff. Namespace parser cache
keys by server version and retain repeated paths across local layers. Parser
failure retains the exact original patch in an escaped React pre instead of
crashing; independent image metadata remains available. Inline empty favicon
avoids the unrelated unauthenticated favicon404 without changing Rust policy.

Tests now mock only browser rendering, keeping the real Pierre parser: single
file,75files, repeated-path local layers, malformed-patch fallback, current
polling/session/image tests. Nine viewer tests and root suite/tooling, typecheck,
lint, Archgate17 and React Doctor pass; browser asset bundle rebuilt in owned
worktree. No live provider/agent or Norn review. Existing browser server caches
its assets; maintainer must quit/restart TUI and create a new g→b session.
No backend/source-store change. PR394 remains open;0074pending until actualmerge.


## 2026-10-13 — Browser file tree and native click support

Plan0074 / PR394: reuse FileTree for the authenticated viewer with search,
expandable folders, selected row, hide/show and per-file navigation anchors.
Tree deduplicates repeated local paths while retaining every Pierre section;
images navigate to their actual preview. Anchors use encoded paths and only one
per repeated path. Viewer keeps independently scrolling sidebar/content.

OpenTUI mouse reporting was enabled, but native diff Select rows lacked click
handlers. Add readonly clickable rows that invoke the existing store selection
and diff/preview loading pipeline, plus pane focus. Home clicks select/load PRs
or Local, filter, start review explicitly, open diff/settings/files, refresh/quit.
Diff choice supports native/browser/cancel clicks; cancelling fences a pending
read's dependent actions. Settings fields edit/cycle by click, Save/Close mirror
keyboard actions. Background clicks stay blocked during modal/loading state.
Thread compact lists cannot accidentally be interpreted as file rows.

Native mouse events test filter/PR selection, native diff, second/image file,
diff focus, settings editing/close, blocked background AI, browser diff and
explicit AI button. Web tests cover search/click/collapse visibility, image
preview navigation and unique repeated-path anchors. Async fixture waits now
require full Markdown paint and complete image/diff reads rather than fixed
short sleeps, retaining positive assertions and golden baselines.

Validation:317UI tests,127root tests plus tooling, typechecks, lint, Archgate17,
223native golden frames/Storybook and rebuilt browser bundle pass. React Doctor
changed delta has no issues (score service unavailable). ADR0020r6/contract/plan
updated under maintainer direction. No Norn review, private provider/agent calls,
user data changes or backend rebuild. Keep0074pending until verified actual merge;
restart TUI/browser session to load rebuilt web assets.

## 2026-10-14 — Resume authorized packaging and cutover

Synced PR394 with current main (#395), retaining independent tests. Windows
post-operation full snapshot had a five-second budget while preview uses twenty;
reconciliation now uses twenty too, without retrying writes or weakening uncertain
state. Owned failed-fetch IPC fixture asserts refreshed HEAD and prints bounded
operation diagnostics on failure. Local focused Rust and UI/root gates pass;
Windows confirmation and actual integration remain pending.

## 2026-10-14 — Current repository baseline shipped; hardening prepared

PR394 merged c5bcabf at08:26:17Z after all8checks on fa61cd4 passed. Windows
confirmed post-operation reconciliation;0074 moved to done. Plan0075 claimed
before implementation. Incoming client frames are bounded in wire bytes and
validated by the shared generated schema; invalid peers/stalled handshakes
reject work and reap their child. Added missing exact settings/auth/recovery/
fix/thread result definitions and nullable operation error already emitted by
Rust; shared fixtures guard real wire compatibility. Complete local Rust suite,
native local journey and4PTY restoration cases pass. Benchmarks retain original
budgets; first simultaneous-load timing failed, separate quiet measurements
passed and retain actual exit0. No live providers/agents/user data or releases.
Hardening integration and3OS gate still pending; packaging/cutover remain next.

Final hardening additions: actual PTY/ConPTY resize and raw UI Ctrl+C scenarios
(the existing keyboard quit returns0; OS SIGINT is distinct); all6POSIX cases
passed. One native UI journey preserves exact pinned AI finding/draft across a
new session and requires preview/explicit approval for one simulated publication.
Initial fixture UI paint/input-subscription timing was corrected by waiting for
paint plus subscription flush and selected state, not production sleeps.
A competing-load stdio run timed out in one owned image case; final bounded
four-test-concurrent stdio passed80 with original deadlines. This limitation is
recorded in the report; no production defect is inferred from a passing rerun.

PR396 source470461f: UI CI passed; expanded native Linux stdio failed because
that lane had never built browser assets (78passed/2missing-assets failures).
Added actual retained browser build before the complete stdio gate on all3OS;
no test fixture, production code or assertion was weakened.
