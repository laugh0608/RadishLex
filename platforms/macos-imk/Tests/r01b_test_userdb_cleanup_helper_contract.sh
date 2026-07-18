#!/usr/bin/env bash
set -euo pipefail

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
platform_dir="$(CDPATH= cd -- "${script_dir}/.." && pwd)"
contract_root="$(mktemp -d "${TMPDIR:-/tmp}/radishlex-r01b-userdb-helper.XXXXXX")"
trap 'rm -rf "${contract_root}"' EXIT

contract_home="${contract_root}/home"
parent="${contract_home}/Library/Application Support/RadishLex"
state_dir="${contract_root}/state"
receipt="${state_dir}/r01b-test-userdb-baseline.receipt"
helper="${contract_root}/r01b-test-userdb-cleanup"
production_home_guard_helper="${contract_root}/r01b-home-guard-cleanup"
source="${platform_dir}/Tools/test_data_cleanup.c"

mkdir -p "${state_dir}"
chmod 0700 "${state_dir}"
clang -std=c11 -Wall -Wextra -Werror \
  -mmacosx-version-min=13.0 \
  -DRLX_TEST_DATA_CONTRACT=1 \
  "-DRLX_TEST_DATA_STATE_DIR=\"${state_dir}\"" \
  "${source}" \
  -o "${helper}"
clang -std=c11 -Wall -Wextra -Werror \
  -mmacosx-version-min=13.0 \
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
    "${helper}" --authorized-delete-r01b-test-userdb
}

expect_failure() {
  if "$@" >"${contract_root}/unexpected-output" \
    2>"${contract_root}/expected-error"; then
    echo "R01B userdb helper contract expected failure" >&2
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
    "r01b_userdb_baseline=captured"
}

prepare_userdb_family() {
  chmod 0700 "${parent}"
  for name in userdb.sqlite3 userdb.sqlite3-wal userdb.sqlite3-shm \
    userdb.sqlite3-journal; do
    printf 'synthetic R01B data\n' >"${parent}/${name}"
    chmod 0600 "${parent}/${name}"
  done
}

assert_failure_preserved_state() {
  test -e "${parent}/userdb.sqlite3"
  test -e "${receipt}"
  test "$(stat -f '%Lp' "${parent}")" = "700"
}

assert_recovery_completed() {
  test -d "${parent}"
  test "$(stat -f '%Lp' "${parent}")" = "755"
  test -z "$(find "${parent}" -mindepth 1 -maxdepth 1 -print -quit)"
  test ! -e "${receipt}"
}

production_strings="$(strings "${production_home_guard_helper}")"
for contract_marker in RADISHLEX_TEST_DATA_CONTRACT_FAIL_PHASE \
  RLXContractFailureIs unlink-sidecar unlink-main receipt-unlink \
  receipt-fsync 'contract injected'; do
  if [[ "${production_strings}" == *"${contract_marker}"* ]]; then
    echo "production R01B helper contains contract-only fault code: ${contract_marker}" >&2
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
grep -Fqx 'radishlex-r01b-test-userdb-baseline-v1' "${receipt}"
grep -Fqx "parent_dev=$(stat -f '%d' "${parent}")" "${receipt}"
grep -Fqx "parent_ino=$(stat -f '%i' "${parent}")" "${receipt}"
grep -Fqx "parent_uid=$(id -u)" "${receipt}"
grep -Fqx 'parent_mode=0755' "${receipt}"
grep -Fqx "receipt_dev=$(stat -f '%d' "${receipt}")" "${receipt}"
grep -Fqx "receipt_ino=$(stat -f '%i' "${receipt}")" "${receipt}"
expect_failure run_helper --capture-baseline

reset_fixture
printf 'unrelated\n' >"${parent}/unrelated"
expect_failure run_helper --capture-baseline
test ! -e "${receipt}"

reset_fixture
chmod 0700 "${parent}"
expect_failure run_helper --capture-baseline
test ! -e "${receipt}"

reset_fixture
rmdir "${parent}"
mkdir "${contract_root}/external-parent"
chmod 0755 "${contract_root}/external-parent"
ln -s "${contract_root}/external-parent" "${parent}"
expect_failure run_helper --capture-baseline
test ! -e "${receipt}"

reset_fixture
capture_baseline
prepare_userdb_family
delete_output="$(run_helper --authorized-delete-r01b-test-userdb)"
test "${delete_output}" = \
  $'r01b_test_userdb=deleted\napplication_support_parent=empty\napplication_support_parent_mode=0755\nr01b_userdb_baseline=removed'
test -d "${parent}"
assert_recovery_completed

reset_fixture
capture_baseline
chmod 0700 "${parent}"
printf 'main only\n' >"${parent}/userdb.sqlite3"
chmod 0600 "${parent}/userdb.sqlite3"
run_helper --authorized-delete-r01b-test-userdb >/dev/null
assert_recovery_completed

reset_fixture
capture_baseline
prepare_userdb_family
expect_failure run_helper_with_fault unlink-sidecar
test ! -e "${parent}/userdb.sqlite3-wal"
test -e "${parent}/userdb.sqlite3-shm"
test -e "${parent}/userdb.sqlite3-journal"
assert_failure_preserved_state
run_helper --authorized-delete-r01b-test-userdb >/dev/null
assert_recovery_completed

reset_fixture
capture_baseline
prepare_userdb_family
expect_failure run_helper_with_fault unlink-main
test -e "${parent}/userdb.sqlite3"
test ! -e "${parent}/userdb.sqlite3-wal"
test ! -e "${parent}/userdb.sqlite3-shm"
test ! -e "${parent}/userdb.sqlite3-journal"
assert_failure_preserved_state
run_helper --authorized-delete-r01b-test-userdb >/dev/null
assert_recovery_completed

