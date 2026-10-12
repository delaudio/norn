# Qualified local comparison and native review terminal controls

## Owning ADRs

- ../../adr/0020-opentui-rust-stdio-workspace.md, AC3–5/8–10.
- ../../adr/0002-http-in-rust.md.
- ../../adr/0003-credentials-keychain.md.

## Scope

Next bounded #358 slice after the actual #376 merge. Connect the existing native
Shell to qualified current/configured local captures, independent layer files,
diffs, scoped findings/history/conversations and actual native review start/cancel.
Add actual source/base/layer controls and qualified native profile catalog reads.
Keep Rust policy/engine/evidence authoritative. Pinned local images/Pierre, saved
snapshot/global-history completeness and elimination of unsafe old Local consumer
fallbacks remain a following required slice; this UI slice alone is not full#358.
Preserve the maintainer's#306 closure without inventing child or cutover proof.

## Concrete starting points

- ui/src/app/store.ts: working-tree PickerEntry currently becomes a provider
  target without a PR. activeTarget/loadSnapshot/loadDiff/loadPreview/startReview,
  openReviewProfiles/selectThread/jumpToFinding/applyEvent/resyncOperation must
  route qualified local state explicitly; never fall back to provider or cwd.
- ui/src/app/components/Shell.tsx: existing keyboard/paste/modal dispatch,
  responsive panes, file rows, diff scroll keys, history/conversation and profile
  panel. F4 can expose local source/comparison controls; uppercase B preserves
  native base cycling while lowercase b remains browser. Plain paste must not
  invoke capture, start, branch comparison, mutation or approval.
- BackendClient localOptions/Capture/Cancel/Info/Files/Diff and localReviewStart/
  History/Findings/Output/Threads are strict generated methods. New bounded
  aggregate helpers must reject non-advancing pages, check bytes/full diff SHA,
  and fence source/generation/nonce throughout; no broad unchecked result casts.
- Source Current belongs to backend startup Git root, Configured contains an
  exact native repository identity. Compare captured response owner/root with
  options selected before capture, especially Current, which has no caller owner.
- Data private canonical root + Info full SnapshotRef are verified by native
  Manager/SQLite. Add a bounded local profile read using this full ref and a
  native live source/root/provider proof; not legacy arbitrary-SHA catalog logic.
  Reuse shared native policy loader/default/errors/from_policy. No path in IPC.
- Native baseline src-tauri/src/tui/mod.rs defaults to current repository and B
  cycles working-tree/candidate base branch comparisons. Current eligibility is
  native remote/config based; missing/unsupported source is honest, not guessed.
- ChangedFile/FindingSummary can carry optional local layer/candidate metadata;
  every same-path file row, selection, viewed key, diff/scroll/syntax/preview fence
  must include layer and qualified snapshot. Existing provider behavior remains.

## Exit Criteria

1. Record predecessor#376's actual exact-head CI/squash proof, sync/rebase onto
   actual main, claim plan0066 before code and check native number collisions.
   No source changes in a sibling worktree before that merge.
2. Real terminal controls select backend-startup Current or exact configured
   repository source, with honest unavailable/ambiguous/unsupported states and
   no client filesystem path/provider guess. Initial supported native launch
   prefers current-repo local view; provider PR navigation remains usable.
3. Source form fetches bounded actual native options. Working-tree defaults to
   canonical committed/staged/unstaged selection; branch comparison is committed
   only and uses a listed base. Explicit checkboxes retain at least one layer.
   Apply performs a capture, not an agent start. B cycles actual base refs and
   working-tree; source/base/layer changes cannot silently keep old evidence.
4. Capture response comparison/source/actual owner/root must match the selected
   options. Generate native-format nonce; explicit cancellation and source change
   cancel that owned capture, fence late responses and leave responsive controls.
   Method deadlines must exceed bounded native capture+DB work, not race default
   30-second transport timeout; test timeout/cancel/generation behavior honestly.
5. Layer-qualified file rows retain duplicates. Diff pages use the saved full
   reference/layer/path, aggregate at most16MiB, verify stable total/hash and full
   content SHA, and never read fresh cwd/index/untracked files. Preserve split,
   culling/collapse, precise anchors, search/syntax, viewed markers and clipboard
   with identity/layer fences. Unsupported captured image/browser controls give
   explicit notices until their required pinned consumer slice; no legacy fallback.
6. Local findings use selected native run. An anchor with multiple captured
   candidate layers requires an explicit layer choice before jumping; no first
   same-name file guess. Severity/group/filter remain. Null/unmapped/outdated
   anchors show a notice; exact old/new side and pinned patch determine presence.
7. History and attributable conversations use full snapshot+run ID; selected
   thread/run updates scoped findings/output without changing source or using
   synthetic provider IDs. Preserve multiple runs, old selection and restart
   semantics; missing checkout cannot cause fresh cwd substitution. Excerpts mark
   truncation and private resume metadata remains absent. Broader saved snapshot/
   cross-source/legacy history is still required in the following history work.
8. Enter starts only qualified saved evidence with actual native profile/default;
   operation target/ID/generation/event sequence fences survive events arriving
   before control response, late selection, cancel and reconnect/status resync.
   Retain previous-target running operation visibility/cancel without attaching
   its findings to a new source. No automatic retry or capture during a start.
9. F6 reads qualified native profiles through exact live source proof and shared
   loader. Generated DTO/fixtures/client checks full ref/catalog target/default/
   names/count/bytes. Selecting a profile changes UI state only; canceled/late
   catalogs cannot mutate another source. Capture metadata/pins/warnings and
   independent layer meanings reach the shared Rust payload, preserving baseline
   local/unpublished-work context rather than claiming excluded content/pushes.
10. Owned native renderer/input tests cover Current/configured/provider clones,
    staged/unstaged same path, working/base/layers, real profile/default, finding
    ambiguity, history/thread selection, source generation/cancel/deadline, events
    before response, paste and unavailable/clean/unborn/detached states. Add wide
    120x30/narrow50x18 native Storybook frames for source/comparison/loading/error,
    separate layers/findings/history/profile. Do not mistake mocks for the later
    actual Rust stdio+native terminal full workflow proof.
11. Full applicable root/UI/Rust/protocol/Archgate/Doctor/fmt/Clippy/noGUI/audit
    gates pass. Any added recipe is mirrored in Makefile/justfile. No Norn review,
    actual agent/live provider/private profile/release. Prepare focused PR, verify
    both required jobs on its exact final head and actual squash merge. #358 and
    #294 remain open; preserve maintainer-closed#306; ADR0020 remains Accepted.

## Dependencies

- #376 verified squash4644a03739c78fbe7e687edbb7a03d54f8366c13 at
  2026-10-12T07:12:32Z; both requiredCI38214976426 jobs SUCCESS on final
  19610d53dc067148367371ee56dd346bfcab2a3f (UI06:56:27Z, verify07:10:53Z).
  Root main ff synced to the actual merge before this branch/claim.
- #375 qualified immutable local evidence/schema20 and #374 native profile catalog.
- Existing native Shell diff/operation/storybook behavior; following local pinned
  images/Pierre/saved-history/unsafe fallback/full workflow proof before#358 ends.

## Status

- Claimed by: default-agent, 2026-10-12, single writer.
- Branch: issue-358-local-terminal-controls.
- Location: isolated /private/tmp/norn-local-terminal-controls, actual376mainbase.
- Blockers: implementation/local gates/own exact-head CI/verified PR squash merge.
- Stopped:
