---
adr: 0016
title: Offer an authenticated browser diff viewer from the terminal UI
status: Accepted
date: 2026-09-02
owner: default-agent
supersedes:
superseded-by:
depends-on: [0002, 0003, 0006]
tags: [tui, diff, browser, security, rust]
---

# ADR 0016 - Offer an authenticated browser diff viewer from the terminal UI

## Context

The terminal UI provides native unified, split, and image diff views, but large
pull requests can be easier to inspect in a browser. This auxiliary surface must
reuse native provider services and stored credentials without exposing secrets
to browser code or creating a generally accessible local API.

Pull request selection and provider loading are asynchronous. Browser state must
therefore identify the pull request to which cached data belongs rather than
assuming the currently selected row and currently loaded diff are the same.
Provider failures must also be bounded so browser polling cannot turn one failed
load into repeated provider traffic.

## Capability statement

Norn will let a reviewer open the selected pull request diff in an ephemeral
loopback browser viewer. The viewer authenticates every route with an
unpredictable session path, reuses Rust provider services, binds cached content
to its pull request identity, and fails closed for unsupported active content
and repeated provider loads.

## User stories / scenarios

- As a terminal reviewer, I can inspect the selected pull request in a browser
  without checking out its branch.
- As a reviewer moving between pull requests, I see only content belonging to
  the pull request named by the viewer.
- As a security-conscious user, I do not expose provider credentials or an
  unauthenticated local diff endpoint to browser pages.
- As a provider user, a failed viewer load does not generate unbounded retries.

## Acceptance criteria

1. The browser server binds only to loopback and every document, API, and file
   route requires an unpredictable per-server session token.
2. A cached diff is supplied to the viewer only when its repository and pull
   request identity match the selected pull request.
3. A failed provider population attempt is shown as a bounded failure and is not
   retried by periodic browser polling; reopening the viewer permits an explicit
   retry.
4. Browser image previews allow only bounded raster formats. SVG or another
   active/unknown format is not served with an executable image document MIME.
5. Response headers prevent cross-origin reads, framing, MIME sniffing, referrer
   disclosure, and unrestricted resource loading.
6. Focused tests cover session authentication, request bounds, pull request
   identity, bounded retries, preview MIME allowlisting, and browser-state
   updates.

## Out of scope

- Replacing the native terminal diff viewer.
- Publishing comments or changing pull requests from the browser viewer.
- Binding the viewer to a LAN or public interface.
- Persisting browser sessions across TUI runs.

## Open questions

- None.

## References

- ./0002-http-in-rust.md
- ./0003-credentials-keychain.md
- ./0006-terminal-ui.md
- ../../src-tauri/src/tui/diff_server.rs

## Revision History

| Date | Revision | Author | Change |
|------|----------|--------|--------|
| 2026-09-02 | r1 | default-agent | Proposed an authenticated browser diff viewer and its security boundaries. |
| 2026-09-02 | r2 | default-agent | Accepted the browser viewer after maintainer review. |

## Approvals

| Role | Name | Date | Signature |
|------|------|------|-----------|
| Maintainer | delaudio | 2026-09-02 | approved remediation and implementation in chat |
