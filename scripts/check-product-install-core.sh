#!/usr/bin/env bash
set -euo pipefail

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
repo_root="$(CDPATH= cd -- "${script_dir}/.." && pwd)"

if [[ $# -ne 0 ]]; then
  echo "product install core check does not accept arguments" >&2
  exit 2
fi

(
  cd "${repo_root}"
  cargo test --locked -p radishlex-ime-product-install --all-targets
  cargo clippy --locked -p radishlex-ime-product-install --all-targets -- -D warnings
)

echo "Product install transaction core passed."
