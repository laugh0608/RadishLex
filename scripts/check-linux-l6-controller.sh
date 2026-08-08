#!/usr/bin/env bash
set -euo pipefail

repo_root="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
cd "$repo_root"

if [[ $# -ne 0 ]]; then
  echo "Linux L6 controller check does not accept arguments" >&2
  exit 2
fi

cargo fmt --all -- --check
cargo check -p radishlex-linux-product-install --features l6-acceptance-checkpoints
cargo test -p radishlex-linux-product-install --features l6-acceptance-checkpoints
cargo clippy -p radishlex-linux-product-install --all-targets \
  --features l6-acceptance-checkpoints -- -D warnings
cargo check -p radishlex-linux-l6-acceptance
cargo test -p radishlex-linux-l6-acceptance
cargo clippy -p radishlex-linux-l6-acceptance --all-targets -- -D warnings
PYTHONDONTWRITEBYTECODE=1 python3 \
  "$repo_root/scripts/linux-product/l6_controller_contract.py" validate
PYTHONDONTWRITEBYTECODE=1 python3 \
  "$repo_root/scripts/linux-product/test_l6_controller_contract.py"

echo "Linux L6 compile-isolated checkpoint and evidence controller gate passed."
