#!/usr/bin/env bash
set -euo pipefail

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
repo_root="$(CDPATH= cd -- "${script_dir}/../../.." && pwd)"
contract_root="$(mktemp -d "${TMPDIR:-/tmp}/radishlex-m2-manager-data-orchestration.XXXXXX")"
trap 'rm -rf "${contract_root}"' EXIT

fake_repo="${contract_root}/repo"
fake_bin="${contract_root}/bin"
fake_home="${contract_root}/home"
parent="${fake_home}/Library/Application Support/RadishLex"
platform_script="${fake_repo}/platforms/macos-imk/cleanup-m2-manager-test-data.sh"
wrapper="${fake_repo}/scripts/cleanup-macos-m2-manager-test-data.sh"
status_script="${fake_repo}/platforms/macos-imk/cleanup-user-install.sh"
helper_source="${fake_repo}/platforms/macos-imk/Tools/test_data_cleanup.c"
helper_stub_source="${contract_root}/helper-stub"
status_state="${contract_root}/status-state"
process_state="${contract_root}/process-state"
lsof_state="${contract_root}/lsof-state"
helper_state="${contract_root}/helper-state"
call_log="${contract_root}/calls"

mkdir -p "${fake_repo}/platforms/macos-imk/Tools" \
  "${fake_repo}/scripts" "${fake_bin}" "${parent}"
chmod 0755 "${parent}"
cp "${repo_root}/platforms/macos-imk/cleanup-m2-manager-test-data.sh" \
  "${platform_script}"
cp "${repo_root}/scripts/cleanup-macos-m2-manager-test-data.sh" \
  "${wrapper}"
cp "${repo_root}/platforms/macos-imk/Tools/test_data_cleanup.c" \
  "${helper_source}"
chmod +x "${platform_script}" "${wrapper}"

cat >"${status_script}" <<'STATUS_STUB'
#!/bin/bash
set -euo pipefail
[[ $# -eq 1 && "${1}" == "--status" ]] || exit 91
printf 'status\n' >>"${RADISHLEX_M2_MANAGER_CONTRACT_CALL_LOG}"
mode="$(<"${RADISHLEX_M2_MANAGER_CONTRACT_STATUS_STATE}")"
[[ "${mode}" != "status_failure" ]] || exit 92

installed_bundle="absent"
parent_kind="empty_directory"
parent_mode="755"
runtime_data="absent"
userdb="absent"
sidecars="absent"
input_process="stopped"
matches="matches=0 enabled=0 selected=0"
case "${mode}" in
  capture_success|delete_recovery_700|delete_recovery_755)
    ;;
  delete_success|lsof_open|lsof_unavailable|helper_delete_failure)
    parent_kind="nonempty_directory"
    parent_mode="700"
    userdb="present"
    sidecars="present"
    ;;
  capture_tis)
    matches="matches=1 enabled=1 selected=0"
    ;;
  capture_bundle)
    installed_bundle="present"
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
  capture_input_process)
    input_process="running_verified"
    ;;
  *)
    exit 93
    ;;
esac

printf '%s\n' \
  "${matches}" \
  "installed_bundle=${installed_bundle}" \
  "cleanup_path_ancestors=safe" \
  "application_support_parent=present" \
  "application_support_parent_kind=${parent_kind}" \
  "application_support_parent_mode=${parent_mode}" \
  "runtime_data=${runtime_data}" \
  "userdb=${userdb}" \
  "userdb_sidecars=${sidecars}" \
  "process=${input_process}"
STATUS_STUB

