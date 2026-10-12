# Pinned local raster/browser and saved snapshot consumers

## Owning ADRs

- ../../adr/0020-opentui-rust-stdio-workspace.md, AC3–5/8–10.
- ../../adr/0002-http-in-rust.md.
- ../../adr/0003-credentials-keychain.md.

## Scope

Follow actual verified PR377 main merge. Finish required #358 consumer behavior:
durable independent-layer before/after raster captures; authenticated Pierre for
one explicitly selected captured layer; qualified saved-local snapshot/run access;
rejection of unsafe arbitrary Local SHA/cwd consumer paths; actual owned native
OpenTUI + Rust stdio review/navigation/cancel/restart smoke. Do not infer broad
repository editing, mutating branch/worktree, global legacy analytics, local
reply/fix, resolved references, polling, recovery or packaging parity from this.
Determine whether those belong to their retained issues from actual shipped
baseline, rather than expanding #358 to unrelated unshipped backlog.

## Starting points

- local_snapshot/capture.rs stores format_version1 root/Info/independent patches,
  verifies the complete serialized hash, and validates persistent owner even on
  cache reads. No image bytes are stored. Recheck currently compares live root/
  owner/native fingerprint; new raster capture must participate in stale proof.
- Stored format extension must preserve old content identities and saved reads.
  Explicit version2 or optional fields with skip-serialization cannot silently
  rehash version1 records. Old captures lacking rasters must report unavailable,
  never fetch new worktree/index content. Keep existing schema20 unless a real
  new storage table/column requires a reviewed schema migration.
- provider_snapshot/images.rs has supported raster validation (2MiB per image,
  dimensions8192/40M pixels, MIME/format verification), SHA and chunked page rules.
  Reuse an extracted Rust raster helper rather than a divergent parser. Capture
  committed base/HEAD, staged base/index and unstaged index/worktree independently;
  rename source, deletion/addition absent sides and symlink/submodule restrictions
  must come from actual captured inventory/native Git modes. No untracked bytes.
- Capture needs bounded/cancel-aware Git/file reads, exact root proof, stable
  before/after collection and explicit per-image/aggregate quota behavior. Byte
  hash/dimensions/format/count/size stored alongside identity, validated on reads;
  no decoding unbounded buffers. Native recheck rejects changed captured raster
  bytes before agent execution even when ordinary binary patch text is unchanged.
- browser_diff WebDiffState.snapshot_source currently only Provider source wrapper.
  Add qualified Local source including full SnapshotRef and selected layer; all
  image serving is immutable manager/storage based. Equality/session/version fences
  must include local root/ref/layer. Never take mutable legacy preview fallback.
  Pierre receives exactly one layer, actual owner/pins/local label and diffstat;
  no concatenating contradictory staged/unstaged patches into one ordinary patch.
- Snapshot listing must expose bounded summary/full refs, no canonical path or
  synthetic provider PR authority. Validate persistent owned source/hash before
  using a saved ref. Select a snapshot with localInfo/read-only files/history even
  when config/checkout vanished; live start remains explicit same-source recheck.
  Preserve selected run and make old same-name clones distinguishable by root ID.
- UI local file preview/browser explicit unsupported notices from plan0066 are
  replaced only by the new qualified APIs. Reuse provider image chunk/hash/size
  aggregation and terminal before/after rendering, fenced by full ref/layer/path/
  generation. Saved snapshot selector is modal, paged, native input/paste safe.
- Legacy Local arbitrary SHA request routes still resolve cwd or configured default.
  Reject them in new-consumer context or remove their public route only after
  mapping and updating actual tests/legitimate consumers. No guessed provider or
  synthetic high-bit local row may authorize provider/credential access.
- Existing CI UI lane has Bun but no built Rust binary. Actual native full-workflow
  gate must build no-default norn-backend and run owned native renderer/input with
  production subprocess transport and fake native agent. Mirror any recipe in
  Makefile/justfile and run the scenario in CI. Missing binary is a failure, never
  skipped proof. Keep headless library/CLI free of GUI/Bun dependencies.

## Exit Criteria

1. Verify377 exact-final-head required jobs and actual squash; sync actualmain,
   check number collisions and claim next plan before code in its own worktree.
2. Owned changed raster tests establish distinct base/staged/unstaged bytes and
   old/new/rename/added/deleted states, restart/missing checkout and no mutable
   reread after modifying checkout. Wrong ref/layer/path/hash/side/bounds, corrupt
   storage, symlink/submodule, oversized/malformed raster and quota behavior fail
   safely without credentials/untracked access; cancellation fences late work.
3. Local image pages/client verify complete content SHA/byte/metadata continuity;
   terminal before/after components render bounded owned captures and explicit
   absent/unsupported states with actual native wide/narrow fixtures.
4. Browser serves identical saved layer patch and old/new image hashes under an
   authenticated current session, with hostile origin/nonce/path/late-session
   tests. Local mode never uses a provider PR or mutable snapshot fallback.
5. Qualified saved-local selector retains old runs after source changes/restart;
   read-only owned missing-root data is accessible. Unknown legacy provenance is
   not guessed. Publication rejects high-bit/local identity before credential/
   provider calls. Preserve user storage and format1 compatibility.
6. Actual owned no-default Rust backend plus OpenTUI/production transport/input
   chooses source/base/layer, navigates diff/finding, starts/cancels a fake native
   review and restarts into its real saved run. Fake output is test-only, not a
   production engine. Native current-repo CLI smoke uses an owned Git fixture.
7. Applicable root/UI/Rust/protocol/Archgate/audit/Doctor/Clippy/fmt/noGUI gates,
   scenario workflow and exact-head required CI pass before actual squash merge.
   Update parity matrix from actual merged evidence; #358 only ends if all its
   real acceptance scenarios are achieved. Maintain #306 maintainer closure and
   ADR0020 Accepted until full cutover; remaining retained tasks stay pending.

## Dependencies

- PR377 verified squash63970f499ae1a16c66e02438ec7fdd59f26d4cb5 at
  2026-10-12T08:12:58Z. Both requiredCI38220689561 jobs SUCCESS on final
  c4278e1efe5ae1f4f0dffd5d6c0d6b3d54fdc1c8. Root main ff synced to
  actual merge before creating this isolated branch or claiming plan0067.
- Existing375/376 qualified source/storage and native lifecycle, schema20.

## Status

- Claimed by: default-agent, 2026-10-12, single writer.
- Branch: issue-358-pinned-local-consumers.
- Location: isolated /private/tmp/norn-local-pinned-consumers on actual377main.
- Blockers: implementation/local gates/own final-head CI/verified squash merge.
- Stopped:
