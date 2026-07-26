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
PYTHONDONTWRITEBYTECODE=1 python3 \
  "${repo_root}/scripts/macos-product/test_community_release.py"
bash -n "${repo_root}/scripts/build-macos-release-dmg.sh"
bash -n "${repo_root}/scripts/notarize-macos-release-dmg.sh"

if [[ "$(uname -s)" == "Darwin" ]]; then
  set +e
  env RADISHLEX_NOTARY_KEYCHAIN_PROFILE= \
    "${repo_root}/scripts/notarize-macos-release-dmg.sh" >/dev/null 2>&1
  community_notary_status=$?
  set -e
  if [[ ${community_notary_status} -ne 2 ]]; then
    echo "community release notarization must fail closed" >&2
    exit 1
  fi
fi

echo "macOS release carrier contract gate passed."
