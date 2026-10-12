# Approved native repository branch operations

## Owning ADRs

- ../../adr/0020-opentui-rust-stdio-workspace.md, AC3–5/8–10.
- ../../adr/0002-http-in-rust.md.
- ../../adr/0003-credentials-keychain.md.

## Scope

Implement #360 after actual verified PR381 merge. Preserve configured clone
status, local/remote branch selection, checkout, fetch, fast-forward pull and
include-untracked stash through Rust services and typed stdio. No unshipped
linked-worktree creation, remote push, force/reset/clean/delete or conflict
resolution. Keep existing review prompts/drafts and provider identity intact.

## Implementation direction

- Full provider/workspace/repository identity with actual canonical matching
  origin and configured root; status includes missing/invalid/unborn/detached,
  dirty/conflict and bounded branch pages. Full ref kind/name/OID semantics
  remain separate from escaped display text and literal Git arguments.
- Read-only previews bind exact action/target/reference, configured canonical
  root, checkout content/index/HEAD/refs/pending operation and effective Git
  configuration/upstream fingerprint. Never return secret config values.
  One-use expiring random tokens cannot cross action/provider/clone or survive
  state/config changes. Claim apply synchronously before later backend actions.
- Preserve existing origin remote behavior: choose existing local branch if
  present, preserving its commits; else create its tracking branch. Preview
  explicitly describes actual local/tracking/detached result. Refuse dirty
  checkout/pull and existing conflicts/pending operations; preserve local
  commits, divergent pull and untracked files. Fetch prunes origin, pull is
  ff-only, stash explicitly includes untracked with existing Norn message.
- Use trusted portable Git, fixed exact argv and bounded owned process-tree/
  pipe supervision. Deliberately preserve intended user credential/config
  lookup for network writes while neutralizing unintended injected environment
  and helper execution in read preflight. No shell interpolation or arbitrary
  process API. Fresh target/config checks immediately precede execution.
- Bound read workers/pages/token/job retention and active writes. Agent,
  publication, settings, editor and other filesystem mutations cannot race
  apply. Progress/status/cancel/shutdown stay responsive. Cancellation, partial
  network state and uncertain outcomes report actual state/recovery and stop
  later writes; no replay/retry on navigation/poll/reconnect/restart.
- Explicit native confirmation with inert paste. Provider/root/epoch fences
  preserve selection, drafts/prompts and late-result isolation. Refresh actual
  local review target branches/state only after observed completion.

## Exit Criteria

1. Reconcile0070 and parity only from actual381 squash/exact-head five-gate
   evidence; create this plan before implementation.
2. Native owned clones+bare remotes cover clean/dirty/untracked checkout,
   local/origin/other-remote semantics, missing/unborn/detached/conflict, local
   commits, fetch updates, fast-forward/divergent pull and actual stash bytes.
3. Full provider collision/path/ref injection, complete state/config freshness,
   exact approval binding, actual expiry/reuse/restart, concurrency and bounded
   cancellation/partial-state recovery tests; no real network provider writes.
4. Owned loopback smart-Git proxy maps only fixed provider-qualified fake URL
   to owned bare git-http-backend. Preserve exact parsed origin, reject other
   requests and prove failure cannot contact real provider. Bound requests,
   output, time and owned process/server cleanup on supported platforms.
5. Native input and wide/narrow Storybook/frame proof for status, previews,
   confirmation, busy/error/cancel/recovery and navigation preserving unsaved
   review state. Paste/shortcut/polling never writes without explicit approval.
6. Root/UI/Rust core/stdio/protocol/noGUI/Archgate/audit/Doctor/Clippy/fmt gates;
   all required checks on exact PR head and actual verified squash merge.
7. Keep this plan todo until actual merge; reconcile receipt/parity and close
   #360 only after all baseline proof. ADR0020 remains Accepted until309;
   preserve maintainer parent-issue dispositions and epic294 remains active.

## Dependencies

- PR380 native repository explorer; PR381 safe editor/terminal handoff must
  actually merged 29f5e2a63efca38fccb0f332d812fde80f11354b at2026-10-12T17:03:36Z.
  All5CI38280325908 SUCCESS on exacte883fc9f8a0f181d99283f56ffade41788816dda.

## Status

- Claimed by default-agent2026-10-12, single writer, after actual381merge.
- Branch issue-360-repository-operations.
- Location /private/tmp/norn-repository-operations on actual381main.
- Implementation not started; plan precedes code.
- No delegation, Norn code review, real provider writes or private profiles.
- Keep todo until actual verified PR merge.
- Stopped:
