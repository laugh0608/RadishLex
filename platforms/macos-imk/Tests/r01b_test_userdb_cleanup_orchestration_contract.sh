#!/usr/bin/env bash
set -euo pipefail

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
repo_root="$(CDPATH= cd -- "${script_dir}/../../.." && pwd)"
contract_root="$(mktemp -d "${TMPDIR:-/tmp}/radishlex-r01b-userdb-orchestration.XXXXXX")"
trap 'rm -rf "${contract_root}"' EXIT

fake_repo="${contract_root}/repo"
fake_bin="${contract_root}/bin"
fake_home="${contract_root}/home"
parent="${fake_home}/Library/Application Support/RadishLex"
platform_script="${fake_repo}/platforms/macos-imk/cleanup-r01b-test-userdb.sh"
wrapper="${fake_repo}/scripts/cleanup-macos-imk-r01b-test-userdb.sh"
status_script="${fake_repo}/platforms/macos-imk/cleanup-user-install.sh"
helper_source="${fake_repo}/platforms/macos-imk/Tools/r01b_test_userdb_cleanup.c"
helper_stub_source="${contract_root}/helper-stub"
status_state="${contract_root}/status-state"
lsof_state="${contract_root}/lsof-state"
helper_state="${contract_root}/helper-state"
call_log="${contract_root}/calls"

mkdir -p "${fake_repo}/platforms/macos-imk/Tools" \
  "${fake_repo}/scripts" "${fake_bin}" "${parent}"
chmod 0755 "${parent}"
cp "${repo_root}/platforms/macos-imk/cleanup-r01b-test-userdb.sh" \
  "${platform_script}"
cp "${repo_root}/scripts/cleanup-macos-imk-r01b-test-userdb.sh" \
  "${wrapper}"
cp "${repo_root}/platforms/macos-imk/Tools/r01b_test_userdb_cleanup.c" \
  "${helper_source}"
chmod +x "${platform_script}" "${wrapper}"

cat >"${status_script}" <<'STATUS_STUB'
#!/bin/bash
set -euo pipefail
[[ $# -eq 1 && "${1}" == "--status" ]] || exit 91
printf 'status\n' >>"${RADISHLEX_R01B_CONTRACT_CALL_LOG}"
mode="$(<"${RADISHLEX_R01B_CONTRACT_STATUS_STATE}")"
[[ "${mode}" != "status_failure" ]] || exit 92

installed_bundle="absent"
ancestors="safe"
parent_kind="empty_directory"
parent_mode="755"
runtime_data="absent"
userdb="absent"
sidecars="absent"
process="stopped"
matches="matches=0 enabled=0 selected=0"

case "${mode}" in
  capture_success|capture_helper_failure|delete_recovery_755|\
  status_ambiguous|status_contradictory)
    ;;
  delete_success|delete_helper_failure|lsof_open|lsof_unavailable)
    parent_kind="nonempty_directory"
    parent_mode="700"
    userdb="present"
    sidecars="present"
    ;;
  delete_main_only)
    parent_kind="nonempty_directory"
    parent_mode="700"
    userdb="present"
    ;;
  delete_recovery_700)
    parent_mode="700"
    ;;
  delete_missing_main_nonempty)
    parent_kind="nonempty_directory"
    parent_mode="700"
    sidecars="present"
    ;;
  delete_unknown_nonempty)
    parent_kind="nonempty_directory"
    parent_mode="700"
    ;;
  delete_empty_with_sidecars)
    parent_mode="700"
    sidecars="present"
    ;;
  capture_tis)
    matches="matches=1 enabled=1 selected=0"
    ;;
  capture_bundle)
    installed_bundle="present"
    ;;
  capture_ancestor)
    ancestors="unsafe"
    ;;
  capture_nonempty)
    parent_kind="nonempty_directory"
    ;;
  capture_mode)
    parent_mode="700"
    ;;
  capture_rime)
    runtime_data="present"
    ;;
  capture_userdb)
    userdb="present"
    ;;
  capture_sidecars)
    sidecars="present"
    ;;
  capture_process)
    process="running_verified"
    ;;
  delete_tis)
    parent_kind="nonempty_directory"
    parent_mode="700"
    userdb="present"
    sidecars="present"
    matches="matches=1 enabled=1 selected=0"
    ;;
  *)
    exit 93
    ;;
esac

printf '%s\n' \
  "installed_bundle=${installed_bundle}" \
  "cleanup_path_ancestors=${ancestors}" \
  "application_support_parent=present" \
  "application_support_parent_kind=${parent_kind}" \
  "application_support_parent_mode=${parent_mode}" \
  "runtime_data=${runtime_data}" \
  "userdb=${userdb}" \
  "userdb_sidecars=${sidecars}" \
  "process=${process}" \
  "${matches}"
if [[ "${mode}" == "status_ambiguous" ]]; then
  printf '%s\n' "${matches}"
fi
if [[ "${mode}" == "status_contradictory" ]]; then
  printf 'installed_bundle=present\n'
