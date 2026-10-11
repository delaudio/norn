# Recover local desktop review data

Norn can recover desktop drafts, review references, viewed files, custom prompts
and theme into local Rust-owned storage. Credentials remain in the OS credential
store or environment. Existing reviews in the native database stay in place.

Open Settings in the desktop and choose **Export local drafts, references and
preferences**. Keep the downloaded JSON and a copy of the complete webview profile.
Export never deletes or modifies the source. The file contains private review
content: keep it local and do not include it in a support report or repository.

In the terminal, press `m`, then Ctrl+D for user data. Enter the export's absolute
file path. Tab moves between fields; Enter edits a path or cycles the historical
provider. Select GitHub or Bitbucket explicitly for old references, viewed files
and prompts, whose desktop keys did not record a provider. If both providers used
the same owner/repository identity, preserve the original export and split entries
by known repository ownership before importing; no automatic disambiguation can
recover information that the old storage never recorded.

Ctrl+P previews without writing. Review additions, rejected entries and conflicts.
Ctrl+Y confirms additions from that preview. Changed source, provider or destination
invalidates it. Existing conflicting scopes are kept, never silently overwritten.
A backup of the preceding destination is written before any replacement. Repeating
an identical import does not duplicate records or change the destination. Rejected
entries remain recoverable in the original export; correct a copy and preview it
again. Ctrl+E exports canonical records to a new file and refuses to overwrite an
existing file. Escape closes; while editing, Escape undoes that field first.

The same workflow is available without a terminal interface:

```
norn data preview /absolute/export.json --provider github
norn data import /absolute/export.json --provider github \
  --input-version <inputVersion-from-preview> \
  --destination-version <destinationVersion-from-preview> --confirm
norn data export /absolute/new-backup.json
```

Exit 0 means no rejected entries or conflicts, 3 means partial import or unresolved
conflicts, and 2 means a failed operation or invalid arguments. Keep the source and
backup until every expected draft/reference/prompt is accounted for. The source
file limit is 16 MiB and desktop values are limited to 256 KiB; split a copy of a
larger export, retaining its format and original keys. Native preview details are
bounded to 200 per category; counts cover the entire export. New data and backups
live beside `workspace-data-v1.json` in Norn's local data directory.

## If you have already upgraded

You do not have to have opened the old desktop before upgrading. If its profile
or a profile backup remains, use the archived export bridge from source revision
[`34a3c2a2b56b698ef8cf9978f0b1e0a7ba4402db`](https://github.com/delaudio/norn/tree/34a3c2a2b56b698ef8cf9978f0b1e0a7ba4402db)
in a separate checkout. This revision preserves the original desktop application
ID and export action. Obtain it from the repository's history/archived source;
use its lockfile and desktop build prerequisites. Do not install it over the new
command distribution.

Close Norn and all its webview processes before copying profile data. Preserve
complete profile roots, including journal files, in separate backup directories:

| Platform | Canonical root | Legacy root |
| --- | --- | --- |
| macOS | `~/Library/WebKit/app.norn.desktop` | `~/Library/WebKit/app.lachesi.desktop` |
| Linux | local data directory / `app.norn.desktop` | local data directory / `app.lachesi.desktop` |
| Windows | local data directory / `app.norn.desktop` | local data directory / `app.lachesi.desktop` |

Subdirectories and origins depend on the webview runtime. Preserve the whole root;
do not edit SQLite/LevelDB files or point recovery at an unrelated browser profile.
If only a legacy profile contains the data, preserve both live roots and recover
from a **copy** in a separate OS user/VM with the original application ID. Do not
remove a live canonical profile to force a legacy fallback.

From the archived checkout, build a production desktop, so it uses the original
production origin instead of a Vite development origin:

```
pnpm install --frozen-lockfile
pnpm exec tauri build --no-bundle --features desktop-bundle
```

Run `src-tauri/target/release/norn` directly on macOS/Linux, or
`src-tauri/target/release/norn.exe` on Windows. The Tauri production build enables
its embedded frontend; a browser development server has separate localStorage.
Export from Settings, verify that the expected entry counts and keys are present,
close the recovery desktop and import with the current terminal/CLI. An empty
export is not proof that older profiles contained no data: check the preserved
profile/origin and retain every backup until recovery is resolved.

The importer never invents missing content. If both the original profile and all
backups were deleted, report the missing source rather than treating a new empty
profile as a successful migration. Current import tests use synthetic exports;
platform-specific recovery builds are checked before final desktop retirement.
