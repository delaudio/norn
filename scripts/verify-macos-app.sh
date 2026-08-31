#!/usr/bin/env bash

set -euo pipefail

app_path="${1:-}"
if [[ -z "$app_path" || ! -d "$app_path" ]]; then
  echo "A built Norn.app path is required." >&2
  exit 1
fi

identifier="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$app_path/Contents/Info.plist")"
executable_name="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleExecutable' "$app_path/Contents/Info.plist")"
executable="$app_path/Contents/MacOS/$executable_name"
test "$identifier" = "app.norn.desktop"
test -x "$executable"

codesign --verify --deep --strict "$app_path"
spctl --assess --type execute "$app_path"
xcrun stapler validate "$app_path"

open -na "$app_path"
pid=""
for _ in {1..40}; do
  if ! pid="$(pgrep -f -x "$executable")"; then
    pid=""
  fi
  if [[ -n "$pid" ]]; then
    break
  fi
  sleep 0.5
done
if [[ -z "$pid" ]]; then
  echo "Norn.app did not remain running after launch." >&2
  exit 1
fi

osascript -e 'tell application id "app.norn.desktop" to quit'
for _ in {1..20}; do
  if ! pgrep -f -x "$executable" >/dev/null; then
    exit 0
  fi
  sleep 0.5
done
echo "Norn.app did not quit cleanly after its launch check." >&2
exit 1
