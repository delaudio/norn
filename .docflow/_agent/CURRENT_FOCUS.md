# Current Focus

This file is the live snapshot of any in-flight session. It is short on
purpose; the durable record lives in git, `_agent/WORKLOG.md`, and
`plan/done/`. Queued work lives in `plan/todo/`.

If status files and git disagree, git is authoritative; correct this file.

## Active state

- **Branch:** `main`.
- **Active item:** prepare the command-only Norn v0.2.8 release.
- **Plan items:** none; release execution follows the implemented distribution
  contract.
- **Verification:** PR #230 merged at `32c51e6`; typecheck, 104 frontend tests
  plus tooling suites, Archgate 17/17, general CI, and the Homebrew contract
  pass after browser viewer delivery.

## Last shipped

`32c51e6` - ship the authenticated browser diff viewer through PR #230.

## Next item

- `.docflow/plan/todo/0001-agentic-code-policy-pack.md` is the lowest-numbered
  remaining queue item.
