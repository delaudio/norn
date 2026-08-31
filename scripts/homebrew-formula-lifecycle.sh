#!/usr/bin/env bash

set -euo pipefail

bootstrap="${NORN_HOMEBREW_BOOTSTRAP:-false}"
if [[ "$bootstrap" != "true" && "$bootstrap" != "false" ]]; then
  echo "NORN_HOMEBREW_BOOTSTRAP must be true or false." >&2
  exit 1
fi

for required in CANDIDATE_FORMULA CANDIDATE_VERSION NORN_ARCHITECTURE; do
  if [[ -z "${!required:-}" ]]; then
    echo "Missing required lifecycle input: ${required}." >&2
    exit 1
  fi
done
if [[ "$bootstrap" = "false" ]]; then
  for required in PREVIOUS_FORMULA PREVIOUS_VERSION; do
    if [[ -z "${!required:-}" ]]; then
      echo "Missing required lifecycle input: ${required}." >&2
      exit 1
    fi
  done
fi

candidate_formula="$(cd "$(dirname "$CANDIDATE_FORMULA")" && pwd)/$(basename "$CANDIDATE_FORMULA")"
previous_formula=""
if [[ "$bootstrap" = "false" ]]; then
  previous_formula="$(cd "$(dirname "$PREVIOUS_FORMULA")" && pwd)/$(basename "$PREVIOUS_FORMULA")"
fi
brew_bin="$(brew --prefix)/bin"
settings_root="${NORN_LIFECYCLE_STATE_ROOT:-$HOME/Library/Application Support}"
legacy_settings="$settings_root/lachesi/settings.json"
canonical_settings="$settings_root/norn/settings.json"
lifecycle_marker="$settings_root/norn/lifecycle-marker"
diagnostics_root="${RUNNER_TEMP:-${TMPDIR:-/tmp}}"

verify_candidate() {
  local phase="$1"
  echo "phase=${phase} version=${CANDIDATE_VERSION} architecture=${NORN_ARCHITECTURE} artifact=norn.rb"
  test "$(command -v norn)" = "$brew_bin/norn"
  test "$(command -v norn-tui)" = "$brew_bin/norn-tui"
  norn --version | grep -F "norn $CANDIDATE_VERSION"
  norn --help >/dev/null
  norn-tui --version | grep -F "$CANDIDATE_VERSION"
  node scripts/run-with-timeout.mjs --timeout-ms 60000 -- \
    norn doctor --machine-only --format json > "$diagnostics_root/${phase}-doctor.json"
}

echo "phase=clean-install version=${CANDIDATE_VERSION} architecture=${NORN_ARCHITECTURE} artifact=norn.rb"
brew install --formula "$candidate_formula"
verify_candidate "clean-install"
brew test norn
if [[ "$bootstrap" = "true" ]]; then
  mkdir -p "$(dirname "$lifecycle_marker")"
  printf '%s\n' 'preserve-across-bootstrap-reinstall' > "$lifecycle_marker"
fi

echo "phase=clean-uninstall version=${CANDIDATE_VERSION} architecture=${NORN_ARCHITECTURE}"
brew uninstall --formula norn

if [[ "$bootstrap" = "true" ]]; then
  echo "phase=bootstrap-reinstall version=${CANDIDATE_VERSION} architecture=${NORN_ARCHITECTURE} artifact=norn.rb"
  brew install --formula "$candidate_formula"
  verify_candidate "bootstrap-reinstall"
  grep -Fx 'preserve-across-bootstrap-reinstall' "$lifecycle_marker"
  brew test norn
  exit 0
fi

echo "phase=previous-install version=${PREVIOUS_VERSION} architecture=${NORN_ARCHITECTURE} artifact=previous-norn.rb"
brew install --formula "$previous_formula"
norn --version | grep -F "norn $PREVIOUS_VERSION"

mkdir -p "$(dirname "$legacy_settings")" "$(dirname "$lifecycle_marker")"
printf '%s\n' '{"defaultDiffView":"split","theme":"light","repos":[]}' > "$legacy_settings"
printf '%s\n' 'preserve-across-upgrade' > "$lifecycle_marker"
test ! -e "$canonical_settings"

echo "phase=upgrade from=${PREVIOUS_VERSION} to=${CANDIDATE_VERSION} architecture=${NORN_ARCHITECTURE} artifact=norn.rb"
brew upgrade --formula "$candidate_formula"
verify_candidate "upgrade"
test -f "$canonical_settings"
cmp -s "$legacy_settings" "$canonical_settings"
grep -Fx 'preserve-across-upgrade' "$lifecycle_marker"

echo "phase=uninstall version=${CANDIDATE_VERSION} architecture=${NORN_ARCHITECTURE}"
brew uninstall --formula norn
test ! -e "$brew_bin/norn"
test ! -e "$brew_bin/norn-tui"

echo "phase=reinstall version=${CANDIDATE_VERSION} architecture=${NORN_ARCHITECTURE} artifact=norn.rb"
brew install --formula "$candidate_formula"
verify_candidate "reinstall"
grep -Fx 'preserve-across-upgrade' "$lifecycle_marker"
brew test norn
