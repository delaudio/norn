# Terminal readiness and first run

The workspace keeps settings, credentials and machine checks in Rust. Existing
users load their saved configuration and use the existing credential store or
environment; the terminal never asks for a stored secret to be returned. Empty
configuration shows a first-run guide: `m` settings, `a` add repository, enter
provider/owner/name, save preferences with Ctrl+S, save a **new** masked credential
separately with Ctrl+K. Cancelling keeps persisted settings unchanged. Ctrl+D
opens explicit desktop-data preview/import/export.

Settings show status-only existing credentials and tool presence; Ctrl+R
refreshes tool paths without erasing unsaved edits. Repeated input coalesces to
one request. Disposal/selection generations fence late status/save results.
Successful preference saves refresh the selected-agent status. No refresh
starts a review, logs in, publishes a comment or opens a browser.

`workspace.readiness` returns only configured repository count, selected agent,
availability of Git/Claude/Codex/OpenCode, canonical config/data-root resolution
and the native-store support of this build. PATH scans check executable files
(Windows includes .exe/.cmd/.bat). They execute **no binaries**, read no stored
credentials and perform no repository/policy scan. The result includes neither
raw tool/version output nor executable paths. Root availability means the path
can be resolved; it is not an assertion that storage is writable or that a
provider is reachable. Existing `norn doctor`, `norn setup` and `norn auth`
remain the fuller headless diagnostics/onboarding workflows.

Tool presence does **not** verify agent authentication or provider connection.
PATH-only detection preserves the fast legacy startup scan; a tool reachable
only through additional shell initialization can be missing here. Actual review
launch retains the existing Rust adapter and its failure handling. The terminal
does not block a review merely because this presence check is negative.

The current Cargo keyring feature set enables Apple native credentials only.
Other builds explicitly report the native-store gap (tracked in #251) and keep
existing environment-based credentials; no plaintext secret fallback is added.
This is compile-time support, not a claim that a particular keychain is unlocked.
Jira/Notion status reads/writes retain their existing adapter semantics and are
separate from the machine-only readiness request.

Tests use fake tool/status probes and isolated settings. A real IPC regression
puts a synthetic executable on an isolated PATH and proves readiness finds it
without running it; no credential query occurs. Native fixtures include wide,
narrow and unsupported-store first-run views. Desktop user-data recovery and
record consumption remain separately tracked before final cutover.

The existing command-distribution onboarding regression previously invoked
production `doctor`, which can query the maintainer's keychain and blocked on
macOS in this session. That run was interrupted and does not count as verified.
The corrected regression runs the real doctor argument parser/engine in an
isolated test child with a mock keyring, empty inherited environment, synthetic
version-only CLIs and isolated HOME/config/data. Actual CLI `doctor --help`
routing remains covered without any probes. Init and data round trips continue
to exercise the real command-distribution binary with isolated storage.
