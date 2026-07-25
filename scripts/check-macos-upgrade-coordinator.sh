#!/usr/bin/env bash
set -euo pipefail

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
repo_root="$(CDPATH= cd -- "${script_dir}/.." && pwd)"

if [[ $# -ne 0 ]]; then
  echo "macOS upgrade coordinator check does not accept arguments" >&2
  exit 2
fi
if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "macOS is required for the upgrade coordinator check" >&2
  exit 2
fi

(
  cd "${repo_root}"
  cargo test --locked -p radishlex-macos-upgrade-coordinator
  cargo test --locked -p radishlex-macos-upgrade-coordinator \
    --features qualification-harness --no-run
  cargo clippy --locked -p radishlex-macos-upgrade-coordinator --all-targets -- -D warnings
  cargo clippy --locked -p radishlex-macos-upgrade-coordinator \
    --features qualification-harness --all-targets -- -D warnings
)
test "$(cat "${repo_root}/platforms/macos-product/UpgradeCoordinatorAdapter/fixtures/qualification.marker")" = \
  "radishlex-upgrade-qualification-v1"
bash -n "${repo_root}/scripts/check-macos-upgrade-product-coordination.sh"
"${repo_root}/platforms/macos-product/UpgradePreflightHost/check.sh"
"${repo_root}/platforms/macos-product/UpgradeValidationHosts/check.sh"

echo "macOS upgrade coordinator adapter gate passed."
