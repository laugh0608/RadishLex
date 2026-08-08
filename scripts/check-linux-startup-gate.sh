#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${repo_root}"

cargo fmt --all -- --check
cargo check -p radishlex-linux-product-install
cargo test -p radishlex-linux-product-install startup::
cargo clippy -p radishlex-linux-product-install --all-targets -- -D warnings
cargo test -p radishlex-ime-ffi linux_product_startup --lib
cargo test -p radishlex-ime-ffi --test input_header_contract
python3 scripts/linux-product/test_startup_gate_order.py

cxx="${CXX:-c++}"
temp_dir="$(mktemp -d "${TMPDIR:-/tmp}/radishlex-linux-startup.XXXXXX")"
cleanup() {
  rm -rf "${temp_dir}"
}
trap cleanup EXIT

for startup_identity in 1 2; do
  "${cxx}" -std=c++17 -Wall -Wextra -Wpedantic -Werror \
    -Iplatforms/linux-fcitx5/include \
    -Icrates/ime-ffi/include \
    "-DRADISHLEX_LINUX_STARTUP_BUILD_IDENTITY=${startup_identity}" \
    platforms/linux-fcitx5/tests/product_startup_test.cpp \
    platforms/linux-fcitx5/src/product_startup.cpp \
    -o "${temp_dir}/product_startup_${startup_identity}"
  "${temp_dir}/product_startup_${startup_identity}"
done

echo "Linux read-only startup gate passed"
