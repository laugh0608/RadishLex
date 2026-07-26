#!/usr/bin/env bash
set -euo pipefail

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
repo_root="$(CDPATH= cd -- "${script_dir}/.." && pwd)"

python3 "${repo_root}/scripts/macos-product/product_manifest.py" validate-source
python3 "${repo_root}/scripts/rime-product/product_data.py" validate
PYTHONDONTWRITEBYTECODE=1 python3 \
  "${repo_root}/scripts/macos-product/test_product_manifest.py"
PYTHONDONTWRITEBYTECODE=1 python3 \
  "${repo_root}/scripts/rime-product/test_product_data.py"
"${repo_root}/scripts/check-macos-install-layout.sh"

echo "macOS product metadata contract passed."
