# Shared Review Prompt and Payload

## Owning ADRs

- `../../adr/0020-opentui-rust-stdio-workspace.md`

## Scope

Extract the review prompt resolution and payload rendering into a shared core
module (`services::review_prompt`) so the headless engine and the stdio
backend's future inline review session reuse one implementation. The headless
output is unchanged; the payload builder is parameterized by the scope label and
note so callers keep their exact formatting.

GitHub issue: #303 (epic #294 step 09/15).

## Exit Criteria

- `services::review_prompt` owns `resolve_review_prompt`, `build_review_payload`
  and `markdown_fence`; the headless engine imports them.
- Unit tests cover prompt resolution (default/extend/replace) and payload
  rendering (fence widening, target header, whitespace preservation); the
  existing headless payload tests pass unchanged.
- `cargo clippy -D warnings`, `cargo test`, `pnpm run lint/typecheck/test`, the
  `ui` job and `archgate check` pass.

## Dependencies

- `../../adr/0020-opentui-rust-stdio-workspace.md`
- `../../adr/0008-shared-review-service-boundary.md`
