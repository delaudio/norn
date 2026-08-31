# Current Focus

This file is the live snapshot of any in-flight session. It is short on
purpose; the durable record lives in git, `_agent/WORKLOG.md`, and
`plan/done/`. Queued work lives in `plan/todo/`.

If status files and git disagree, git is authoritative; correct this file.

## Active state

- **Branch:** `fix/release-skipped-dependency-chain`.
- **Active item:** recover the first governed Norn release as `v0.2.3` after
  the `v0.2.2` candidate exposed transitive GitHub Actions skip propagation
  from the intentionally disabled desktop build.
- **Plan item:** `.docflow/plan/done/2026-08-31-cli-only-release-channel.md`,
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

## Last shipped

`86f23d5` - make setup tests environment-independent through PR #211.

## Next item

- Require downstream command-only jobs to evaluate predecessor results with
  `always()`, align version sources to `0.2.3`, and publish the next signed
  recovery tag after review and integration.
