#!/usr/bin/env bash
set -euo pipefail

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
platform_dir="$(CDPATH= cd -- "${script_dir}/.." && pwd)"
contract_root="$(mktemp -d "${TMPDIR:-/tmp}/radishlex-m2-manager-data-helper.XXXXXX")"
trap 'rm -rf "${contract_root}"' EXIT

contract_home="${contract_root}/home"
parent="${contract_home}/Library/Application Support/RadishLex"
state_dir="${contract_root}/state"
receipt="${state_dir}/m2-manager-test-data-baseline.receipt"
helper="${contract_root}/m2-manager-test-data-cleanup"
production_home_guard_helper="${contract_root}/m2-manager-home-guard-cleanup"
source="${platform_dir}/Tools/test_data_cleanup.c"

mkdir -p "${state_dir}"
chmod 0700 "${state_dir}"
clang -std=c11 -Wall -Wextra -Werror \
  -mmacosx-version-min=13.0 \
  -DRLX_TEST_DATA_CONTRACT=1 \
  -DRLX_TEST_DATA_PROFILE_MANAGER=1 \
  "-DRLX_TEST_DATA_STATE_DIR=\"${state_dir}\"" \
  "${source}" \
  -o "${helper}"
clang -std=c11 -Wall -Wextra -Werror \
  -mmacosx-version-min=13.0 \
  -DRLX_TEST_DATA_PROFILE_MANAGER=1 \
  "-DRLX_TEST_DATA_STATE_DIR=\"${state_dir}\"" \
  "${source}" \
  -o "${production_home_guard_helper}"

run_helper() {
  HOME="${contract_home}" "${helper}" "$@"
}

run_helper_with_fault() {
  local phase="${1}"
  env HOME="${contract_home}" \
    RADISHLEX_TEST_DATA_CONTRACT_FAIL_PHASE="${phase}" \
    "${helper}" --authorized-delete-m2-manager-test-data
}

expect_failure() {
  if "$@" >"${contract_root}/unexpected-output" \
    2>"${contract_root}/expected-error"; then
    echo "M2 manager test-data helper contract expected failure" >&2
    exit 1
  fi
}

reset_fixture() {
  rm -rf "${contract_home}" "${state_dir}"
  mkdir -p "${parent}" "${state_dir}"
  chmod 0755 "${contract_home}" \
    "${contract_home}/Library" \
    "${contract_home}/Library/Application Support" \
    "${parent}"
  chmod 0700 "${state_dir}"
}

capture_baseline() {
  test "$(run_helper --capture-baseline)" = \
    "m2_manager_test_data_baseline=captured"
}

rewrite_receipt_devices() {
  local parent_device="${1}"
  local receipt_device="${2}"
  printf '%s\n' \
    'radishlex-m2-manager-test-data-baseline-v1' \
    "parent_dev=${parent_device}" \
    "parent_ino=$(stat -f '%i' "${parent}")" \
    "parent_uid=$(id -u)" \
    'parent_mode=0755' \
    "receipt_dev=${receipt_device}" \
    "receipt_ino=$(stat -f '%i' "${receipt}")" >"${receipt}"
  chmod 0600 "${receipt}"
}

prepare_test_data() {
  chmod 0700 "${parent}"
  for name in userdb.sqlite3 userdb.sqlite3-wal userdb.sqlite3-shm \
    userdb.sqlite3-journal manager-settings.json manager-settings.json.tmp; do
    printf 'synthetic M2 manager data\n' >"${parent}/${name}"
    chmod 0600 "${parent}/${name}"
  done
}

assert_recovery_completed() {
  test -d "${parent}"
  test "$(stat -f '%Lp' "${parent}")" = "755"
  test -z "$(find "${parent}" -mindepth 1 -maxdepth 1 -print -quit)"
  test ! -e "${receipt}"
}

production_strings="$(strings "${production_home_guard_helper}")"
for contract_marker in RADISHLEX_TEST_DATA_CONTRACT_FAIL_PHASE \
  RLXContractFailureIs unlink-sidecar unlink-main unlink-settings \
  receipt-unlink receipt-fsync 'contract injected'; do
  if [[ "${production_strings}" == *"${contract_marker}"* ]]; then
    echo "production M2 manager helper contains contract-only fault code: ${contract_marker}" >&2
    exit 1
  fi
done

reset_fixture
expect_failure env HOME="${contract_home}" \
  "${production_home_guard_helper}" --capture-baseline

