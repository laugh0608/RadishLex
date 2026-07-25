#!/usr/bin/env bash
set -euo pipefail

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
repo_root="$(CDPATH= cd -- "${script_dir}/.." && pwd)"
product_tool="${repo_root}/scripts/macos-product/product_manifest.py"
layout_tool="${repo_root}/scripts/macos-product/install_layout.py"

if [[ $# -ne 0 ]]; then
  echo "macOS install payload build does not accept arguments" >&2
  exit 2
fi
if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "RadishLex macOS install payload assembly requires macOS." >&2
  exit 1
fi

product_version="$(python3 "${product_tool}" field product_version)"
product_build="$(python3 "${product_tool}" field build_number)"
product_root="${repo_root}/target/macos-product/${product_version}-${product_build}"
payload_root="${repo_root}/target/macos-install-payload/${product_version}-${product_build}"

if [[ ! -d "${product_root}" || -L "${product_root}" ]]; then
  echo "verified macOS product assembly is required: ${product_root}" >&2
  exit 1
fi

PYTHONDONTWRITEBYTECODE=1 python3 "${layout_tool}" assemble \
  --product-root "${product_root}" \
  --output "${payload_root}"
PYTHONDONTWRITEBYTECODE=1 python3 "${layout_tool}" verify \
  --payload-root "${payload_root}"

echo "RadishLex macOS install payload: ${payload_root}"
