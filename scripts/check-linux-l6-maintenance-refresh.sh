#!/usr/bin/env bash
set -euo pipefail

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
repo_root="$(CDPATH= cd -- "${script_dir}/.." && pwd)"

if [[ $# -ne 0 ]]; then
  echo "Linux L6 maintenance refresh check does not accept arguments" >&2
  exit 2
fi

PYTHONDONTWRITEBYTECODE=1 python3 \
  "${repo_root}/scripts/linux-product/l6_maintenance_refresh.py" validate-contract
PYTHONDONTWRITEBYTECODE=1 python3 \
  "${repo_root}/scripts/linux-product/test_l6_maintenance_refresh.py"
PYTHONDONTWRITEBYTECODE=1 python3 \
  "${repo_root}/scripts/linux-product/l6_maintenance_refresh_contract.py" validate
PYTHONDONTWRITEBYTECODE=1 python3 \
  "${repo_root}/scripts/linux-product/test_l6_maintenance_refresh_contract.py"
bash -n "${repo_root}/scripts/build-linux-l6-maintenance-refresh.sh"

echo "Linux L6 maintenance refresh contract passed without package rebuild, guest, or system mutation."