cat >"${helper_stub_source}" <<'HELPER_STUB'
#!/bin/bash
set -euo pipefail
case "${1:-}" in
  --capture-baseline)
    printf 'helper:capture\n' >>"${RADISHLEX_M2_MANAGER_CONTRACT_CALL_LOG}"
    [[ "$(<"${RADISHLEX_M2_MANAGER_CONTRACT_HELPER_STATE}")" != "fail_capture" ]] || exit 81
    printf 'm2_manager_test_data_baseline=captured\n'
    ;;
  --authorized-delete-m2-manager-test-data)
    printf 'helper:delete\n' >>"${RADISHLEX_M2_MANAGER_CONTRACT_CALL_LOG}"
    [[ "$(<"${RADISHLEX_M2_MANAGER_CONTRACT_HELPER_STATE}")" != "fail_delete" ]] || exit 82
    printf 'm2_manager_test_data=deleted\n'
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
/bin/cp "${RADISHLEX_M2_MANAGER_CONTRACT_HELPER_STUB}" "${output}"
/bin/chmod 0700 "${output}"
CLANG_STUB

cat >"${fake_bin}/pgrep" <<'PGREP_STUB'
#!/bin/bash
set -euo pipefail
[[ $# -eq 2 && "${1}" == "-x" && "${2}" == "radishlex_manager" ]] || exit 61
printf 'pgrep\n' >>"${RADISHLEX_M2_MANAGER_CONTRACT_CALL_LOG}"
case "$(<"${RADISHLEX_M2_MANAGER_CONTRACT_PROCESS_STATE}")" in
  stopped)
    exit 1
    ;;
  running)
    printf '12345\n'
    exit 0
    ;;
  unavailable)
    exit 2
    ;;
  *)
    exit 62
    ;;
esac
PGREP_STUB

cat >"${fake_bin}/lsof" <<'LSOF_STUB'
#!/bin/bash
set -euo pipefail
[[ $# -eq 4 && "${1}" == "-n" && "${2}" == "-P" && \
  "${3}" == "--" ]] || exit 51
name="${4##*/}"
case "${name}" in
  userdb.sqlite3|userdb.sqlite3-wal|userdb.sqlite3-shm|\
  userdb.sqlite3-journal|manager-settings.json|manager-settings.json.tmp)
    ;;
  *)
    exit 52
    ;;
esac
printf 'lsof:%s\n' "${name}" >>"${RADISHLEX_M2_MANAGER_CONTRACT_CALL_LOG}"
case "$(<"${RADISHLEX_M2_MANAGER_CONTRACT_LSOF_STATE}")" in
  clear)
    exit 1
    ;;
  open)
    [[ "${name}" != "manager-settings.json" ]] || printf 'COMMAND PID USER FD TYPE NAME\n'
    [[ "${name}" != "manager-settings.json" ]] || exit 0
    exit 1
    ;;
  unavailable)
    [[ "${name}" != "userdb.sqlite3-shm" ]] || printf 'lsof unavailable\n' >&2
    [[ "${name}" != "userdb.sqlite3-shm" ]] || exit 2
    exit 1
    ;;
  *)
    exit 53
    ;;
esac
LSOF_STUB

chmod +x "${status_script}" "${helper_stub_source}" \
  "${fake_bin}/clang" "${fake_bin}/pgrep" "${fake_bin}/lsof"
for command_name in bash chmod dirname grep mkdir stat; do
  case "${command_name}" in
    bash|chmod|mkdir)
      source_path="/bin/${command_name}"
      ;;
    *)
      source_path="/usr/bin/${command_name}"
      ;;
  esac
  ln -s "${source_path}" "${fake_bin}/${command_name}"
done

run_wrapper() {
  (
    cd "${contract_root}"
    HOME="${fake_home}" \
      PATH="${fake_bin}" \
      RADISHLEX_M2_MANAGER_CONTRACT_CALL_LOG="${call_log}" \
      RADISHLEX_M2_MANAGER_CONTRACT_STATUS_STATE="${status_state}" \
      RADISHLEX_M2_MANAGER_CONTRACT_PROCESS_STATE="${process_state}" \
      RADISHLEX_M2_MANAGER_CONTRACT_LSOF_STATE="${lsof_state}" \
      RADISHLEX_M2_MANAGER_CONTRACT_HELPER_STATE="${helper_state}" \
      RADISHLEX_M2_MANAGER_CONTRACT_HELPER_STUB="${helper_stub_source}" \
      "${wrapper}" "$@"
  )
}

