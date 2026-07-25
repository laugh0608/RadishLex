#!/usr/bin/env bash
set -euo pipefail

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
repo_root="$(CDPATH= cd -- "${script_dir}/.." && pwd)"

if [[ $# -ne 0 ]]; then
  echo "macOS release carrier check does not accept arguments" >&2
  exit 2
fi

PYTHONDONTWRITEBYTECODE=1 python3 \
  "${repo_root}/scripts/macos-product/test_release_carrier.py"
bash -n "${repo_root}/scripts/build-macos-release-dmg.sh"
bash -n "${repo_root}/scripts/notarize-macos-release-dmg.sh"

if [[ "$(uname -s)" == "Darwin" ]]; then
  set +e
  env RADISHLEX_DEVELOPER_ID_APPLICATION=- \
    "${repo_root}/scripts/build-macos-release-dmg.sh" >/dev/null 2>&1
  missing_identity_status=$?
  env RADISHLEX_NOTARY_KEYCHAIN_PROFILE= \
    "${repo_root}/scripts/notarize-macos-release-dmg.sh" >/dev/null 2>&1
  missing_profile_status=$?
  set -e
  if [[ ${missing_identity_status} -ne 2 ]]; then
    echo "release DMG build must reject a missing Developer ID identity" >&2
    exit 1
  fi
  if [[ ${missing_profile_status} -ne 2 ]]; then
    echo "release notarization must reject a missing Keychain profile" >&2
    exit 1
  fi
fi

echo "macOS release carrier contract gate passed."
