#!/usr/bin/env bash
set -euo pipefail

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
platform_dir="$(CDPATH= cd -- "${script_dir}/.." && pwd)"
contract_root="$(mktemp -d "/private/tmp/radishlex-privacy-contract.XXXXXX")"
trap 'rm -rf "${contract_root}"' EXIT

tool="${contract_root}/privacy-mode-control"
production_tool="${contract_root}/privacy-mode-control-production"
store="${contract_root}/preferences-state"
state_parent="${contract_root}/state-parent"
state_dir="${state_parent}/r01b"
receipt="${state_dir}/privacy-mode-baseline"
sentinel="${contract_root}/unrelated-preference"
export CLANG_MODULE_CACHE_PATH="${contract_root}/clang-module-cache"
mkdir -p "${CLANG_MODULE_CACHE_PATH}" "${state_parent}"
chmod 0755 "${state_parent}"

state_dir_define="-DRLX_PRIVACY_STATE_DIR=\"${state_dir}\""
clang -fobjc-arc -fmodules -Wall -Wextra -Werror \
  -mmacosx-version-min=13.0 \
  -DRADISHLEX_PRIVACY_CONTRACT=1 \
  "${state_dir_define}" \
  "${platform_dir}/Tools/privacy_mode_control.m" \
  -framework Foundation \
  -o "${tool}"
clang -fobjc-arc -fmodules -Wall -Wextra -Werror \
  -mmacosx-version-min=13.0 \
  -DRADISHLEX_PRIVACY_CONTRACT=0 \
  "${state_dir_define}" \
  "${platform_dir}/Tools/privacy_mode_control.m" \
  -framework Foundation \
  -o "${production_tool}"
if strings "${production_tool}" | \
  rg -q 'RADISHLEX_PRIVACY_CONTRACT_(STORE|FAIL|VERIFY|FOREIGN|REPLACE)'; then
  echo "production privacy tool contains contract-only hooks" >&2
  exit 1
fi

run_tool() {
  RADISHLEX_PRIVACY_CONTRACT_STORE="${store}" "${tool}" "$@"
}

expect_failure() {
  if "$@" >"${contract_root}/unexpected-output" \
    2>"${contract_root}/expected-error"; then
    echo "privacy contract expected command failure" >&2
    exit 1
  fi
}

reset_fixture() {
  rm -rf "${state_dir}" "${contract_root}/state-target"
  rm -f "${store}" "${contract_root}/receipt-target" \
    "${contract_root}/receipt-hardlink"
  mkdir -p "${state_parent}"
  chmod 0755 "${state_parent}"
}

assert_receipt_identity() {
  test -f "${receipt}"
  test ! -L "${receipt}"
  test "$(stat -f '%u' "${receipt}")" = "$(id -u)"
  test "$(stat -f '%Lp' "${receipt}")" = "600"
  test "$(stat -f '%l' "${receipt}")" = "1"
  grep -Fqx 'radishlex-privacy-baseline-v2' "${receipt}"
  grep -Fqx "device=$(stat -f '%d' "${receipt}")" "${receipt}"
  grep -Fqx "inode=$(stat -f '%i' "${receipt}")" "${receipt}"
}

expect_failure run_tool
expect_failure run_tool --capture-baseline "${receipt}"
test "$(run_tool --status)" = "privacy_mode=absent"
printf 'false\n' >"${store}"
test "$(run_tool --status)" = "privacy_mode=false"
printf 'true\n' >"${store}"
test "$(run_tool --status)" = "privacy_mode=true"
printf 'string\n' >"${store}"
expect_failure run_tool --status

reset_fixture
test "$(run_tool --capture-baseline)" = "privacy_baseline=absent"
test "$(stat -f '%Lp' "${state_dir}")" = "700"
assert_receipt_identity
expect_failure run_tool --capture-baseline
printf 'sentinel\n' >"${sentinel}"
test "$(run_tool --authorized-enable)" = \
  $'privacy_mode=true\nprivacy_baseline=absent'
test -e "${receipt}"
test "$(tr -d '\n' <"${store}")" = "true"
test "$(run_tool --authorized-restore)" = \
  $'privacy_mode=absent\nprivacy_baseline=restored'
test ! -e "${store}"
test ! -e "${receipt}"
test "$(cat "${sentinel}")" = "sentinel"

reset_fixture
printf 'false\n' >"${store}"
run_tool --capture-baseline >/dev/null
grep -Fqx 'state=false' "${receipt}"
run_tool --authorized-enable >/dev/null
run_tool --authorized-restore >/dev/null
test "$(tr -d '\n' <"${store}")" = "false"
test ! -e "${receipt}"

