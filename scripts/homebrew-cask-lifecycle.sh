#!/usr/bin/env bash

set -euo pipefail

bootstrap="${NORN_HOMEBREW_BOOTSTRAP:-false}"
if [[ "$bootstrap" != "true" && "$bootstrap" != "false" ]]; then
  echo "NORN_HOMEBREW_BOOTSTRAP must be true or false." >&2
  exit 1
fi

for required in CANDIDATE_CASK CANDIDATE_VERSION NORN_ARCHITECTURE; do
  if [[ -z "${!required:-}" ]]; then
    echo "Missing required desktop lifecycle input: ${required}." >&2
    exit 1
  fi
done
if [[ "$bootstrap" = "false" ]]; then
  for required in PREVIOUS_CASK PREVIOUS_VERSION; do
    if [[ -z "${!required:-}" ]]; then
      echo "Missing required desktop lifecycle input: ${required}." >&2
      exit 1
    fi
  done
fi

candidate_cask="$(cd "$(dirname "$CANDIDATE_CASK")" && pwd)/$(basename "$CANDIDATE_CASK")"
previous_cask=""
if [[ "$bootstrap" = "false" ]]; then
  previous_cask="$(cd "$(dirname "$PREVIOUS_CASK")" && pwd)/$(basename "$PREVIOUS_CASK")"
fi
app_path="/Applications/Norn.app"
brew_bin="$(brew --prefix)/bin"
state_root="${NORN_LIFECYCLE_STATE_ROOT:-$HOME/Library/Application Support/norn}"
lifecycle_marker="$state_root/desktop-lifecycle-marker"
credential_account="norn-homebrew-${CANDIDATE_VERSION}-${NORN_ARCHITECTURE}"
credential_service="app.norn.desktop.lifecycle"
credential_value="preserve-credential-across-desktop-upgrade"
credential_created=0

cleanup_credential() {
  if [[ "$credential_created" = "1" ]]; then
    security delete-generic-password -a "$credential_account" -s "$credential_service" >/dev/null
  fi
}
trap cleanup_credential EXIT

create_preservation_evidence() {
  mkdir -p "$state_root"
  printf '%s\n' 'preserve-across-desktop-upgrade' > "$lifecycle_marker"
  security add-generic-password -U \
    -a "$credential_account" \
    -s "$credential_service" \
    -w "$credential_value"
  credential_created=1
}

verify_preservation_evidence() {
  grep -Fx 'preserve-across-desktop-upgrade' "$lifecycle_marker"
  test "$(security find-generic-password -w -a "$credential_account" -s "$credential_service")" = "$credential_value"
}

verify_candidate() {
  local phase="$1"
  echo "phase=${phase} version=${CANDIDATE_VERSION} architecture=${NORN_ARCHITECTURE} artifact=norn-cask.rb"
  test "$(command -v norn)" = "$brew_bin/norn"
  norn --version | grep -F "norn $CANDIDATE_VERSION"
  bash scripts/verify-macos-app.sh "$app_path"
}

echo "phase=desktop-clean-install version=${CANDIDATE_VERSION} architecture=${NORN_ARCHITECTURE} artifact=norn-cask.rb"
brew install --cask "$candidate_cask"
verify_candidate "desktop-clean-install"
if [[ "$bootstrap" = "true" ]]; then
  create_preservation_evidence
fi

echo "phase=desktop-clean-uninstall version=${CANDIDATE_VERSION} architecture=${NORN_ARCHITECTURE}"
brew uninstall --cask norn
test ! -e "$app_path"
test "$(command -v norn)" = "$brew_bin/norn"

if [[ "$bootstrap" = "true" ]]; then
  echo "phase=desktop-bootstrap-reinstall version=${CANDIDATE_VERSION} architecture=${NORN_ARCHITECTURE} artifact=norn-cask.rb"
  brew install --cask "$candidate_cask"
  verify_candidate "desktop-bootstrap-reinstall"
  verify_preservation_evidence
  exit 0
fi

echo "phase=desktop-previous-install version=${PREVIOUS_VERSION} architecture=${NORN_ARCHITECTURE} artifact=previous-norn-cask.rb"
brew install --cask "$previous_cask"
create_preservation_evidence

echo "phase=desktop-upgrade from=${PREVIOUS_VERSION} to=${CANDIDATE_VERSION} architecture=${NORN_ARCHITECTURE} artifact=norn-cask.rb"
brew upgrade --cask "$candidate_cask"
verify_candidate "desktop-upgrade"
verify_preservation_evidence

echo "phase=desktop-uninstall version=${CANDIDATE_VERSION} architecture=${NORN_ARCHITECTURE}"
brew uninstall --cask norn
test ! -e "$app_path"
test "$(command -v norn)" = "$brew_bin/norn"

echo "phase=desktop-reinstall version=${CANDIDATE_VERSION} architecture=${NORN_ARCHITECTURE} artifact=norn-cask.rb"
brew install --cask "$candidate_cask"
verify_candidate "desktop-reinstall"
verify_preservation_evidence
