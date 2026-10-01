#!/usr/bin/env bash
set -euo pipefail

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
repo_root="$(CDPATH= cd -- "${script_dir}/.." && pwd)"

if [[ $# -ne 0 ]]; then
  echo "Linux L6 release pair check does not accept arguments" >&2
  exit 2
fi

PYTHONDONTWRITEBYTECODE=1 python3 \
  "${repo_root}/scripts/linux-product/l6_release_pair.py" validate-frozen-contract
PYTHONDONTWRITEBYTECODE=1 python3 \
  "${repo_root}/scripts/linux-product/test_l6_release_pair.py"
PYTHONDONTWRITEBYTECODE=1 python3 \
  "${repo_root}/scripts/linux-product/l6_release_pair_contract.py" validate
PYTHONDONTWRITEBYTECODE=1 python3 \
  "${repo_root}/scripts/linux-product/test_l6_release_pair_contract.py"
bash -n "${repo_root}/scripts/build-linux-l6-release-pair.sh"

echo "Linux L6 release pair contract passed without package, guest, or system mutation."
