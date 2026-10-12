# Native external provider PR navigation

Internal evidence for retained #306 navigation, plan0061.

The provider PR list/detail accepts `b` as an explicit page-open action. Its
request carries full provider/workspace/repository/ordinary PR identity; it
contains no URL. Rust validates this before any credential/launcher access and
constructs only canonical HTTPS GitHub/Bitbucket PR URLs. The action is separate
from Pierre and terminal copy URL. Older backends report that the action is
unavailable instead of sending an unsupported request.

Ordinary slugs need no provider API or credentials. Bitbucket workspace/repo
UUIDs are resolved with a bounded authenticated native repository GET under the
existing two-read gate and 5/10/20-second deadlines. Response repository/workspace
UUIDs must match the requested identity, aliases must pass normal identity/path
validation, and native `full_name` supplies the actual web slugs. Metadata HTML
links and redirects are never opened. Missing/corrupt/foreign metadata fails
explicitly instead of guessing a web URL or substituting another repository.

The shared one-worker browser gate covers external and Pierre opens. Rust
checks shutdown before URL resolution and again before launch; native openers
reuse bounded Unix child ownership/null stdio/reaping and Windows native shell
dispatch. Results acknowledge a launch request and report launcher failure;
none claim the page was displayed. With `NORN_BROWSER_OPEN=0`, fixtures return
a disabled acknowledgment and start no process. The UI preserves current
selection/generation, coalesces repeated actions and discards late results after
navigation or panel disposal. Pasted shortcut characters never launch pages.

Owned HTTP fixtures verify canonical URLs, no provider read for slugs, UUID
ownership/alias/path/redirect rejection and ignoring hostile metadata links.
Real stdio exercises both providers without credentials or a launcher and proves
subsequent control/shutdown responsiveness. Strict client DTO/echo/canonical URL
checks reject foreign hosts/credentials/ports/local-diff URLs. Native input tests
cover list/detail at 120 and 50 columns; wide/narrow disabled-opener frames are
included in Storybook. Existing launcher tests remain in force. No live provider
writes, private credentials/profiles or native Windows runtime claims are made.

Explicit provider approval, profile catalog/editor and remaining parity/native
recovery/platform/install/cutover stay required. This delivery closes none of
#306, #364, #307–#309 or epic #294 and does not advance the owning architecture
migration decision.

References:
- [Bitbucket repository metadata](https://developer.atlassian.com/cloud/bitbucket/rest/api-group-repositories/)
- [Bitbucket UUID and slug identity](https://developer.atlassian.com/cloud/bitbucket/rest/intro/)
- [Bitbucket repository/workspace payload fields](https://support.atlassian.com/bitbucket-cloud/docs/event-payloads/)
