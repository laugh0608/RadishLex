#!/usr/bin/env bash
set -euo pipefail

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
repo_root="$(CDPATH= cd -- "${script_dir}/.." && pwd)"

if [[ $# -ne 0 ]]; then
  echo "macOS install layout check does not accept arguments" >&2
  exit 2
fi

PYTHONDONTWRITEBYTECODE=1 python3 \
  "${repo_root}/scripts/macos-product/install_layout.py" validate-source
PYTHONDONTWRITEBYTECODE=1 python3 \
  "${repo_root}/scripts/macos-product/test_install_layout.py"
bash -n "${repo_root}/scripts/build-macos-install-payload.sh"

echo "macOS install layout contract passed."