reset_fixture
capture_baseline
test -f "${receipt}"
test ! -L "${receipt}"
test "$(stat -f '%u' "${receipt}")" = "$(id -u)"
test "$(stat -f '%Lp' "${receipt}")" = "600"
grep -Fqx 'radishlex-m2-manager-test-data-baseline-v1' "${receipt}"
grep -Fqx "parent_dev=$(stat -f '%d' "${parent}")" "${receipt}"
grep -Fqx "parent_ino=$(stat -f '%i' "${parent}")" "${receipt}"
grep -Fqx 'parent_mode=0755' "${receipt}"
expect_failure run_helper --capture-baseline

reset_fixture
capture_baseline
prepare_test_data
delete_output="$(run_helper --authorized-delete-m2-manager-test-data)"
test "${delete_output}" = \
  $'m2_manager_test_data=deleted\napplication_support_parent=empty\napplication_support_parent_mode=0755\nm2_manager_test_data_baseline=removed'
assert_recovery_completed

reset_fixture
capture_baseline
chmod 0700 "${parent}"
printf 'synthetic settings\n' >"${parent}/manager-settings.json"
chmod 0600 "${parent}/manager-settings.json"
run_helper --authorized-delete-m2-manager-test-data >/dev/null
assert_recovery_completed

reset_fixture
capture_baseline
prepare_test_data
drifted_device="$(($(stat -f '%d' "${parent}") + 1))"
rewrite_receipt_devices "${drifted_device}" "${drifted_device}"
run_helper --authorized-delete-m2-manager-test-data >/dev/null
assert_recovery_completed

reset_fixture
capture_baseline
prepare_test_data
current_device="$(stat -f '%d' "${parent}")"
drifted_device="$((current_device + 1))"
rewrite_receipt_devices "${drifted_device}" "${current_device}"
expect_failure run_helper --authorized-delete-m2-manager-test-data
test -e "${parent}/userdb.sqlite3"
test -e "${receipt}"

reset_fixture
capture_baseline
prepare_test_data
current_device="$(stat -f '%d' "${parent}")"
drifted_device="$((current_device + 1))"
rewrite_receipt_devices "${current_device}" "${drifted_device}"
expect_failure run_helper --authorized-delete-m2-manager-test-data
test -e "${parent}/userdb.sqlite3"
test -e "${receipt}"

reset_fixture
capture_baseline
prepare_test_data
expect_failure run_helper_with_fault unlink-settings
test -e "${parent}/userdb.sqlite3"
test -e "${parent}/manager-settings.json"
test -e "${receipt}"
run_helper --authorized-delete-m2-manager-test-data >/dev/null
assert_recovery_completed

reset_fixture
capture_baseline
chmod 0700 "${parent}"
printf 'synthetic sidecar\n' >"${parent}/userdb.sqlite3-wal"
chmod 0600 "${parent}/userdb.sqlite3-wal"
expect_failure run_helper --authorized-delete-m2-manager-test-data
test -e "${parent}/userdb.sqlite3-wal"
test -e "${receipt}"

reset_fixture
capture_baseline
prepare_test_data
printf 'unknown\n' >"${parent}/unknown-entry"
chmod 0600 "${parent}/unknown-entry"
expect_failure run_helper --authorized-delete-m2-manager-test-data
test -e "${parent}/userdb.sqlite3"
test -e "${receipt}"

reset_fixture
capture_baseline
prepare_test_data
rm "${parent}/manager-settings.json.tmp"
ln -s "${parent}/manager-settings.json" \
  "${parent}/manager-settings.json.tmp"
expect_failure run_helper --authorized-delete-m2-manager-test-data
test -e "${parent}/userdb.sqlite3"
test -e "${receipt}"

reset_fixture
capture_baseline
prepare_test_data
printf 'tampered\n' >"${receipt}"
chmod 0600 "${receipt}"
expect_failure run_helper --authorized-delete-m2-manager-test-data
test -e "${parent}/userdb.sqlite3"

rg -Fq '"manager-settings.json"' "${source}"
rg -Fq '"manager-settings.json.tmp"' "${source}"
rg -Fq 'unlinkat(parent_fd, kTestDataNames[index], 0)' "${source}"
rg -Fq 'AT_SYMLINK_NOFOLLOW' "${source}"
if rg -n '(^|[^A-Za-z])(remove|rename|system|popen)[[:space:]]*\(' "${source}"; then
  echo "M2 manager test-data helper contains a general path or process primitive" >&2
  exit 1
fi

echo "M2 manager test-data cleanup helper contract passed"
