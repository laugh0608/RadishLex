#!/usr/bin/env bash
set -euo pipefail

umask 022

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
repo_root="$(CDPATH= cd -- "${script_dir}/.." && pwd)"

if [[ $# -ne 0 ]]; then
  echo "Linux Debian artifact check does not accept arguments" >&2
  exit 2
fi

"${repo_root}/scripts/check-linux-product-metadata.sh"
PYTHONDONTWRITEBYTECODE=1 python3 \
  "${repo_root}/scripts/linux-product/test_deb_artifact.py"
bash -n "${repo_root}/scripts/build-linux-deb-artifact.sh"

echo "Linux deterministic Debian artifact contract passed."
