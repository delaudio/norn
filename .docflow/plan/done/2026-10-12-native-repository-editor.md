# Native repository editor and real terminal handoff

## Owning ADRs

- ../../adr/0020-opentui-rust-stdio-workspace.md, AC3–5/8–10.
- ../../adr/0002-http-in-rust.md.
- ../../adr/0003-credentials-keychain.md.

## Scope

Complete #359's explicit external editor/file-open capability after actual
PR380 merge. Preserve the shipped zed/cursor/code priority and system fallback,
with platform-aware no-GUI Rust adapters, bounded typed IPC and truthful
launch/unavailable/error states. Add safe interactive-editor handoff and actual
terminal/console proof required by #359. No branch/worktree or provider writes,
arbitrary finding anchors, private profiles or general process-execution API.

## Implementation direction

- Rust owns configured provider/root/path confinement, regular-file validation,
  integration lookup, exact argv and child ownership. No caller absolute root/
  executable/PID; filenames never become shell syntax. Preserve user-installed
  macOS login-shell lookup using fixed command/positional data, not constructed
  repository shell strings. Windows uses executable/native system association
  adapters rather than cmd/start filename interpolation. Desktop wrappers remain
  thin until final removal.
- Explicit native action from the selected F8 file and current source line;
  binary/system-open, missing/deleted/unconfigured and stale contexts are
  deliberate states. Display escapes do not alter original path/line/argv.
  UI input/paste does not launch automatically; generation fences every result.
- GUI launch returns promptly and honestly; no indefinite UI wait or invented
  success acknowledgment. Bound active launchers/monitoring and reap owned
  subprocesses without treating an intentionally opened GUI as an agent.
- For terminal integrations, backend protocol stdin/stdout stay pipes. Open the
  actual terminal/console through platform handles, never through protocol
  streams. Native OpenTUI suspend/resume brackets handoff, restores modes and
  focus in finally on launch failure/exit/nonzero/cancel/backend death.
- Source-led model to verify: Rust managed terminal child with POSIX confined
  process group, foreground assignment and SIGTTOU-safe restoration; Windows
  attached real console handles and owned Job Object, using a private native
  child wrapper if required for console controls. UI suspends its own input/
  render while Rust retains authority and status/cancellation. Avoid simultaneous
  readers and protect the persistent backend from editor interrupt signals.
- Use bounded one-terminal-operation ownership/status/cancel plus shutdown/EOF
  cleanup. If transport close currently hard-kills before cleanup, implement
  the necessary bounded graceful shutdown and fallback with regression proof.
  Refuse handoff during an active agent/mutation workflow. Resolve actual
  platform failures rather than weakening the acceptance test.
- Full terminal smoke uses owned POSIX PTY/Windows ConPTY and owned compiled
  fake editor, including input, argv, mode changes and restore. Native frame
  tests alone cannot prove terminal lifecycle.

## Exit Criteria

1. Claim after actual380merge; reconcile0069 done receipt and parity matrix only
   from observed source/check/squash evidence.
2. Owned native tests for exact Unicode/spaces/option-like/control filenames,
   line position, both provider clones, symlink escape, stale path/context,
   unavailable integration, spawn/nonzero failure and cancellation.
3. Wide/narrow OpenTUI input/frame proof for explicit action, safe paste, late
   close/source switch, launch state and preserved review selection.
4. Real owned terminal/console smoke on all3OS verifies editor input and
   terminal/focus restoration for success/failure/cancel/backend death.
5. Root/UI/Rust/stdio/protocol/noGUI/Archgate/audit/Doctor/Clippy/fmt gates; own
   exact-head CI and actual verified squash. Keep plan in todo until that event.
6. Update parity with actual merged editor proof; close #359 only if its entire
   baseline and required scenarios are proven. ADR0020 remains Accepted until309.

## Dependencies

- PR380 actual verified squashd623231b16215a8d828764bfdfe48349c6d73254
  at2026-10-12T12:43:36Z; all5CI38249399379 SUCCESS on
  99b33bdbe73befa2b2ef25be30171f657cb8bea3, real native3OS F8 workflow.

## Status

- Claimed by: default-agent,2026-10-12, single writer after verified380merge.
- Branch: issue-359-native-editor.
- Location: isolated /private/tmp/norn-native-editor on actual380main.
- Implementation prepared: typed one-use editor RPCs, native GUI adapters,
  private terminal wrapper/ownership and OpenTUI suspension/restore. Owned
  macOS PTY proves input, literal argv, real mode changes and restore after
  success/nonzero/Ctrl-C/backend death; GUI IPC proves both providers, stale
  file/root, symlink escape, missing paths and token replay/context rejection.
- Local gates observed: root/UI typecheck, root123+tooling, UI253/1588,195
  native frames/Storybook, Rust767+2ignored/76stdio/6CLI/5onboarding/noGUI,
  Archgate17, audit, strict all-feature Clippy including final native adapter
  test rerun. React Doctor25files: no diagnostics;
  current score API unavailable (earlier scan score66). No installation/setup files added by Doctor.
- Completed: PR381 actually merged with all five exact-head checks passing.
- Status: completed from observed squash/native proof, reconciled into done
  with following repository-operations claim.
- Stopped:

## Shipped receipt

- Actual PR381 squash 29f5e2a63efca38fccb0f332d812fde80f11354b at2026-10-12T17:03:36Z.
- Exact sourcee883fc9f8a0f181d99283f56ffade41788816dda; all five
  CI38280325908 jobs SUCCESS: verify, UI and Linux/macOS/Windows native.
- All three platforms prove actual production UI/Rust editor success/error/
  Ctrl-C/backend-death, literal argv/input, owned child reaping and exact
  native mode/focus restoration. Windows known native exit73 independently
  observed and matched UI exit/cause; console guard survives interrupt.
- Local253UI/1588assertions/195frames/Storybook; Rust767core+2ignored/76stdio,
  strictClippy/noGUI/CLI/onboarding/root/typecheck/lint/build/Archgate17/audit.
  Cached Doctor25files no diagnostics; score service unavailable.
- Receipt follows actual merge. Combined379/380/381 evidence completes359;
  parent parity and epic294 remain active. ADR0020 stays Accepted until309.
