# Desktop user-data recovery (internal migration evidence)

Owning decision: [0020](../../.docflow/adr/0020-opentui-rust-stdio-workspace.md).
Owning issue: #305. This document records the implementation and recovery gate;
public recovery instructions must omit internal catalogue references.

## Inventory and format

The desktop keeps user data in origin-scoped DOM localStorage. Allowlisted keys:

| Content | Canonical prefix | Legacy fallback | Rust scope |
| --- | --- | --- | --- |
| Staged comments | `norn.drafts.v1.` | `lachesi.drafts.` | provider, owner, repository, PR |
| Review references | `norn.reviewReferences.v1.` | `lachesi.reviewReferences.` | explicit provider, owner, repository, PR |
| Viewed files | `norn.viewedFiles.v1.` | `lachesi.viewedFiles.` | explicit provider, owner, repository, PR |
| User review prompt | `norn.reviewPrompt.v1.` | `lachesi.reviewPrompt.` | explicit provider, owner, repository |
| Theme | `norn.theme.v1` | `lachesi.theme` | global |

Rust review runs/findings already live in the existing SQLite store and are not
webview migration input. Secrets remain in the OS store/environment. Menu-bar
snapshots and other transient caches are excluded. The exporter neither reads
credential keys nor copies arbitrary storage keys. User-authored text is preserved
as content; exports contain private review material and should be stored locally.

The desktop exporter emits `{schemaVersion:1,source:"desktopLocalStorage",entries:
[{key,value}]}`. Values retain the original storage serialization; user prompts
and theme are plain strings. Rust normalizes to `{schemaVersion:1,records:[{key,
payload:{kind,value}}]}` with typed drafts/references/viewed/prompt/theme values.
Draft finding/publication identity and original timestamps survive migration.
Numeric legacy parent IDs normalize to strings. Canonical keys win over matching
legacy keys, including an intentionally empty canonical value. Invalid entries
are reported by index with generic errors; their raw text is never echoed.

Limits: 16 MiB file, 10,000 records, 256 KiB desktop value, 64 KiB draft/reference
body or prompt; at most 1,000 drafts/references per scope. Native preview detail
lists are capped at 200 per category with complete counts and an explicit truncation
flag. Source files, symlinks and corrupt destination files fail closed.

## Normal route

In the existing desktop's Settings, choose **Export local drafts, references and
preferences**. Verify that the downloaded JSON exists and retain a copy. No export
writes localStorage, deletes a profile or migrates credentials.

In the terminal, `m` opens settings and Ctrl+D opens user data. Enter edits a path;
Tab navigates the import path, historical provider and new export path. Ctrl+P
previews the selected input without creating/changing the destination. For old
provider-less reference/viewed/prompt scopes, select the historical provider
explicitly. If a single old profile contains both providers with identical scopes,
the old storage never distinguished them: retain the export, split entries by
known repository ownership, preview each split and resolve ambiguity manually.
Do not infer a provider from the current default or silently import twice.

Ctrl+Y confirms the current preview. Confirmation is fenced by a SHA-256 of the
source bytes plus provider choice and by a destination version. Changes invalidate
the preview. Existing conflicting scopes are retained in full, including newer
empty records; the original export remains the recovery source. Identical repeated
imports make no writes and no duplicates. Rejected entries are not imported; the
result reports their count and reasons. An additions-only import creates a verified
backup of the preceding destination before atomic replacement. Interrupted writes
leave either the preceding or complete new document; a leftover verified backup
can be reused on retry. No automatic replacement/merge of conflicting draft lists
is attempted. A process mutex and persistent sidecar file lock serialize canonical import
and per-record writers across processes. Corrupt bytes are retained and
conflicting scope records still require explicit reconciliation.

Equivalent headless route:

```
norn data preview /absolute/desktop-export.json --provider github
norn data import /absolute/desktop-export.json --provider github \
  --input-version <preview-input-version> \
  --destination-version <preview-destination-version> --confirm
norn data export /absolute/new-backup.json
```

JSON output reports counts, rejected indexes, conflicts and backup path. Exit 0
means no conflicts/rejections, exit 3 means partial import or unresolved conflicts,
exit 2 means invalid arguments or a failed operation. Export never overwrites an
existing file. The canonical file is `workspace-data-v1.json` under Norn's local
data directory (`NORN_DATA_DIR` is the isolated-test override). Backups are named
`workspace-data-before-<destination-version>.json` beside it.

## Already-upgraded route and platform inventory

