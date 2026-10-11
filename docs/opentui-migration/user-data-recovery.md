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
is attempted. A same-process mutex serializes this service; unrelated external
writers are not covered by an operating-system compare-and-swap.

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
same prerequisites as the old desktop. The archive reference will be pinned in
public recovery instructions before #309 removes the desktop. Install dependencies,
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

This delivery supplies canonical persistence/import ownership. Draft publication,
reference UI, viewed-file navigation and prompt consumption must use these records
in #304/#303/#306 before #305 and final parity are declared complete. Platform
recovery builds and the exact archived source reference must be verified/pinned
before cutover. No cross-platform installed recovery test is claimed by Linux-
or macOS-hosted synthetic exports alone.
