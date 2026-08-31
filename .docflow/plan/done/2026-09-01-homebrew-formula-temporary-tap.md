# Homebrew Formula Temporary Tap Validation

## Owning ADRs

- `../../adr/0014-reproducible-release-and-upgrade-channels.md`

## Scope

Implement GitHub issue #213 by moving Formula lifecycle validation from direct
Ruby-file installation to an isolated temporary Homebrew tap. Exercise the
candidate through its fully qualified tap reference, replace the tap Formula
when validating a previous-to-candidate upgrade, and remove the Formula and tap
on both success and failure.

Out of scope: changing the public tap repository, enabling the desktop Cask
channel, adding an Apple signing requirement, or changing supported platforms.

## Exit Criteria

- ADR 0014 AC3-4: clean Formula installation and testing use a fully qualified
  temporary-tap reference on Intel and Apple Silicon runners.
- ADR 0014 AC5: previous-to-candidate validation replaces the Formula inside
  the same temporary tap before exercising Homebrew upgrade behavior.
- ADR 0014 AC8: lifecycle cleanup removes the installed Formula and temporary
  tap without masking the original failure.
- Contract coverage rejects direct Formula-path installation in the lifecycle
  script.
- The repository verification gate passes: `pnpm run typecheck`, `pnpm run
  test`, and `archgate check`.

## Dependencies

- `../../adr/0014-reproducible-release-and-upgrade-channels.md`
- GitHub issue #213

---

Shipped at HEAD `7e9c7592840411240d45d3722dc9346aabc48a86` through PR #214. Clean-runner
Homebrew lifecycle run `33444357192` passed on Intel and Apple Silicon.
