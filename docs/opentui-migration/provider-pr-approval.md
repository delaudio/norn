# Native human PR approval

Internal migration evidence for #306 / plan 0062, implementing the retained
human PR action under ADR0020 without requiring an AI review or repository clone.
This is separate from staged comment publication and filesystem fix approval.

## Interaction and boundary

In the provider PR list/detail, `a` loads a read-only native approval preview.
The preview displays the full repository/PR, actual authenticated reviewer,
head/destination commits, branches and explicit remote effect. `y` confirms only
that preview; `Esc` dismisses an unsent preview. Pasted keys cannot confirm.
Repeated confirmation consumes the UI token immediately and the Rust token once.
A send already in progress cannot be canceled by closing its panel. `s` reads
status, `r` checks an unresolved send with GETs, and `b` opens the provider page.

The generated protocol exposes `provider.approval.preview`, `.confirm`, `.status`
and `.reconcile`; ready advertises `provider-pr-approval`. Every request is scoped
to a validated native provider/repository/ordinary PR. Only confirm accepts an
opaque approval ID; callers cannot supply an actor, HTTP URL/body or success
assertion. Rust keeps at most 32 tokens, each expiring after five minutes, and
consumes a token before credentials/network access, including a cross-scope use.
UI requests and replies validate strict generated DTOs and current selection.

Native metadata reads use the shared two-read budget, fixed authenticated API
origins, disabled redirects, 1 MiB body limits and a 20 second total deadline.
Approval requests serialize independently of comments. Reads prove immutable
native destination repository IDs and stable authenticated reviewer IDs: GitHub
numeric user ID and Bitbucket account ID. Preview reads current metadata twice
and includes both pins, branches, title, fork, state and draft status. Confirm
rechecks that proof and refuses drift, draft/closed PRs or duplicate approval.
No agents, Git processes, clone guesses or browser credentials are involved.

GitHub sends exactly `event: APPROVE` with the captured `commit_id`; success
requires an acknowledged review ID, reviewer ID, APPROVED state and commit ID.
Bitbucket uses the native empty-body approve POST and validates its acknowledged
participant, followed by another metadata/status read. Bitbucket has no commit
CAS argument: a concurrent metadata change or missing post-send proof remains
uncertain rather than being reported as approval of the captured commit.

## Durable attempts and recovery

SQLite schema 19 adds `provider_pr_approvals`, bounded to 1,000 records of at most
64 KiB. Each record has validated native parent ownership with cascading deletion
and participates in existing consistent SQLite backup/export. The separate
canonical workspace-record JSON export does not carry this journal. New owner creation and the
writing claim happen in one immediate transaction; SQLite FULL sync is required
before POST. An invalid/full journal prevents the remote send. Existing review
rows and legacy evidence are not rewritten.

The immutable action key includes provider, native repository/principal IDs,
PR number and both commits. Unresolved writing/uncertain records also block new
POSTs for the same actual repository/PR/reviewer across changed heads. A renamed
or case-variant repository alias cannot bypass that guard or read another saved
owner's payload. Rejected attempts allow a new explicit preview/confirmation;
approved or unresolved attempts do not silently retry.

Validated acknowledgments transition to approved. Definitive 4xx rejection stays
separate from transport errors, 5xx/redirects, dropped/slow/malformed/foreign or
oversized acknowledgments. Unknown outcomes stay uncertain. Failure to persist
the final receipt leaves the pre-send writing claim intact and reports local
receipt uncertainty; it never says the approval was not sent. Restart recovery
uses durable records and GETs only. GitHub can verify a review for the stored old
head and report that the current PR has changed. Bitbucket's mutable participant
flag cannot prove a historical commit after either pin changes, so it remains
uncertain. Provider-page recovery stays available; there is no automatic resend
or undocumented local clearing of a possibly accepted write.

Schema-18 and older binaries refuse this newer database. Packaging rollback must
restore a verified compatible owned backup with its matched UI/backend version;
never open the upgraded database with an old binary or copy a live SQLite file.
This remains part of #308's actual installed upgrade/rollback gate.

## Verification and remaining scope

Owned fake API/storage tests cover single-use, expiry, foreign scope, actor/repo/
metadata drift, durable pre-POST claim, uncertain restart, changed-head blocking,
GET recovery, rejected attempts, alias guards, stale receipt CAS, corruption,
quota, failed receipt persistence, verified consistent SQLite backup and deletion cascade. Owned loopback HTTP
fixtures exercise both real adapters, exact POST bodies, accepted-write dropped
acknowledgment, malformed/foreign/oversized/redirect/rate/5xx/deadline responses,
and GitHub historical versus Bitbucket unpinned recovery. Real stdio verifies
capability, offline invalid-token consumption and continued control after restart.
Native UI tests verify explicit keys, paste isolation, stale/late/canceled/expired
previews and repeated confirmation at 120 and 50 columns. Native frame fixtures
cover preview and uncertainty at both widths.

No live provider writes, private credentials/profiles or public releases are
used in these tests. This delivery does not close #306, complete ADR0020, claim
native-profile recovery or installed platform parity, or waive #307–#309.
Profile catalog/editor and the remaining parity rows still need implementation.

## Native API references

- [GitHub create/list PR reviews](https://docs.github.com/en/rest/pulls/reviews):
  explicit commit_id and APPROVE action, authenticated acknowledgment, paginated
  chronological review evidence.
- [Bitbucket approve PR](https://developer.atlassian.com/cloud/bitbucket/rest/api-group-pullrequests/#api-repositories-workspace-repo-slug-pullrequests-pull-request-id-approve-post):
  native participant approval with no commit CAS input.
