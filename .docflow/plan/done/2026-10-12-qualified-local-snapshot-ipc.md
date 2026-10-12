# Qualified immutable local snapshot IPC foundation

## Owning ADRs

- `../../adr/0020-opentui-rust-stdio-workspace.md` (AC 3–5, 8–10).
- `../../adr/0002-http-in-rust.md`.
- `../../adr/0003-credentials-keychain.md`.

## Scope

First bounded implementation slice of retained #358. Inventory the shipped
terminal/desktop/CLI local contract; expose typed local source/options/capture/
read/recheck IPC backed by Rust's existing local snapshot collectors. Bind a
snapshot to exact canonical root, actual configured provider owner or explicitly
supported current repository, branch/base selection, native pins and selected
committed/staged/unstaged layers. Preserve overlapping layer evidence and native
untracked exclusion warnings. Persist bounded immutable evidence for restart.
TypeScript receives opaque identities and bounded metadata/pages, never path
input granting filesystem access. Follow-up #358 work must connect native UI,
diff/findings/review start/cancel/history/Pierre to these same snapshots before
#358 or full local parity can close; this item alone does not claim that UI.

## Exit Criteria

1. Record shipped behavior: terminal Local working-tree default uses native
   upstream/merge-base and distinct committed/staged/unstaged evidence; B cycles
   native base refs into committed-only branch comparison, excluding dirty and
   untracked content explicitly. Separate unshipped desktop backlog from parity.
2. Strict generated local source/options/capture/lookup/read/recheck contracts
   reject unknown fields, invalid source/ref/layers/pages/handles before Git,
   config/credential access. Limits cover fields, refs, files, patch chunks,
   result/frame size, capture workers, in-memory/storage quotas and deadlines.
3. Exact configured provider/repository lookup validates canonical Git root and
   actual remote without provider-less/case/name/cwd fallback. Current-repo
   source is explicitly backend-startup scoped, never a caller-supplied path or
   guessed provider. Ambiguous/missing/unavailable/current-without-origin states
   are represented honestly without creating provider publication authority.
4. Existing native snapshot collectors produce immutable actual branch/base/
   head/local evidence. Working-tree layer selection keeps distinct overlapping
   path hunks; branch mode is committed-only. No shell interpolation, user
   filters/hooks/helper/network execution, hidden branch switch, index/worktree
   modification or agent start. Unborn/detached/clean/dirty states remain useful.
5. Domain-separated immutable identity includes canonical root and actual source
   provenance plus requested comparison/layers and captured native hash/pins.
   Identical owner/repo/file names across providers or clones cannot share a
   handle. Full source/request/handle ownership/hash/inventory checks precede
   reads. Never resolve an arbitrary Local SHA through backend cwd.
6. Bounded durable evidence restores after backend restart, with explicit corrupt/
   stale/quota/version errors and actual ownership. Reads return saved evidence,
   not fresh file contents. Recheck recaptures the same comparison/layers and
   rejects changed HEAD/index/worktree/base before consumers can run agents.
   Preserve existing local run/storage/CLI contracts; document schema compatibility
   and backup implications if a new table is necessary.
7. Owned real Git/native policy/stdio/contract fixtures cover both configured
   providers, different same-name clones, current scope, no-origin, staged plus
   unstaged overlap, committed divergence/base movement, untracked exclusion,
   deletion/rename/binary, detached/unborn, malformed/oversized/corrupt/stale
   data, restart, concurrent/slow capture/EOF control. No private repository/
   profile/credentials, root unrelated .norn.yaml, live provider writes or agents.
8. Root typecheck/tests/lint/protocol/Archgate; Rust no-default full/stdio/CLI/
   onboarding, strict all-feature/all-target Clippy/fmt/noGUI; applicable UI
   contract/tests/frames, docflow audit pass. This backend foundation has no new
   terminal controls; native input/frames/full fake-agent local workflow remain
   mandatory in the next #358 slice, not waived by this PR.
9. Verify predecessor PR#374 exact-head CI and actual squash merge; rebase onto
   its squash and audit numbering before this PR. Open a focused PR and complete
   only after its fixed-head required CI and verified squash merge. Keep #358,
   #306/#294 open and ADR0020 Accepted until all remaining criteria are proved.

## Dependencies

- PR#374 final `3dd4fb082d0b5154a32094c0db72225dbc845892` passed CI38205709559
  and was verified squash-merged `a3b6e4dcdd62278fb3cd5e781df4f8597e80b5c6`
  at05:09:19UTC. Rebase onto actual squash before this PR.
- Existing local_review collectors, native policy/review engine and storage
  ownership, generated stdio protocol and completed profile catalog proof.
- Native UI/engine/history/Pierre integration is the next bounded #358 slice.

## Prepared completion receipt

Implementation commit: `d7ba477bc28dbb3c28f72a041c8f56357ddc8031`.
This receipt becomes effective only after the final PR head passes both required
CI jobs and its squash merge to main is verified. No pending PR is shipped proof.

Local gate: root typecheck/123 Vitest tests plus tooling/lint/protocol, Archgate17
and docflow audit; UI typecheck/201 tests/1,254 assertions/135 native frames;
Rust744 core tests plus2 existing ignored,64 stdio,6 CLI,5 onboarding,3 binary;
no-default all-target check, strict all-feature/all-target Clippy, fmt and noGUI.
ReactDoctor100 with no findings in the three changed TS files; no setup/suppression.

Owned12 local tests prove strict current/configured sources, exact remote owner,
identical-clone separation, separate overlapping layers, branch-only evidence,
base movement, detached/unborn state, rename/delete/binary metadata, a9MiB patch,
UTF-8 chunks, immutable restart/foreign/corrupt/stale data, consistent backup and
cascade, atomic quota rollback and bounded reservations. Real stdio additionally
proves both providers, restart, immediate cancel/control and EOF cleanup. Generated
fixtures agree on safe relative paths after correcting the initially weak schema.

Schema20 durability requires a verified compatible backup and matched rollback in
#308. No private data/live writes/agents/Norn review/public release was used.
The next #358 integration must connect native UI/diff/findings/start/cancel/history/
Pierre to these exact snapshots; #358/#306/#294 remain open. ADR0020 stays Accepted.
