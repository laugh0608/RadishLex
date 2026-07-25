#!/usr/bin/env bash
set -euo pipefail

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
repo_root="$(CDPATH= cd -- "${script_dir}/.." && pwd)"

if [[ $# -ne 0 ]]; then
  echo "macOS install coordinator check does not accept arguments" >&2
  exit 2
fi
if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "macOS is required for the product install coordinator check" >&2
  exit 2
fi

(
  cd "${repo_root}"
  cargo test --locked -p radishlex-macos-product-install-coordinator --all-targets
  cargo clippy --locked -p radishlex-macos-product-install-coordinator --all-targets -- -D warnings
)

echo "macOS product install coordinator gate passed."
