#!/usr/bin/env bash
set -euo pipefail

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
repo_root="$(CDPATH= cd -- "${script_dir}/.." && pwd)"

if [[ $# -ne 0 ]]; then
  echo "macOS Installer check does not accept arguments" >&2
  exit 2
fi
if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "macOS is required for the Installer check" >&2
  exit 2
fi

PYTHONDONTWRITEBYTECODE=1 python3 \
  "${repo_root}/scripts/macos-product/test_release_identity.py"
(
  cd "${repo_root}"
  cargo test --locked -p radishlex-macos-installer-driver --all-targets
  cargo clippy --locked -p radishlex-macos-installer-driver --all-targets -- -D warnings
  cargo test --locked -p radishlex-macos-installer-executor --all-targets
  cargo clippy --locked -p radishlex-macos-installer-executor --all-targets -- -D warnings
  cargo test --locked -p radishlex-macos-installer-bridge --all-targets
  cargo clippy --locked -p radishlex-macos-installer-bridge --all-targets -- -D warnings
)
"${repo_root}/platforms/macos-product/InstallerApp/check.sh"

echo "macOS Installer UI/bridge/driver/executor gate passed."