reset_fixture
capture_baseline
prepare_userdb_family
expect_failure run_helper_with_fault fchmod
test "$(stat -f '%Lp' "${parent}")" = "700"
test -z "$(find "${parent}" -mindepth 1 -maxdepth 1 -print -quit)"
test -e "${receipt}"
run_helper --authorized-delete-r01b-test-userdb >/dev/null
assert_recovery_completed

reset_fixture
capture_baseline
prepare_userdb_family
expect_failure run_helper_with_fault receipt-unlink
test "$(stat -f '%Lp' "${parent}")" = "755"
test -z "$(find "${parent}" -mindepth 1 -maxdepth 1 -print -quit)"
test -e "${receipt}"
run_helper --authorized-delete-r01b-test-userdb >/dev/null
assert_recovery_completed

reset_fixture
capture_baseline
prepare_userdb_family
expect_failure run_helper_with_fault receipt-fsync
grep -Fq 'retry receipt was restored' "${contract_root}/expected-error"
test "$(stat -f '%Lp' "${parent}")" = "755"
test -z "$(find "${parent}" -mindepth 1 -maxdepth 1 -print -quit)"
test -e "${receipt}"
grep -Fqx "receipt_ino=$(stat -f '%i' "${receipt}")" "${receipt}"
run_helper --authorized-delete-r01b-test-userdb >/dev/null
assert_recovery_completed

reset_fixture
capture_baseline
prepare_userdb_family
printf 'unknown\n' >"${parent}/unknown-entry"
chmod 0600 "${parent}/unknown-entry"
expect_failure run_helper --authorized-delete-r01b-test-userdb
assert_failure_preserved_state

reset_fixture
capture_baseline
chmod 0700 "${parent}"
printf 'sidecar only\n' >"${parent}/userdb.sqlite3-wal"
chmod 0600 "${parent}/userdb.sqlite3-wal"
expect_failure run_helper --authorized-delete-r01b-test-userdb
test -e "${parent}/userdb.sqlite3-wal"
test -e "${receipt}"

reset_fixture
capture_baseline
prepare_userdb_family
chmod 0644 "${parent}/userdb.sqlite3-shm"
expect_failure run_helper --authorized-delete-r01b-test-userdb
assert_failure_preserved_state

reset_fixture
capture_baseline
prepare_userdb_family
rm "${parent}/userdb.sqlite3-wal"
ln -s "${parent}/userdb.sqlite3" "${parent}/userdb.sqlite3-wal"
expect_failure run_helper --authorized-delete-r01b-test-userdb
assert_failure_preserved_state

reset_fixture
capture_baseline
prepare_userdb_family
chmod 0755 "${parent}"
expect_failure run_helper --authorized-delete-r01b-test-userdb
test -e "${parent}/userdb.sqlite3"
test -e "${receipt}"

reset_fixture
capture_baseline
prepare_userdb_family
chmod 0644 "${receipt}"
expect_failure run_helper --authorized-delete-r01b-test-userdb
assert_failure_preserved_state

reset_fixture
capture_baseline
prepare_userdb_family
chmod 0777 "${state_dir}"
expect_failure run_helper --authorized-delete-r01b-test-userdb
assert_failure_preserved_state

reset_fixture
capture_baseline
prepare_userdb_family
ln "${receipt}" "${state_dir}/receipt-hardlink"
expect_failure run_helper --authorized-delete-r01b-test-userdb
assert_failure_preserved_state

reset_fixture
capture_baseline
prepare_userdb_family
cp "${receipt}" "${state_dir}/replacement-receipt"
chmod 0600 "${state_dir}/replacement-receipt"
mv -f "${state_dir}/replacement-receipt" "${receipt}"
expect_failure run_helper --authorized-delete-r01b-test-userdb
assert_failure_preserved_state

reset_fixture
capture_baseline
prepare_userdb_family
printf 'tampered\n' >"${receipt}"
chmod 0600 "${receipt}"
expect_failure run_helper --authorized-delete-r01b-test-userdb
assert_failure_preserved_state

reset_fixture
capture_baseline
prepare_userdb_family
mv "${parent}" "${contract_root}/original-parent"
mkdir "${parent}"
chmod 0700 "${parent}"
printf 'replacement\n' >"${parent}/userdb.sqlite3"
chmod 0600 "${parent}/userdb.sqlite3"
expect_failure run_helper --authorized-delete-r01b-test-userdb
test -e "${parent}/userdb.sqlite3"
test -e "${receipt}"
test -e "${contract_root}/original-parent/userdb.sqlite3"

reset_fixture
capture_baseline
prepare_userdb_family
rm "${receipt}"
printf 'radishlex-r01b-test-userdb-baseline-v1\n' >"${state_dir}/target"
chmod 0600 "${state_dir}/target"
ln -s "${state_dir}/target" "${receipt}"
expect_failure run_helper --authorized-delete-r01b-test-userdb
test -e "${parent}/userdb.sqlite3"

rg -Fq 'openat(parent' "${source}"
rg -Fq 'RLXOpenDirectoryAt(parent_fd, ".")' "${source}"
rg -Fq 'fstatat(parent_fd' "${source}"
rg -Fq 'AT_SYMLINK_NOFOLLOW' "${source}"
rg -Fq 'unlinkat(parent_fd, kTestDataNames[index], 0)' "${source}"
if rg -n '(^|[^A-Za-z])(remove|rename|system|popen)[[:space:]]*\(' "${source}"; then
  echo "R01B userdb helper contains a general path or process primitive" >&2
  exit 1
fi

echo "R01B test userdb cleanup helper contract passed"
