# Terminal Image Preview

## Owning ADRs

- `../../adr/0020-opentui-rust-stdio-workspace.md`

## Scope

Port the terminal "single preview" image parity: when the selected changed file
is an image, the OpenTUI shell renders its new side with the native `<image>`
element, or shows an explicit fallback instead of an empty diff panel. The
backend adds a `file.preview` request that returns the bounded new-side bytes
(from the reviewed head for stored PR targets, or the checkout otherwise) as
base64 plus MIME type and size.

GitHub issue: #302 (epic #294 step 08/15).

Out of scope (tracked): before/after image comparison, and provider-side image
fetches when the reviewed revision is not available locally.

## Exit Criteria

- `file.preview` returns `{ path, mimeType, size, dataBase64 }` for supported
  image types, bounded to 2 MiB, and errors for unsupported types, oversized
  files, or missing revisions.
- The shell renders `<image>` for the selected image file, decodes base64 once,
  and falls back to a warning when the terminal cannot render inline images.
- Preview loading is fenced by a request id and runs independently of the text
  diff; preview concurrency is bounded.
- `cargo test`, `cargo clippy -D warnings`, `pnpm run lint/typecheck/test`, the
  `ui` job and `frames:check` pass.

## Dependencies

- `../../adr/0020-opentui-rust-stdio-workspace.md`
- `../../../docs/opentui-migration/migration-contract.md`