The recovery route uses the archived working desktop export bridge through DOM
localStorage, rather than guessing WebKit SQLite or Chromium LevelDB internals.
WebKit manages origin-scoped storage through SQLite storage areas ([WebKit storage
architecture](https://docs.webkit.org/Deep%20Dive/Architecture/Storage.html));
WebView2 keeps LocalStorage in a profile-specific user-data folder ([Microsoft
user-data folders](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/user-data-folder)).
Those formats and hashed subdirectories are implementation details, not a stable
cross-platform import contract. Read-only external database parsing is not claimed.

Close Norn and its webview processes, then copy the complete canonical and legacy
profile directories to separate backup directories before opening a recovery
build. Keep those original backups through cutover. Norn's existing profile
migration copies legacy data without removing the source. Inventory in
`runtime_identity.rs`:

| Platform | Canonical profile root | Legacy root |
| --- | --- | --- |
| macOS | `~/Library/WebKit/app.norn.desktop` | `~/Library/WebKit/app.lachesi.desktop` |
| Linux | local data directory / `app.norn.desktop` | local data directory / `app.lachesi.desktop` |
| Windows | local data directory / `app.norn.desktop` | local data directory / `app.lachesi.desktop` |

Actual runtime subfolders and origins vary. Preserve the **whole root**, including
journal files, and verify the exporter sees the expected entry counts. Never
point recovery at an unrelated browser profile. If an empty canonical profile
hides a populated legacy one, preserve both and restore a **copy** of the legacy
root into a separate recovery environment using the original application ID.
Do not delete/replace the user's live canonical root to force a fallback.

An already-upgraded user can obtain the archived source revision containing the
bridge and build it in a separate checkout on macOS, Linux or Windows with the
same prerequisites as the old desktop. The archive reference is pinned to `72440eaef4177b809664bf6b563647d7659f3e52` in
[public recovery instructions](../user-data-recovery.md); installed platform builds
remain a required verification before #309 removes the desktop. Install dependencies,
build production frontend assets, and compile with `custom-protocol,desktop-bundle`
so the original production origin is used (a Vite dev origin has different storage).
Run the resulting desktop binary directly from that checkout, export, close it,
and return to the terminal; do not install it over the new command distribution.
A separate OS user/VM with a copied profile is the safest way to recover a preserved
legacy-only profile without modifying live canonical state. This route does not
require having opened the old app before upgrading. If neither profile nor a
backup remains, there is no invented recovery claim: report the missing source.

## Evidence and remaining consumption gates

Synthetic desktop DOM exports are independent of native storage encoding and
exercise canonical/legacy input, credentials/cache exclusion, changed/oversized
source and no source writes. Rust tests prove draft metadata preservation, numeric
parent normalization, provider fencing, bounded preview counts, stale/corrupt/
oversized/symlink refusal, backup, repeats and conflicts. The real backend and CLI
integration use temporary config/data with no OS credential access. Terminal tests
and native frames cover preview, rejection/conflict display, explicit confirmation,
failed import and cancellation/input fencing.

This delivery supplies canonical persistence/import ownership. Viewed-file navigation and prompt consumption use these canonical records.
Imported themes can be explicitly previewed/staged in terminal settings, then
saved through the existing settings version fence. Draft publication and
reference UI/enrichment remain coordinated with #304/#306 before final parity
is declared complete. Platform
recovery builds and the exact archived source reference must be verified/pinned
before cutover. No cross-platform installed recovery test is claimed by Linux-
or macOS-hosted synthetic exports alone.

## Archived bridge source verification

`node scripts/verify-recovery-bridge.mjs` checks the pinned merged revision,
original application ID, embedded production frontend/custom-protocol feature,
registered Settings export action, and executes that **archived** exporter
in a bounded isolated JavaScript context. Six synthetic canonical/legacy DOM
fixtures labeled for macOS WebKit, Linux WebKitGTK and Windows WebView2 prove
the shared DOM export format, source retention and cache/credential exclusion.
Those labels do not simulate the native storage encodings or demonstrate
actual platform profile recovery. The script uses local git history only;
fetch the pinned revision if working in a shallow checkout.

The exact complete exporter revision is now the merge of its delivery PR,
not an intermediate implementation commit. The source evidence is reproducible
without launching a desktop, accessing a user profile or reading a credential.

The `recovery-bridge` workflow compiles this immutable archive with a production
frontend/custom protocol on Linux, macOS and Windows and runs isolated profile
directory-copy tests. It never launches a webview or accesses the runner's native
credential store. These are build/copy gates, not a native user-profile recovery
claim. The same archived production frontend and embedded desktop compile
passed locally on macOS 26.2; no recovery desktop was launched.

The first Windows archive run compiled the desktop successfully, but its
GUI-linked unit-test executable failed before running any test with
`0xc0000139 / STATUS_ENTRYPOINT_NOT_FOUND`. The isolated profile-copy lane now
uses the archive's engine-only `--no-default-features` unit build; the separate
full desktop compile/custom-protocol gate is retained. This narrows the copy
regression to its required filesystem dependencies and does not claim the
Windows desktop runtime/loading problem is diagnosed or repaired. Actual
native recovery remains a distinct final gate. Contract-only tooling imports
the revision constant without eagerly requiring TypeScript; transpilation is
loaded only when the installed-dependency source verifier executes.
