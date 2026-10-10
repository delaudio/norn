# Web Browser Diff Viewer With Pierre

## Owning ADRs

- `../../adr/0020-opentui-rust-stdio-workspace.md`

## Scope

Give the retained web (browser) diff viewer the best visualization by rendering
provider diffs with `@pierre/diffs` (Shiki-based, split/unified, per-line
highlighting and annotations) instead of the shared `react-diff-view` component.
The terminal keeps its own renderer because terminal performance is the
priority there. Loopback binding, per-route session authentication, MIME
allowlisting and security headers are unchanged; image previews are preserved
with a small panel.

GitHub issue: #302 (epic #294 step 08/15).

Out of scope: relocating the shared DOM `DiffViewer` used by the retained desktop
app, and enabling the Pierre worker pool.

## Exit Criteria

- The browser viewer renders `remoteState.diff` with `<PatchDiff>` and the
  split/unified toggle maps to Pierre's `diffStyle`.
- Image previews keep the old/new-side contract (alt path, `new image`/`base
  image` label, authenticated preview URL).
- `pnpm run typecheck`, `pnpm run test` (Vitest + tooling), `pnpm run build`,
  `pnpm run lint` and `archgate check` pass.
- The authenticated session/security tests for the Rust server still pass.

## Known limitations

- Pierre runs without its worker pool (main-thread highlighting); the bundle
  gains Shiki language chunks. A worker pool and code-splitting are follow-ups.

## Dependencies

- `../../adr/0020-opentui-rust-stdio-workspace.md`
- `../../../docs/opentui-migration/migration-contract.md`
