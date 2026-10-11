# Syntax, explicit review copy and native image smoke

## Syntax and source fidelity

The terminal uses the same Refractor 5 language mapping as the existing desktop
highlighting. It reconstructs old/new source separately inside each hunk so
multiline comments/strings retain context. Unified and split rows retain their
original diff prefixes, source numbers, Unicode and exact text. Anchored/search
rows use the existing highlight color. Only visible rows mount colored spans;
syntax does not replace the geometry text or change source identities.

Coloring is optional: unknown grammar, more than 128 KiB/3,000 raw rows, a line
over 4,096 characters, parse/start failure or a 1.5-second timeout leaves the
complete diff available with a plain-text notice. Syntax tree depth is bounded
to 64, visited nodes to 100,000 and rendered fragments to 16,000. Parsed output
is source-checked before it reaches the renderer. At most one result is cached
by selected path/content hash; selection generations fence stale results.

The first complete suite exposed a **Bun 1.3.8 WebWorker host segmentation fault**
while tests created/cancelled multiple tokenizers. Production therefore runs the
optional tokenizer in an owned **Bun subprocess**, without a shell or inherited
provider/configuration environment. Its stdout/stderr are ignored and an IPC
channel carries only selected diff text and tokens. Cancellation kills and reaps
that process before starting a replacement; queued selections coalesce to the
latest. Parent disconnect also exits the tokenizer. Termination failure stops
further replacements and retains plain text. The regression includes 100 rapid
selections plus timeout/malformed/start-failure cases. This is observed local
runtime evidence, not a claim about other Bun releases.

Packaging must include the tokenizer entrypoint/Refractor dependencies and a
matching Bun invocation, then test it in the **installed pair**. Development
subprocess coverage does not satisfy that later gate.

## Explicit clipboard action

Press `y` with a stored PR review selected. Rust's typed `review.output` reads
the exact selected run summary; provider/workspace/repository/PR/run identity
must match. Missing run/output and output above **64 KiB UTF-8** fail without
returning an excerpt. Thread selection pins the associated run; this action
copies that run's stored summary, not the bounded conversation preview.

The frontend invokes OpenTUI's OSC52 clipboard bridge only after explicit input
and a successful, size/identity-checked read. Navigation or backend closure
fences a pending read; repeated input shares one pending request. The terminal
may reject OSC52 by capability or policy. A successful emission says to paste
and verify; it cannot acknowledge the user's remote/system clipboard. Tests use
fake captures only and never touch the maintainer's clipboard.

## Physical image checklist

Run this synthetic scene in Kitty or a Sixel-capable terminal **outside tmux**:

```sh
cd ~/dev/current/norn
bun run ui/scripts/image-terminal-smoke.ts
```

The scene uses in-memory images and no backend, provider, credentials, settings
or actual review data. Confirm both BEFORE/AFTER colors are visible; resize
across wide side-by-side/narrow stacked layouts; press `q` and confirm normal
terminal input is restored. Record terminal/version, graphics protocol, OS,
dimensions and the observed result. Browser opening is deliberately unavailable
in this isolated demo. `--help` works without a terminal.

Native frame/color tests and image decoder/protocol tests remain automated
evidence; they do not establish physical display. **No physical Kitty/Sixel
result has been observed in this agent session yet.** That check remains a
cutover gate pending an actual run, not an inferred success.
