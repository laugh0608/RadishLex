#!/usr/bin/env bash
set -euo pipefail

if [ "$(uname -s)" != "Darwin" ]; then
  echo "upgrade validation host contract requires macOS." >&2
  exit 1
fi

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
repo_root="$(CDPATH= cd -- "${script_dir}/../../.." && pwd)"
smoke_dir="$(mktemp -d "${TMPDIR:-/tmp}/radishlex-upgrade-validation-host.XXXXXX")"

cleanup() {
  rm -rf "${smoke_dir}"
}
trap cleanup EXIT

clang -fobjc-arc -fmodules -Wall -Wextra -Werror \
  -mmacosx-version-min=13.0 \
  -I"${script_dir}/Sources" \
  -I"${repo_root}/crates/ime-ffi/include" \
  "${script_dir}/Sources/RLXUpgradeValidationSupport.m" \
  "${script_dir}/Tests/support_contract.m" \
  -framework Foundation \
  -o "${smoke_dir}/support-contract"

"${smoke_dir}/support-contract"
