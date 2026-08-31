# Current Focus

This file is the live snapshot of any in-flight session. It is short on
purpose; the durable record lives in git, `_agent/WORKLOG.md`, and
`plan/done/`. Queued work lives in `plan/todo/`.

If status files and git disagree, git is authoritative; correct this file.

## Active state

- **Branch:** `docs/close-homebrew-formula-temporary-tap`.
- **Active item:** close the issue #213 delivery record after PR #214 and
  prepare the signed `v0.2.4` recovery release.
- **Plan item:** `.docflow/plan/done/2026-09-01-homebrew-formula-temporary-tap.md`,
  owned by implemented ADR 0014.
- **Verification:** PR #205 shipped at `643ff6f` and its Docflow closeout at
  `d78e108`. Version sources were aligned to `0.2.0` through PR #207. The
  Homebrew tap credential is configured. PR #208 shipped the command-only
  release path at `24d9ce1`; typecheck, 104 frontend tests plus tooling suites,
  Tauri IPC smoke, build, Archgate 17/17, and bounded Norn review passed. Apple
  signing and notarization remain intentionally unavailable. The signed
  `v0.2.0` tag was retained as immutable after run `33432277406` failed before
  creating a GitHub release or modifying the public tap. PR #210 installed the
  Tauri Linux prerequisites; the signed `v0.2.1` tag then passed that step but
  run `33433900934` stopped at two setup tests before release publication. PR
  #211 fixed those tests; run `33437187047` published the complete `v0.2.2`
  command candidate as a prerelease, then skipped formula smoke, stable
  promotion, and tap publication through the transitive desktop skip chain.
  PR #212 forced explicit downstream result evaluation and version `0.2.3`.
  Run `33439848231` then built both command archives and correctly started the
  Formula smoke matrix, but both runners rejected direct Formula-path
  installation before stable promotion or tap publication. PR #214 fixed the
  lifecycle with an isolated temporary tap, explicit candidate-tag resolution,
  and schema-checked readiness outcomes. Clean-runner lifecycle run
  `33444357192` passed on Intel and Apple Silicon.

## Last shipped

`7e9c759` - validate the Formula lifecycle through a temporary tap and close
issue #213 through PR #214.

## Next item

- Publish and monitor the signed `v0.2.4` recovery tag, verify stable GitHub
  promotion and public Formula tap publication, then remove the one-time
  bootstrap variable.