reset_fixture
printf 'true\n' >"${store}"
expect_failure run_tool --capture-baseline
printf 'invalid\n' >"${store}"
expect_failure run_tool --capture-baseline

reset_fixture
mkdir -m 0700 "${contract_root}/state-target"
ln -s "${contract_root}/state-target" "${state_dir}"
expect_failure run_tool --capture-baseline
test ! -e "${contract_root}/state-target/privacy-mode-baseline"

reset_fixture
chmod 0777 "${state_parent}"
expect_failure run_tool --capture-baseline
chmod 0755 "${state_parent}"

reset_fixture
mkdir -m 0777 "${state_dir}"
expect_failure run_tool --capture-baseline

reset_fixture
mkdir -m 0755 "${state_dir}"
run_tool --capture-baseline >/dev/null
test "$(stat -f '%Lp' "${state_dir}")" = "700"

reset_fixture
expect_failure env \
  RADISHLEX_PRIVACY_CONTRACT_STORE="${store}" \
  RADISHLEX_PRIVACY_CONTRACT_FAIL_PARENT_FSYNC=1 \
  "${tool}" --capture-baseline
test -d "${state_dir}"
test ! -e "${receipt}"

reset_fixture
mkdir -m 0700 "${state_dir}"
expect_failure env \
  RADISHLEX_PRIVACY_CONTRACT_STORE="${store}" \
  RADISHLEX_PRIVACY_CONTRACT_FOREIGN_STATE_OWNER=1 \
  "${tool}" --capture-baseline

reset_fixture
printf 'false\n' >"${store}"
run_tool --capture-baseline >/dev/null
chmod 0644 "${receipt}"
expect_failure run_tool --authorized-enable

reset_fixture
printf 'false\n' >"${store}"
run_tool --capture-baseline >/dev/null
ln "${receipt}" "${contract_root}/receipt-hardlink"
expect_failure run_tool --authorized-enable

reset_fixture
printf 'false\n' >"${store}"
run_tool --capture-baseline >/dev/null
printf 'malformed\n' >"${receipt}"
chmod 0600 "${receipt}"
expect_failure run_tool --authorized-enable

reset_fixture
printf 'false\n' >"${store}"
run_tool --capture-baseline >/dev/null
rm "${receipt}"
printf 'target\n' >"${contract_root}/receipt-target"
chmod 0600 "${contract_root}/receipt-target"
ln -s "${contract_root}/receipt-target" "${receipt}"
expect_failure run_tool --authorized-enable

reset_fixture
printf 'false\n' >"${store}"
run_tool --capture-baseline >/dev/null
printf 'true\n' >"${store}"
expect_failure run_tool --authorized-enable

reset_fixture
printf 'false\n' >"${store}"
run_tool --capture-baseline >/dev/null
expect_failure env \
  RADISHLEX_PRIVACY_CONTRACT_STORE="${store}" \
  RADISHLEX_PRIVACY_CONTRACT_FAIL_WRITE=1 \
  "${tool}" --authorized-enable
test "$(tr -d '\n' <"${store}")" = "false"

reset_fixture
printf 'false\n' >"${store}"
run_tool --capture-baseline >/dev/null
expect_failure env \
  RADISHLEX_PRIVACY_CONTRACT_STORE="${store}" \
  RADISHLEX_PRIVACY_CONTRACT_REPLACE_BEFORE_WRITE=1 \
  "${tool}" --authorized-enable
test "$(tr -d '\n' <"${store}")" = "false"

reset_fixture
printf 'false\n' >"${store}"
run_tool --capture-baseline >/dev/null
run_tool --authorized-enable >/dev/null
expect_failure env \
  RADISHLEX_PRIVACY_CONTRACT_STORE="${store}" \
  RADISHLEX_PRIVACY_CONTRACT_REPLACE_BEFORE_UNLINK=1 \
  "${tool}" --authorized-restore
test "$(tr -d '\n' <"${store}")" = "false"
test -e "${receipt}"

reset_fixture
printf 'false\n' >"${store}"
run_tool --capture-baseline >/dev/null
expect_failure env \
  RADISHLEX_PRIVACY_CONTRACT_STORE="${store}" \
  RADISHLEX_PRIVACY_CONTRACT_VERIFY_STATE=false \
  "${tool}" --authorized-enable
test -e "${receipt}"

echo "privacy mode contract passed"