expect_failure() {
  if "$@" >"${contract_root}/unexpected-output" \
    2>"${contract_root}/expected-error"; then
    echo "M2 manager test-data orchestration contract expected failure" >&2
    exit 1
  fi
}

reset_fixture() {
  rm -f "${parent}/userdb.sqlite3" "${parent}/userdb.sqlite3-wal" \
    "${parent}/userdb.sqlite3-shm" "${parent}/userdb.sqlite3-journal" \
    "${parent}/manager-settings.json" "${parent}/manager-settings.json.tmp"
  chmod 0755 "${parent}"
  : >"${call_log}"
  printf 'stopped\n' >"${process_state}"
  printf 'clear\n' >"${lsof_state}"
  printf 'ok\n' >"${helper_state}"
}

prepare_test_data() {
  reset_fixture
  chmod 0700 "${parent}"
  for name in userdb.sqlite3 userdb.sqlite3-wal userdb.sqlite3-shm \
    userdb.sqlite3-journal manager-settings.json manager-settings.json.tmp; do
    printf 'synthetic\n' >"${parent}/${name}"
    chmod 0600 "${parent}/${name}"
  done
}

reset_fixture
expect_failure run_wrapper
expect_failure run_wrapper --authorized-delete-m2-manager-test-data /tmp/path
test ! -s "${call_log}"

printf 'capture_success\n' >"${status_state}"
capture_output="$(run_wrapper --capture-baseline)"
test "${capture_output}" = "m2_manager_test_data_baseline=captured"
test "$(cat "${call_log}")" = $'status\npgrep\nhelper:capture'

for negative_state in capture_tis capture_bundle capture_nonempty capture_mode \
  capture_rime capture_userdb capture_sidecars capture_input_process \
  status_failure; do
  reset_fixture
  printf '%s\n' "${negative_state}" >"${status_state}"
  expect_failure run_wrapper --capture-baseline
  if grep -Fq 'helper:' "${call_log}"; then
    echo "capture negative status reached the helper: ${negative_state}" >&2
    exit 1
  fi
done

reset_fixture
printf 'capture_success\n' >"${status_state}"
printf 'synthetic settings\n' >"${parent}/manager-settings.json"
chmod 0600 "${parent}/manager-settings.json"
expect_failure run_wrapper --capture-baseline
test "$(tail -n 1 "${call_log}")" = 'pgrep'

reset_fixture
printf 'capture_success\n' >"${status_state}"
printf 'running\n' >"${process_state}"
expect_failure run_wrapper --capture-baseline
test "$(tail -n 1 "${call_log}")" = 'pgrep'

prepare_test_data
printf 'delete_success\n' >"${status_state}"
delete_output="$(run_wrapper --authorized-delete-m2-manager-test-data)"
test "${delete_output}" = "m2_manager_test_data=deleted"
test "$(grep -c '^lsof:' "${call_log}")" = "6"
test "$(tail -n 1 "${call_log}")" = 'helper:delete'

prepare_test_data
printf 'lsof_open\n' >"${status_state}"
printf 'open\n' >"${lsof_state}"
expect_failure run_wrapper --authorized-delete-m2-manager-test-data
test "$(tail -n 1 "${call_log}")" = 'lsof:manager-settings.json'

prepare_test_data
printf 'lsof_unavailable\n' >"${status_state}"
printf 'unavailable\n' >"${lsof_state}"
expect_failure run_wrapper --authorized-delete-m2-manager-test-data
test "$(tail -n 1 "${call_log}")" = 'lsof:userdb.sqlite3-shm'

prepare_test_data
printf 'helper_delete_failure\n' >"${status_state}"
printf 'fail_delete\n' >"${helper_state}"
expect_failure run_wrapper --authorized-delete-m2-manager-test-data
test "$(tail -n 1 "${call_log}")" = 'helper:delete'

echo "M2 manager test-data cleanup orchestration contract passed"
