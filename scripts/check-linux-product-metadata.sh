#!/usr/bin/env bash
set -euo pipefail

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
repo_root="$(CDPATH= cd -- "${script_dir}/.." && pwd)"

if [[ $# -ne 0 ]]; then
  echo "Linux product metadata check does not accept arguments" >&2
  exit 2
fi

temp_dir="$(mktemp -d "${TMPDIR:-/tmp}/radishlex-linux-metadata.XXXXXX")"
cleanup() {
  rm -rf "${temp_dir}"
}
trap cleanup EXIT

PYTHONDONTWRITEBYTECODE=1 python3 \
  "${repo_root}/scripts/linux-product/product_metadata.py" validate-source
PYTHONDONTWRITEBYTECODE=1 python3 \
  "${repo_root}/scripts/rime-product/product_data.py" validate
PYTHONDONTWRITEBYTECODE=1 python3 \
  "${repo_root}/scripts/rime-product/test_product_data.py"
PYTHONDONTWRITEBYTECODE=1 python3 \
  "${repo_root}/scripts/linux-product/test_product_metadata.py"

PYTHONDONTWRITEBYTECODE=1 python3 \
  "${repo_root}/scripts/linux-product/product_metadata.py" \
  render-control --output "${temp_dir}/control.first"
PYTHONDONTWRITEBYTECODE=1 python3 \
  "${repo_root}/scripts/linux-product/product_metadata.py" \
  render-control --output "${temp_dir}/control.second"
cmp "${temp_dir}/control.first" "${temp_dir}/control.second"
PYTHONDONTWRITEBYTECODE=1 python3 \
  "${repo_root}/scripts/linux-product/product_metadata.py" \
  render-shlibdeps-control --output "${temp_dir}/shlibdeps-control.first"
PYTHONDONTWRITEBYTECODE=1 python3 \
  "${repo_root}/scripts/linux-product/product_metadata.py" \
  render-shlibdeps-control --output "${temp_dir}/shlibdeps-control.second"
cmp "${temp_dir}/shlibdeps-control.first" "${temp_dir}/shlibdeps-control.second"

echo "Linux product metadata contract passed."