fi
STATUS_STUB

cat >"${helper_stub_source}" <<'HELPER_STUB'
#!/bin/bash
set -euo pipefail
case "${1:-}" in
  --capture-baseline)
    printf 'helper:capture\n' >>"${RADISHLEX_R01B_CONTRACT_CALL_LOG}"
    [[ "$(<"${RADISHLEX_R01B_CONTRACT_HELPER_STATE}")" != "fail_capture" ]] || exit 81
    printf 'r01b_userdb_baseline=captured\n'
    ;;
  --authorized-delete-r01b-test-userdb)
    printf 'helper:delete\n' >>"${RADISHLEX_R01B_CONTRACT_CALL_LOG}"
    [[ "$(<"${RADISHLEX_R01B_CONTRACT_HELPER_STATE}")" != "fail_delete" ]] || exit 82
    printf 'r01b_test_userdb=deleted\n'
    ;;
  *)
    exit 83
    ;;
esac
HELPER_STUB

cat >"${fake_bin}/clang" <<'CLANG_STUB'
#!/bin/bash
set -euo pipefail
output=""
while [[ $# -gt 0 ]]; do
  if [[ "${1}" == "-o" ]]; then
    shift
    output="${1:-}"
  fi
  shift || true
done
[[ -n "${output}" ]] || exit 71
/bin/cp "${RADISHLEX_R01B_CONTRACT_HELPER_STUB}" "${output}"
/bin/chmod 0700 "${output}"
CLANG_STUB

cat >"${fake_bin}/lsof" <<'LSOF_STUB'
#!/bin/bash
set -euo pipefail
[[ $# -eq 4 && "${1}" == "-n" && "${2}" == "-P" && \
  "${3}" == "--" ]] || exit 61
name="${4##*/}"
case "${name}" in
  userdb.sqlite3|userdb.sqlite3-wal|userdb.sqlite3-shm|userdb.sqlite3-journal)
    ;;
  *)
    exit 62
    ;;
esac
printf 'lsof:%s\n' "${name}" >>"${RADISHLEX_R01B_CONTRACT_CALL_LOG}"
case "$(<"${RADISHLEX_R01B_CONTRACT_LSOF_STATE}")" in
  clear)
    exit 1
    ;;
  open)
    if [[ "${name}" == "userdb.sqlite3" ]]; then
      printf 'COMMAND PID USER FD TYPE NAME\n'
      exit 0
    fi
    exit 1
    ;;
  unavailable)
    if [[ "${name}" == "userdb.sqlite3-shm" ]]; then
      printf 'lsof inspection unavailable\n' >&2
      exit 2
    fi
    exit 1
    ;;
  *)
    exit 63
    ;;
esac
LSOF_STUB

chmod +x "${status_script}" "${helper_stub_source}" \
  "${fake_bin}/clang" "${fake_bin}/lsof"
ln -s /bin/bash "${fake_bin}/bash"
ln -s /usr/bin/dirname "${fake_bin}/dirname"
ln -s /usr/bin/grep "${fake_bin}/grep"
ln -s /bin/mkdir "${fake_bin}/mkdir"

run_wrapper() {
  (
    cd "${contract_root}"
    HOME="${fake_home}" \
      PATH="${fake_bin}" \
      RADISHLEX_R01B_CONTRACT_CALL_LOG="${call_log}" \
      RADISHLEX_R01B_CONTRACT_STATUS_STATE="${status_state}" \
      RADISHLEX_R01B_CONTRACT_LSOF_STATE="${lsof_state}" \
      RADISHLEX_R01B_CONTRACT_HELPER_STATE="${helper_state}" \
      RADISHLEX_R01B_CONTRACT_HELPER_STUB="${helper_stub_source}" \
      "${wrapper}" "$@"
  )
}

expect_failure() {
  if "$@" >"${contract_root}/unexpected-output" \
    2>"${contract_root}/expected-error"; then
    echo "R01B userdb orchestration contract expected failure" >&2
    exit 1
  fi
}

reset_calls() {
  : >"${call_log}"
  printf 'clear\n' >"${lsof_state}"
  printf 'ok\n' >"${helper_state}"
}

prepare_userdb_family() {
  rm -f "${parent}/userdb.sqlite3" "${parent}/userdb.sqlite3-wal" \
    "${parent}/userdb.sqlite3-shm" "${parent}/userdb.sqlite3-journal"
  chmod 0700 "${parent}"
  for name in userdb.sqlite3 userdb.sqlite3-wal userdb.sqlite3-shm \
    userdb.sqlite3-journal; do
    printf 'synthetic\n' >"${parent}/${name}"
    chmod 0600 "${parent}/${name}"
  done
}

reset_capture_parent() {
  rm -f "${parent}/userdb.sqlite3" "${parent}/userdb.sqlite3-wal" \
    "${parent}/userdb.sqlite3-shm" "${parent}/userdb.sqlite3-journal"
  chmod 0755 "${parent}"
}

