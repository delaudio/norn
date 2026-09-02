# Browser Diff Shared UI

## Owning ADRs

- `../../adr/0016-browser-diff-viewer.md`

## Scope

Replace the browser diff viewer's duplicated HTML, CSS, diff parser, and
renderer with a dedicated React entry point that reuses the desktop
application's `DiffViewer`, `FileTree`, `FileDiff`, parsing utilities, and
design tokens. Keep the existing ephemeral loopback server, authenticated
session path, provider boundary, bounded polling, and raster-preview controls.

Package the browser bundle with command-distribution builds so a Homebrew
installation remains self-contained. Preserve a clear source-development path
and fail visibly if packaged assets are unavailable.

Out of scope: browser comment publication, AI review controls, persistent
browser sessions, or changing provider credential handling.

## Exit Criteria

- ADR 0016 AC1 and AC5: the shared React viewer is served only through the
  authenticated loopback session and retains the existing browser response
  protections.
- ADR 0016 AC2-3: browser state remains bound to the selected pull request and
  failed provider population stays bounded across polling.
- ADR 0016 AC4: image previews continue to accept only bounded inert raster
  formats.
- ADR 0016 AC6: focused tests cover the browser entry point, state loading,
  asset authentication, missing assets, and release packaging.
- The browser renders textual and image diffs through the same maintained
  components and theme used by the desktop application.
- Homebrew command archives contain every browser-viewer asset required by
  `norn-tui` without a source checkout or development server.
- `pnpm run typecheck`, `pnpm run test`, and `archgate check` pass.

## Dependencies

- `../../adr/0016-browser-diff-viewer.md`
