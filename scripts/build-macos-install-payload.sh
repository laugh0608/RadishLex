#!/usr/bin/env bash
set -euo pipefail

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
repo_root="$(CDPATH= cd -- "${script_dir}/.." && pwd)"
product_tool="${repo_root}/scripts/macos-product/product_manifest.py"
layout_tool="${repo_root}/scripts/macos-product/install_layout.py"

upgrade_source_args=()
has_upgrade_sources=false
while [[ $# -gt 0 ]]; do
  case "$1" in
    --upgrade-source-product-root)
      if [[ $# -lt 2 || -z "$2" ]]; then
        echo "--upgrade-source-product-root requires a path" >&2
        exit 2
      fi
      upgrade_source_args+=(--upgrade-source-product-root "$2")
      has_upgrade_sources=true
      shift 2
      ;;
    *)
      echo "unknown macOS install payload build argument: $1" >&2
      exit 2
      ;;
  esac
done
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

if [[ "${has_upgrade_sources}" == true ]]; then
  PYTHONDONTWRITEBYTECODE=1 python3 "${layout_tool}" assemble \
    --product-root "${product_root}" \
    --output "${payload_root}" \
    "${upgrade_source_args[@]}"
else
  PYTHONDONTWRITEBYTECODE=1 python3 "${layout_tool}" assemble \
    --product-root "${product_root}" \
    --output "${payload_root}"
fi
PYTHONDONTWRITEBYTECODE=1 python3 "${layout_tool}" verify \
  --payload-root "${payload_root}"

echo "RadishLex macOS install payload: ${payload_root}"
