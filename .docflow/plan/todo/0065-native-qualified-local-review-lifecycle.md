# Native review lifecycle for qualified local evidence

## Owning ADRs

- ../../adr/0020-opentui-rust-stdio-workspace.md, AC3–5/8–10.
- ../../adr/0002-http-in-rust.md.
- ../../adr/0003-credentials-keychain.md.

## Scope

Second bounded #358 slice after the immutable local snapshot foundation. Connect
qualified captured local evidence to the existing Rust inline review engine,
operation streaming/cancel and durable local run/history/findings/output. Actual
local terminal selection and pinned Pierre/preview consumers remain the next
required #358 slice; this backend delivery cannot close local parity by itself.
Use owned fixtures and fake native agent children only.

## Concrete starting points

- services/local_snapshot.rs and local_snapshot/capture.rs: Source, Comparison,
  Info, SnapshotRef, validated Data and startup-scoped Manager. Its serialized
  private root cannot become an IPC path authority. Manager get validates durable
  source/root/hash/inventory/owner and recheck repeats the selected comparison.
- review_storage/local_evidence.rs: parent target_kind local, key includes actual
  workspace/repo plus opaque snapshot ID, high-bit localReviewId. Runs under this
  parent must have actual provider/workspace/repo/id and opaque reviewedHeadSha.
  No legacy provider-less store or same-name clone may supply this source.
- services/review.rs: ReviewStoreTarget::Local, start_local_inline_review_native,
  start_inline_review_for_target and run_inline_review_pipeline. The legacy local
  wrapper currently supplies no explicit repo_path_override/provider override.
  Pipeline fallback resolve_local_repo is provider-less; captured source must
  explicitly bypass it. Reuse native prompt/policy/agent/parser/store/lifecycle,
  not a second TS review implementation.
- backend.rs: ReviewRuntime/start_operation/run_real_review_operation,
  InlineSession/monitor_inline_session and owned native agent seam. Existing
  Local branch in start_inline_session flattens cwd changes/untracked files with
  legacy ID0 and must never implement the new qualified start path.
- Existing global history/findings/output/threads are provider-only. New local
  query selection must use full SnapshotRef plus bounded selectedRunId and actual
  local storage; synthetic high-bit IDs never authorize provider calls.

## Exit Criteria

1. Add strict generated request/result contracts for captured local start and
   scoped paged history/selected run findings/output/conversation reads, with
   source/snapshot/run echo, byte/count/frame bounds and two worker/source checks.
   A client path, arbitrary local SHA, unknown run or synthetic provider PR is
   rejected before credentials/agent construction. Keep version negotiation and
   headless builds free of UI/runtime dependencies.
2. Native captured-local execution context contains actual qualified owner,
   opaque snapshot ID/high-bit local ID, validated canonical root and captured
   branch/base/native fingerprints. Fresh starts recheck the same comparison
   under owned native cancellation before launch. Wrong/stale/changed source
   fails; no cwd/name/provider inference, fresh patch or untracked fallback.
3. Use existing shared native policy/profile/default/prompt builder and parser.
   Payload must explicitly label each selected committed/staged/unstaged layer,
   preserving duplicate paths with incompatible hunks. Do not concatenate them
   as one ordinary Git patch or claim excluded files were reviewed. Actual pins,
   context and evidence identity must agree with saved run/history. Honor native
   agent payload limits: reject oversized exact evidence before child launch;
   never silently truncate it to fit.
4. Existing local CLI/service wrappers and stored legacy data stay compatible.
   New exact source context must use actual provider and root through pipeline,
   agents/analyzers and materialization, even when config default provider is
   different or same-name GH/BB/clones exist. No global mutable source register.
5. Native operation events/terminal state/cancel are owned by operation ID and
   local snapshot. Fake native process receives exact payload and actual root,
   cancellation reaps it and preserves honest partial/failed result. EOF fences
   late output and does not launch queued agent work. No real agent/provider write.
6. New local runs persist under the existing local native parent without replacing
   other runs or aliasing providers/clones. History/findings/output/conversations
   select exact stored run/source after restart and work from immutable evidence;
   a missing checkout must not substitute cwd. Bound and validate records before
   producing IPC. Untyped/unknown historical provenance remains #361.
7. Protect existing provider workflows and qualification; reject local publication
   before provider credential access, including synthetic high-bit provider IDs.
   Do not use provider review_scope.key as the local native parent.
8. Owned real Git/fake-child/stdio/client fixtures cover both actual providers,
   wrong default provider, same-name clones, layered overlap, branch base,
   untracked exclusion, stale pre-start, wrong source/run, canceled/failed/success,
   exact root/payload and restart history/output. A real child process through
   the existing native fake-agent seam is required; simulated progress alone is
   not proof of an implemented review path.
9. Root typecheck/tests/lint/protocol/Archgate; UI applicable contract/tests/frames;
   Rust no-default full/core/stdio/CLI/onboarding, strict all-feature/all-target
   Clippy/fmt/noGUI and docflow audit. Preserve Makefile/justfile parity.
10. Claim the plan before code, after predecessor#375 exact-head CI/actual squash
    merge and rebase. Prepare a focused PR, verify its actual fixed-head required
    CI and squash merge before completion. #358/#306/#294 remain open and
    ADR0020 Accepted. The next UI/Pierre slice must prove actual native inputs,
    wide/narrow frames, late selection fencing and complete local workflow.

## Dependencies

- #375 verified squash-merged5e4e1b293b7044c693b5f8f2e8308f59d810cebf
  at2026-10-12T05:58:26Z. Both required CI38209164847 jobs passed on
  final42dea41dc7723be0a5ad503fc973038f3b47ba89. Root main synced.
- Native local collectors, schema20 local evidence, shared native engine/policy
  and operation streaming/cancellation already exist; do not reimplement in TS.

## Status

- Claimed by: default-agent, 2026-10-12, single writer.
- Branch: issue-358-local-review-lifecycle.
- Location: isolated /private/tmp/norn-local-review-lifecycle, actual main base.
- Blockers: implementation, local gates, own fixed-head CI and verified PR merge.
- Stopped:
