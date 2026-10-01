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
  # This target uses only synthetic products and isolated files; enabling the
  # harness here does not execute real-product qualification targets.
  cargo test --locked -p radishlex-macos-product-install-coordinator \
    --features qualification-harness --test cancellation_archive
  cargo clippy --locked -p radishlex-macos-product-install-coordinator --all-targets -- -D warnings
  cargo clippy --locked -p radishlex-macos-product-install-coordinator \
    --features qualification-harness --test cancellation_archive -- -D warnings
)

echo "macOS product install coordinator gate passed."
