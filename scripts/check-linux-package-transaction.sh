#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

cargo fmt --all -- --check
cargo check -p radishlex-linux-product-install
cargo test -p radishlex-linux-product-install
cargo clippy -p radishlex-linux-product-install --all-targets -- -D warnings

echo "Linux package transaction gate passed"