prepare_main_only() {
  reset_capture_parent
  chmod 0700 "${parent}"
  printf 'synthetic\n' >"${parent}/userdb.sqlite3"
  chmod 0600 "${parent}/userdb.sqlite3"
}

reset_calls
expect_failure run_wrapper
test ! -s "${call_log}"
expect_failure run_wrapper --authorized-delete-r01b-test-userdb /tmp/caller-path
test ! -s "${call_log}"

reset_capture_parent
reset_calls
printf 'capture_success\n' >"${status_state}"
capture_output="$(run_wrapper --capture-baseline)"
test "${capture_output}" = "r01b_userdb_baseline=captured"
test "$(cat "${call_log}")" = $'status\nhelper:capture'

for negative_state in capture_tis capture_bundle capture_ancestor \
  capture_nonempty capture_mode capture_rime capture_userdb \
  capture_sidecars capture_process status_ambiguous status_contradictory \
  status_failure; do
  reset_capture_parent
  reset_calls
  printf '%s\n' "${negative_state}" >"${status_state}"
  expect_failure run_wrapper --capture-baseline
  grep -Fqx 'status' "${call_log}"
  if grep -Fq 'helper:' "${call_log}"; then
    echo "capture negative status reached the helper: ${negative_state}" >&2
    exit 1
  fi
done

reset_capture_parent
reset_calls
printf 'capture_helper_failure\n' >"${status_state}"
printf 'fail_capture\n' >"${helper_state}"
expect_failure run_wrapper --capture-baseline
test "$(cat "${call_log}")" = $'status\nhelper:capture'

prepare_userdb_family
reset_calls
printf 'delete_success\n' >"${status_state}"
delete_output="$(run_wrapper --authorized-delete-r01b-test-userdb)"
test "${delete_output}" = "r01b_test_userdb=deleted"
test "$(cat "${call_log}")" = \
  $'status\nlsof:userdb.sqlite3\nlsof:userdb.sqlite3-wal\nlsof:userdb.sqlite3-shm\nlsof:userdb.sqlite3-journal\nhelper:delete'

prepare_main_only
reset_calls
printf 'delete_main_only\n' >"${status_state}"
main_only_output="$(run_wrapper --authorized-delete-r01b-test-userdb)"
test "${main_only_output}" = "r01b_test_userdb=deleted"
test "$(cat "${call_log}")" = \
  $'status\nlsof:userdb.sqlite3\nhelper:delete'

reset_capture_parent
chmod 0700 "${parent}"
reset_calls
printf 'delete_recovery_700\n' >"${status_state}"
recovery_700_output="$(run_wrapper --authorized-delete-r01b-test-userdb)"
test "${recovery_700_output}" = "r01b_test_userdb=deleted"
test "$(cat "${call_log}")" = $'status\nhelper:delete'

reset_capture_parent
reset_calls
printf 'delete_recovery_755\n' >"${status_state}"
recovery_755_output="$(run_wrapper --authorized-delete-r01b-test-userdb)"
test "${recovery_755_output}" = "r01b_test_userdb=deleted"
test "$(cat "${call_log}")" = $'status\nhelper:delete'

for rejected_recovery_state in delete_missing_main_nonempty \
  delete_unknown_nonempty delete_empty_with_sidecars; do
  reset_capture_parent
  reset_calls
  printf '%s\n' "${rejected_recovery_state}" >"${status_state}"
  expect_failure run_wrapper --authorized-delete-r01b-test-userdb
  test "$(cat "${call_log}")" = 'status'
done

prepare_userdb_family
reset_calls
printf 'lsof_open\n' >"${status_state}"
printf 'open\n' >"${lsof_state}"
expect_failure run_wrapper --authorized-delete-r01b-test-userdb
test "$(cat "${call_log}")" = $'status\nlsof:userdb.sqlite3'

prepare_userdb_family
reset_calls
printf 'lsof_unavailable\n' >"${status_state}"
printf 'unavailable\n' >"${lsof_state}"
expect_failure run_wrapper --authorized-delete-r01b-test-userdb
test "$(cat "${call_log}")" = \
  $'status\nlsof:userdb.sqlite3\nlsof:userdb.sqlite3-wal\nlsof:userdb.sqlite3-shm'

prepare_userdb_family
reset_calls
printf 'delete_tis\n' >"${status_state}"
expect_failure run_wrapper --authorized-delete-r01b-test-userdb
test "$(cat "${call_log}")" = 'status'

prepare_userdb_family
reset_calls
printf 'delete_helper_failure\n' >"${status_state}"
printf 'fail_delete\n' >"${helper_state}"
expect_failure run_wrapper --authorized-delete-r01b-test-userdb
test "$(tail -n 1 "${call_log}")" = 'helper:delete'
test "$(grep -c '^lsof:' "${call_log}")" = "4"

test -d "${fake_home}"
test ! -e "${contract_root}/real-home-touched"

echo "R01B test userdb cleanup orchestration contract passed"
