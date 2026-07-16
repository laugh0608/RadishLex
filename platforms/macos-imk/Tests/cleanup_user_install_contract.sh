#!/usr/bin/env bash
set -euo pipefail

fail() {
  echo "cleanup user install contract failed: $*" >&2
  exit 1
}

run_stub() {
  local code=0
  local command_name="${0##*/}"
  local output=""
  local tis_call=""
  local tis_state_path=""

  : "${RADISHLEX_CLEANUP_CONTRACT_ROOT:?contract root is required}"
  : "${RADISHLEX_CLEANUP_CONTRACT_LOG:?contract log is required}"
  : "${RADISHLEX_CLEANUP_CONTRACT_SCRIPT:?contract script is required}"

  case "${command_name}" in
    clang)
      [[ $# -eq 13 && "${1}" == "-fobjc-arc" && \
        "${2}" == "-fmodules" && "${3}" == "-Wall" && \
        "${4}" == "-Wextra" && "${5}" == "-Werror" && \
        "${6}" == "-mmacosx-version-min=13.0" && \
        "${7}" == "${RADISHLEX_CLEANUP_CONTRACT_CLEANUP_SCRIPT%/*}/Tools/tis_source_status.m" && \
        "${8}" == "-framework" && "${9}" == "Carbon" && \
        "${10}" == "-framework" && "${11}" == "Foundation" && \
        "${12}" == "-o" && \
        "${13}" == "${RADISHLEX_CLEANUP_CONTRACT_TOOL_DIR}/tis-source-status" ]] || \
        fail "clang stub received unexpected arguments"
      [[ -d "${RADISHLEX_CLEANUP_CONTRACT_TOOL_DIR}" && \
        ! -L "${RADISHLEX_CLEANUP_CONTRACT_TOOL_DIR}" && \
        -d "${RADISHLEX_CLEANUP_CONTRACT_CLANG_CACHE}" && \
        ! -L "${RADISHLEX_CLEANUP_CONTRACT_CLANG_CACHE}" ]] || \
        fail "clang ran before production created its fixed tool directories"
      output="${13}"
      /bin/ln -sf "${RADISHLEX_CLEANUP_CONTRACT_SCRIPT}" "${output}"
      echo "clang" >>"${RADISHLEX_CLEANUP_CONTRACT_LOG}"
      ;;
    dirname)
      [[ $# -eq 2 && "${1}" == "--" ]] || \
        fail "dirname stub received unexpected arguments"
      case "${2}" in
        "${RADISHLEX_CLEANUP_CONTRACT_WRAPPER}")
          printf '%s\n' "${RADISHLEX_CLEANUP_CONTRACT_WRAPPER%/*}"
          ;;
        "${RADISHLEX_CLEANUP_CONTRACT_CLEANUP_SCRIPT}")
          printf '%s\n' "${RADISHLEX_CLEANUP_CONTRACT_CLEANUP_SCRIPT%/*}"
          ;;
        *)
          fail "dirname stub received a path outside the two cleanup scripts"
          ;;
      esac
      echo "dirname" >>"${RADISHLEX_CLEANUP_CONTRACT_LOG}"
      ;;
    find)
      [[ $# -eq 7 && \
        "${1}" == "${RADISHLEX_CLEANUP_CONTRACT_APPLICATION_SUPPORT_PARENT}" && \
        "${2}" == "-mindepth" && "${3}" == "1" && \
        "${4}" == "-maxdepth" && "${5}" == "1" && \
        "${6}" == "-print" && "${7}" == "-quit" ]] || \
        fail "find stub received unexpected arguments"
      echo "find" >>"${RADISHLEX_CLEANUP_CONTRACT_LOG}"
      /usr/bin/find "$@"
      ;;
    grep)
      if [[ $# -eq 2 && "${1}" == "-Eq" && \
        ("${2}" == '^source_id=.* selected=1 ' || \
        "${2}" == '^source_id=.* enabled=1 selected=0 select_capable=0 ') ]]; then
        :
      elif [[ $# -eq 2 && "${1}" == "-q" && \
        "${2}" == '^matches=0 enabled=0 selected=0$' ]]; then
        :
      else
        fail "grep stub received unexpected arguments"
      fi
      if /usr/bin/grep "$@"; then
        code=0
      else
        code=$?
      fi
      echo "grep" >>"${RADISHLEX_CLEANUP_CONTRACT_LOG}"
      return "${code}"
      ;;
    mkdir)
      [[ $# -eq 3 && "${1}" == "-p" && \
        "${2}" == "${RADISHLEX_CLEANUP_CONTRACT_TOOL_DIR}" && \
        "${3}" == "${RADISHLEX_CLEANUP_CONTRACT_CLANG_CACHE}" ]] || \
        fail "mkdir stub received unexpected arguments"
      echo "mkdir" >>"${RADISHLEX_CLEANUP_CONTRACT_LOG}"
      /bin/mkdir -p -- "${2}" "${3}"
      ;;
    sed)
      [[ $# -eq 1 && \
        "${1}" == 's/[][\\.^$*+?(){}|]/\\&/g' ]] || \
        fail "sed stub received unexpected arguments"
      echo "sed" >>"${RADISHLEX_CLEANUP_CONTRACT_LOG}"
      /usr/bin/sed "${1}"
      ;;
    stat)
      [[ $# -eq 3 && "${1}" == "-f" && "${2}" == "%Lp" && \
        "${3}" == "${RADISHLEX_CLEANUP_CONTRACT_APPLICATION_SUPPORT_PARENT}" ]] || \
        fail "stat stub received unexpected arguments"
      echo "stat" >>"${RADISHLEX_CLEANUP_CONTRACT_LOG}"
      /usr/bin/stat "$@"
      ;;
    tis-source-status)
      [[ $# -eq 1 && "${1}" == "org.radishlex.inputmethod.macos" ]] || \
        fail "TIS stub received unexpected arguments"
      tis_call="$(<"${RADISHLEX_CLEANUP_CONTRACT_TIS_CURSOR}")"
      [[ "${tis_call}" =~ ^[12]$ ]] || \
        fail "TIS stub exceeded the two allowed inspections"
      tis_state_path="${RADISHLEX_CLEANUP_CONTRACT_TIS_STATE_PREFIX}.${tis_call}"
      [[ -f "${tis_state_path}.output" && -f "${tis_state_path}.exit" ]] || \
        fail "TIS stub state slot is incomplete"
      printf '%s\n' "$((tis_call + 1))" \
        >"${RADISHLEX_CLEANUP_CONTRACT_TIS_CURSOR}"
      echo "tis" >>"${RADISHLEX_CLEANUP_CONTRACT_LOG}"
      /bin/cat "${tis_state_path}.output"
      code="$(<"${tis_state_path}.exit")"
      [[ "${code}" =~ ^[0-9]+$ ]] || fail "TIS stub exit code is invalid"
      return "${code}"
      ;;
    pgrep)
      [[ $# -eq 2 && "${1}" == "-x" && "${2}" == "RadishLex" ]] || \
        fail "pgrep stub received unexpected arguments"
      echo "pgrep" >>"${RADISHLEX_CLEANUP_CONTRACT_LOG}"
      case "$(<"${RADISHLEX_CLEANUP_CONTRACT_PROCESS_STATE}")" in
        running_verified|running_unverified|running_stubborn|running_then_unavailable|running_swap_ancestor)
          echo "4242"
          ;;
        stopped)
          return 1
          ;;
        unavailable)
          return 2
          ;;
        *)
          fail "pgrep stub received an invalid process state"
          ;;
      esac
      ;;
    ps)
      [[ $# -eq 5 && "${1}" == "-ww" && "${2}" == "-p" && \
        "${3}" == "4242" && "${4}" == "-o" && "${5}" == "command=" ]] || \
        fail "ps stub received unexpected arguments"
      echo "ps" >>"${RADISHLEX_CLEANUP_CONTRACT_LOG}"
      case "$(<"${RADISHLEX_CLEANUP_CONTRACT_PROCESS_STATE}")" in
        running_verified|running_stubborn|running_then_unavailable|running_swap_ancestor)
          echo "${RADISHLEX_CLEANUP_CONTRACT_PROCESS_EXECUTABLE}"
          ;;
        *)
          echo "/tmp/Unrelated/RadishLex"
          ;;
      esac
      ;;
    pkill)
      [[ $# -eq 2 && "${1}" == "-f" ]] || \
        fail "pkill stub received unexpected arguments"
      [[ "${2}" == "${RADISHLEX_CLEANUP_CONTRACT_PROCESS_PATTERN}" ]] || \
        fail "pkill pattern does not exactly match the fixed executable pattern"
      [[ "${RADISHLEX_CLEANUP_CONTRACT_PROCESS_EXECUTABLE}" =~ ${2} ]] || \
        fail "pkill pattern does not match the exact executable"
      [[ "${RADISHLEX_CLEANUP_CONTRACT_PROCESS_EXECUTABLE} --argument" =~ ${2} ]] || \
        fail "pkill pattern does not match executable arguments"
      if [[ "${RADISHLEX_CLEANUP_CONTRACT_PROCESS_EXECUTABLE}-other" =~ ${2} ]]; then
        fail "pkill pattern accepts a similar executable prefix"
      fi
      if [[ "/tmp/Unrelated/RadishLex" =~ ${2} ]]; then
        fail "pkill pattern accepts an unrelated same-name executable"
      fi
      [[ -e "${RADISHLEX_CLEANUP_CONTRACT_RUNTIME_DATA}" ]] || \
        fail "pkill ran after Rime runtime data was removed"
      echo "pkill" >>"${RADISHLEX_CLEANUP_CONTRACT_LOG}"
      case "$(<"${RADISHLEX_CLEANUP_CONTRACT_PROCESS_STATE}")" in
        running_verified)
          echo "stopped" >"${RADISHLEX_CLEANUP_CONTRACT_PROCESS_STATE}"
          ;;
        running_then_unavailable)
          echo "unavailable" >"${RADISHLEX_CLEANUP_CONTRACT_PROCESS_STATE}"
          ;;
        running_stubborn)
          ;;
        running_swap_ancestor)
          [[ -d "${RADISHLEX_CLEANUP_CONTRACT_INPUT_METHODS_PARENT}" && \
            ! -e "${RADISHLEX_CLEANUP_CONTRACT_SWAPPED_INPUT_METHODS}" ]] || \
            fail "ancestor swap precondition is invalid"
          /bin/mv "${RADISHLEX_CLEANUP_CONTRACT_INPUT_METHODS_PARENT}" \
            "${RADISHLEX_CLEANUP_CONTRACT_SWAPPED_INPUT_METHODS}"
          /bin/ln -s "${RADISHLEX_CLEANUP_CONTRACT_EXTERNAL_INPUT_METHODS}" \
            "${RADISHLEX_CLEANUP_CONTRACT_INPUT_METHODS_PARENT}"
          echo "stopped" >"${RADISHLEX_CLEANUP_CONTRACT_PROCESS_STATE}"
          ;;
        *)
          fail "pkill stub received an invalid process state"
          ;;
      esac
      ;;
    sleep)
      [[ $# -eq 1 && "${1}" == "0.1" ]] || \
        fail "sleep stub received unexpected arguments"
      echo "sleep" >>"${RADISHLEX_CLEANUP_CONTRACT_LOG}"
      ;;
    rm)
      [[ $# -eq 2 && "${1}" == "-rf" ]] || \
        fail "rm stub received unexpected arguments"
      case "${2}" in
        "${RADISHLEX_CLEANUP_CONTRACT_INSTALLED_BUNDLE}")
          echo "rm:installed_bundle" >>"${RADISHLEX_CLEANUP_CONTRACT_LOG}"
          ;;
        "${RADISHLEX_CLEANUP_CONTRACT_RUNTIME_DATA}")
          echo "rm:runtime_data" >>"${RADISHLEX_CLEANUP_CONTRACT_LOG}"
          ;;
        *)
          fail "rm stub received a path outside the fixed cleanup targets"
          ;;
      esac
      /bin/rm -rf -- "${2}"
      ;;
    *)
      fail "unexpected stub command ${command_name}"
      ;;
  esac
}

case "${0##*/}" in
  clang|dirname|find|grep|mkdir|sed|stat|tis-source-status|pgrep|ps|pkill|sleep|rm)
    run_stub "$@"
    exit 0
    ;;
esac

script_dir="$(CDPATH= cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(CDPATH= cd -- "${script_dir}/../../.." && pwd)"
contract_script="${script_dir}/cleanup_user_install_contract.sh"
contract_root="$(mktemp -d "${TMPDIR:-/tmp}/radishlex-cleanup-contract.XXXXXX")"
contract_root="$(CDPATH= cd -- "${contract_root}" && pwd -P)"
fake_repo="${contract_root}/repo"
fake_home="${contract_root}/home"
fake_bin="${contract_root}/bin"
contract_log="${contract_root}/commands.log"
process_state="${contract_root}/process.state"
tis_state_prefix="${contract_root}/tis.state"
tis_cursor="${contract_root}/tis.cursor"
library_dir="${fake_home}/Library"
input_methods_parent="${library_dir}/Input Methods"
application_support_root="${library_dir}/Application Support"
parent="${fake_home}/Library/Application Support/RadishLex"
runtime_data="${parent}/Rime"
userdb="${parent}/userdb.sqlite3"
installed_bundle="${fake_home}/Library/Input Methods/RadishLexInputMethod.app"
process_executable="${installed_bundle}/Contents/MacOS/RadishLex"
process_pattern="^$(printf '%s' "${process_executable}" | \
  /usr/bin/sed 's/[][\\.^$*+?(){}|]/\\&/g')([[:space:]]|$)"
external_input_methods="${contract_root}/external-input-methods"
swapped_input_methods="${contract_root}/swapped-input-methods"
cleanup_script_copy="${fake_repo}/platforms/macos-imk/cleanup-user-install.sh"
cleanup_wrapper_copy="${fake_repo}/scripts/cleanup-macos-imk.sh"
tool_dir="${fake_repo}/target/macos-imk/tools"
clang_cache="${tool_dir}/clang-module-cache"

cleanup_contract() {
  rm -rf "${contract_root}"
}
trap cleanup_contract EXIT

mkdir -p "${fake_repo}/platforms/macos-imk" "${fake_repo}/scripts" \
  "${fake_home}" "${fake_bin}" "${contract_root}/tmp"
cp "${repo_root}/platforms/macos-imk/cleanup-user-install.sh" \
  "${fake_repo}/platforms/macos-imk/cleanup-user-install.sh"
cp "${repo_root}/scripts/cleanup-macos-imk.sh" \
  "${fake_repo}/scripts/cleanup-macos-imk.sh"
chmod +x "${fake_repo}/platforms/macos-imk/cleanup-user-install.sh" \
  "${fake_repo}/scripts/cleanup-macos-imk.sh"
for command_name in clang dirname find grep mkdir pgrep ps pkill rm sed sleep stat; do
  ln -s "${contract_script}" "${fake_bin}/${command_name}"
done
ln -s /bin/bash "${fake_bin}/bash"
: >"${contract_log}"
echo "stopped" >"${process_state}"

run_cleanup() {
  echo "1" >"${tis_cursor}"
  (
    cd "${contract_root}"
    env -i \
      HOME="${fake_home}" \
      PATH="${fake_bin}" \
      TMPDIR="${contract_root}/tmp" \
      RADISHLEX_CLEANUP_CONTRACT_ROOT="${contract_root}" \
      RADISHLEX_CLEANUP_CONTRACT_LOG="${contract_log}" \
      RADISHLEX_CLEANUP_CONTRACT_SCRIPT="${contract_script}" \
      RADISHLEX_CLEANUP_CONTRACT_CLEANUP_SCRIPT="${cleanup_script_copy}" \
      RADISHLEX_CLEANUP_CONTRACT_WRAPPER="${cleanup_wrapper_copy}" \
      RADISHLEX_CLEANUP_CONTRACT_TOOL_DIR="${tool_dir}" \
      RADISHLEX_CLEANUP_CONTRACT_CLANG_CACHE="${clang_cache}" \
      RADISHLEX_CLEANUP_CONTRACT_APPLICATION_SUPPORT_PARENT="${parent}" \
      RADISHLEX_CLEANUP_CONTRACT_PROCESS_STATE="${process_state}" \
      RADISHLEX_CLEANUP_CONTRACT_PROCESS_PATTERN="${process_pattern}" \
      RADISHLEX_CLEANUP_CONTRACT_TIS_STATE_PREFIX="${tis_state_prefix}" \
      RADISHLEX_CLEANUP_CONTRACT_TIS_CURSOR="${tis_cursor}" \
      RADISHLEX_CLEANUP_CONTRACT_PROCESS_EXECUTABLE="${process_executable}" \
      RADISHLEX_CLEANUP_CONTRACT_INSTALLED_BUNDLE="${installed_bundle}" \
      RADISHLEX_CLEANUP_CONTRACT_INPUT_METHODS_PARENT="${input_methods_parent}" \
      RADISHLEX_CLEANUP_CONTRACT_EXTERNAL_INPUT_METHODS="${external_input_methods}" \
      RADISHLEX_CLEANUP_CONTRACT_SWAPPED_INPUT_METHODS="${swapped_input_methods}" \
      RADISHLEX_CLEANUP_CONTRACT_RUNTIME_DATA="${runtime_data}" \
      "${fake_repo}/scripts/cleanup-macos-imk.sh" "$@"
  )
}

assert_status() {
  local parent_presence="${1}"
  local parent_kind="${2}"
  local parent_mode="${3}"
  local runtime_presence="${4}"
  local userdb_presence="${5}"
  local sidecar_presence="${6}"
  local ancestor_safety="${7}"
  local actual
  local expected

  actual="$(run_cleanup --status)"
  expected="$(printf '%s\n' \
    'matches=0 enabled=0 selected=0' \
    'installed_bundle=absent' \
    "cleanup_path_ancestors=${ancestor_safety}" \
    "application_support_parent=${parent_presence}" \
    "application_support_parent_kind=${parent_kind}" \
    "application_support_parent_mode=${parent_mode}" \
    "runtime_data=${runtime_presence}" \
    "userdb=${userdb_presence}" \
    "userdb_sidecars=${sidecar_presence}" \
    'process=stopped')"
  if [[ "${actual}" != "${expected}" ]]; then
    printf 'expected status:\n%s\nactual status:\n%s\n' \
      "${expected}" "${actual}" >&2
    fail "status output mismatch"
  fi
}

prepare_deletion_targets() {
  rm -rf "${installed_bundle}" "${runtime_data}"
  mkdir -p "${installed_bundle}" "${runtime_data}"
  printf 'bundle sentinel\n' >"${installed_bundle}/sentinel"
  printf 'runtime sentinel\n' >"${runtime_data}/sentinel"
}

assert_deletion_targets_present() {
  [[ -f "${installed_bundle}/sentinel" ]] || \
    fail "cleanup changed the installed bundle before refusing"
  [[ -f "${runtime_data}/sentinel" ]] || \
    fail "cleanup changed Rime runtime data before refusing"
}

assert_no_pkill() {
  if grep -q '^pkill$' "${contract_log}"; then
    fail "refused cleanup attempted process termination"
  fi
}

assert_no_rm() {
  if grep -q '^rm:' "${contract_log}"; then
    fail "refused cleanup attempted fixed-path removal"
  fi
}

set_tis_slot() {
  local index="${1}"
  local exit_code="${2}"
  shift 2

  [[ "${index}" =~ ^[12]$ && "${exit_code}" =~ ^[0-9]+$ && $# -gt 0 ]] || \
    fail "invalid TIS state slot"
  printf '%s\n' "$@" >"${tis_state_prefix}.${index}.output"
  printf '%s\n' "${exit_code}" >"${tis_state_prefix}.${index}.exit"
}

set_zero_tis_state() {
  set_tis_slot 1 0 'matches=0 enabled=0 selected=0'
  set_tis_slot 2 0 'matches=0 enabled=0 selected=0'
}

set_selected_tis_state() {
  set_tis_slot 1 4 \
    'source_id=org.radishlex.inputmethod.macos.Pinyin bundle_id=org.radishlex.inputmethod.macos enabled=1 selected=1 select_capable=1' \
    'matches=1 enabled=1 selected=1'
  set_tis_slot 2 4 \
    'source_id=org.radishlex.inputmethod.macos.Pinyin bundle_id=org.radishlex.inputmethod.macos enabled=1 selected=1 select_capable=1' \
    'matches=1 enabled=1 selected=1'
}

set_enabled_parent_tis_state() {
  set_tis_slot 1 4 \
    'source_id=org.radishlex.inputmethod.macos enabled=1 selected=0 select_capable=0 bundle_id=org.radishlex.inputmethod.macos' \
    'matches=1 enabled=1 selected=0'
  set_tis_slot 2 4 \
    'source_id=org.radishlex.inputmethod.macos enabled=1 selected=0 select_capable=0 bundle_id=org.radishlex.inputmethod.macos' \
    'matches=1 enabled=1 selected=0'
}

set_enabled_mode_then_zero_tis_state() {
  set_tis_slot 1 4 \
    'source_id=org.radishlex.inputmethod.macos.Pinyin bundle_id=org.radishlex.inputmethod.macos enabled=1 selected=0 select_capable=1' \
    'matches=1 enabled=1 selected=0'
  set_tis_slot 2 0 'matches=0 enabled=0 selected=0'
}

set_zero_then_residual_tis_state() {
  set_tis_slot 1 0 'matches=0 enabled=0 selected=0'
  set_tis_slot 2 0 \
    'source_id=org.radishlex.inputmethod.macos.Pinyin bundle_id=org.radishlex.inputmethod.macos enabled=0 selected=0 select_capable=1' \
    'matches=1 enabled=0 selected=0'
}

assert_two_tis_calls() {
  [[ "$(grep -c '^tis$' "${contract_log}")" == "2" ]] || \
    fail "cleanup did not inspect TIS exactly twice"
  [[ "$(<"${tis_cursor}")" == "3" ]] || \
    fail "TIS state queue did not consume exactly two slots"
}

assert_bootstrap_command_order() {
  local actual
  local expected

  actual="$(<"${contract_log}")"
  expected="$(printf '%s\n' dirname dirname sed mkdir clang tis pgrep)"
  [[ "${actual}" == "${expected}" ]] || {
    printf 'expected bootstrap commands:\n%s\nactual bootstrap commands:\n%s\n' \
      "${expected}" "${actual}" >&2
    fail "status bootstrap command order changed unexpectedly"
  }
}

set_zero_tis_state
assert_status absent absent not_applicable absent absent absent safe
assert_bootstrap_command_order

mkdir -p "${parent}"
parent_mode="$(stat -f '%Lp' "${parent}")"
assert_status present empty_directory "${parent_mode}" absent absent absent safe

mkdir -p "${runtime_data}"
assert_status present nonempty_directory "${parent_mode}" present absent absent safe

rmdir "${runtime_data}"
touch "${userdb}"
assert_status present nonempty_directory "${parent_mode}" absent present absent safe

rm "${userdb}"
ln -s missing-userdb "${userdb}"
assert_status present nonempty_directory "${parent_mode}" absent present absent safe

rm "${userdb}"
ln -s missing-wal "${userdb}-wal"
assert_status present nonempty_directory "${parent_mode}" absent absent present safe

rm "${userdb}-wal"
ln -s missing-shm "${userdb}-shm"
assert_status present nonempty_directory "${parent_mode}" absent absent present safe

rm "${userdb}-shm"
ln -s missing-journal "${userdb}-journal"
assert_status present nonempty_directory "${parent_mode}" absent absent present safe

rm -rf "${parent}"
external_parent="${contract_root}/external-parent"
mkdir -p "${external_parent}/Rime"
printf 'external sentinel\n' >"${external_parent}/Rime/sentinel"
ln -s "${external_parent}" "${parent}"
assert_status present unsafe_type not_applicable present absent absent unsafe

: >"${contract_log}"
set +e
unsafe_cleanup_output="$(run_cleanup --authorized-after-settings-removal 2>&1)"
unsafe_cleanup_code=$?
set -e
[[ ${unsafe_cleanup_code} -eq 4 ]] || \
  fail "cleanup did not reject an unsafe application support parent"
[[ -L "${parent}" ]] || fail "cleanup changed the unsafe parent before refusing"
[[ -f "${external_parent}/Rime/sentinel" ]] || \
  fail "cleanup followed the unsafe parent before refusing"
grep -Fq 'Refusing cleanup without a safe fixed parent path.' \
  <<<"${unsafe_cleanup_output}" || fail "unsafe parent refusal reason is missing"
[[ "$(grep -c '^tis$' "${contract_log}")" == "1" ]] || \
  fail "unsafe parent cleanup did not stop after the initial TIS inspection"
assert_no_pkill
assert_no_rm

rm "${parent}"
rmdir "${application_support_root}"
external_application_support="${contract_root}/external-application-support"
mkdir -p "${external_application_support}/RadishLex/Rime"
printf 'external ancestor sentinel\n' \
  >"${external_application_support}/RadishLex/Rime/sentinel"
ln -s "${external_application_support}" "${application_support_root}"
ancestor_status="$(run_cleanup --status)"
grep -Fxq 'cleanup_path_ancestors=unsafe' <<<"${ancestor_status}" || \
  fail "status did not expose an unsafe application support ancestor"
: >"${contract_log}"
set +e
unsafe_ancestor_output="$(run_cleanup --authorized-after-settings-removal 2>&1)"
unsafe_ancestor_code=$?
set -e
[[ ${unsafe_ancestor_code} -eq 10 ]] || \
  fail "cleanup did not reject a symlink application support ancestor"
grep -Fq 'Refusing cleanup without ordinary fixed-path ancestors.' \
  <<<"${unsafe_ancestor_output}" || fail "unsafe ancestor refusal reason is missing"
[[ -f "${external_application_support}/RadishLex/Rime/sentinel" ]] || \
  fail "cleanup followed a symlink application support ancestor"
assert_no_pkill
assert_no_rm
rm "${application_support_root}"
mkdir -p "${parent}"

mkdir -p "${external_input_methods}/RadishLexInputMethod.app"
printf 'external bundle sentinel\n' \
  >"${external_input_methods}/RadishLexInputMethod.app/sentinel"
ln -s "${external_input_methods}" "${input_methods_parent}"
install_ancestor_status="$(run_cleanup --status)"
grep -Fxq 'cleanup_path_ancestors=unsafe' <<<"${install_ancestor_status}" || \
  fail "status did not expose an unsafe input methods ancestor"
: >"${contract_log}"
set +e
unsafe_install_output="$(run_cleanup --authorized-after-settings-removal 2>&1)"
unsafe_install_code=$?
set -e
[[ ${unsafe_install_code} -eq 10 ]] || \
  fail "cleanup did not reject a symlink input methods ancestor"
grep -Fq 'Refusing cleanup without ordinary fixed-path ancestors.' \
  <<<"${unsafe_install_output}" || fail "unsafe install ancestor reason is missing"
[[ -f "${external_input_methods}/RadishLexInputMethod.app/sentinel" ]] || \
  fail "cleanup followed a symlink input methods ancestor"
assert_no_pkill
assert_no_rm
rm "${input_methods_parent}"

prepare_deletion_targets
set_selected_tis_state
echo "stopped" >"${process_state}"
: >"${contract_log}"
set +e
selected_output="$(run_cleanup --authorized-after-settings-removal 2>&1)"
selected_code=$?
set -e
[[ ${selected_code} -eq 3 ]] || fail "cleanup did not reject a selected source"
grep -Fq 'RadishLex is still selected' <<<"${selected_output}" || \
  fail "selected source refusal reason is missing"
assert_deletion_targets_present
assert_no_pkill
assert_no_rm

set_enabled_parent_tis_state
: >"${contract_log}"
set +e
enabled_parent_output="$(run_cleanup --authorized-after-settings-removal 2>&1)"
enabled_parent_code=$?
set -e
[[ ${enabled_parent_code} -eq 3 ]] || \
  fail "cleanup did not reject an enabled non-selectable parent"
grep -Fq 'non-selectable parent is enabled' <<<"${enabled_parent_output}" || \
  fail "enabled parent refusal reason is missing"
assert_deletion_targets_present
assert_no_pkill
assert_no_rm
set_zero_tis_state

echo "unavailable" >"${process_state}"
: >"${contract_log}"
set +e
unavailable_output="$(run_cleanup --authorized-after-settings-removal 2>&1)"
unavailable_code=$?
set -e
[[ ${unavailable_code} -eq 8 ]] || \
  fail "cleanup did not reject an unavailable process state"
grep -Fq 'Refusing cleanup while process state is unavailable.' \
  <<<"${unavailable_output}" || fail "unavailable process refusal reason is missing"
assert_deletion_targets_present
assert_no_pkill
assert_no_rm

echo "running_unverified" >"${process_state}"
: >"${contract_log}"
set +e
unverified_output="$(run_cleanup --authorized-after-settings-removal 2>&1)"
unverified_code=$?
set -e
[[ ${unverified_code} -eq 9 ]] || \
  fail "cleanup did not reject an unverified same-name process"
grep -Fq 'Refusing cleanup without exact process identity.' \
  <<<"${unverified_output}" || fail "unverified process refusal reason is missing"
assert_deletion_targets_present
assert_no_pkill
assert_no_rm

echo "running_then_unavailable" >"${process_state}"
: >"${contract_log}"
set +e
post_kill_unavailable_output="$(run_cleanup --authorized-after-settings-removal 2>&1)"
post_kill_unavailable_code=$?
set -e
[[ ${post_kill_unavailable_code} -eq 6 ]] || \
  fail "cleanup did not reject unavailable post-termination observation"
grep -Fq 'process did not stop before path cleanup' \
  <<<"${post_kill_unavailable_output}" || \
  fail "post-termination observation refusal reason is missing"
assert_deletion_targets_present
[[ "$(grep -c '^pkill$' "${contract_log}")" == "1" ]] || \
  fail "post-termination observation case did not call the pkill stub once"
assert_no_rm

echo "running_stubborn" >"${process_state}"
: >"${contract_log}"
set +e
stubborn_output="$(run_cleanup --authorized-after-settings-removal 2>&1)"
stubborn_code=$?
set -e
[[ ${stubborn_code} -eq 6 ]] || \
  fail "cleanup did not reject a process that remained running"
grep -Fq 'process did not stop before path cleanup' <<<"${stubborn_output}" || \
  fail "stubborn process refusal reason is missing"
assert_deletion_targets_present
[[ "$(grep -c '^pkill$' "${contract_log}")" == "1" ]] || \
  fail "stubborn process case did not call the pkill stub once"
[[ "$(grep -c '^sleep$' "${contract_log}")" == "20" ]] || \
  fail "stubborn process case did not complete the bounded wait"
assert_no_rm

echo "running_swap_ancestor" >"${process_state}"
: >"${contract_log}"
set +e
swapped_ancestor_output="$(run_cleanup --authorized-after-settings-removal 2>&1)"
swapped_ancestor_code=$?
set -e
[[ ${swapped_ancestor_code} -eq 10 ]] || \
  fail "cleanup did not reject an ancestor changed after process termination"
grep -Fq 'Refusing cleanup without ordinary fixed-path ancestors.' \
  <<<"${swapped_ancestor_output}" || \
  fail "post-termination ancestor refusal reason is missing"
[[ -L "${input_methods_parent}" ]] || \
  fail "ancestor swap case did not install its test symlink"
[[ -f "${swapped_input_methods}/RadishLexInputMethod.app/sentinel" ]] || \
  fail "cleanup changed the moved bundle after the ancestor swap"
[[ -f "${runtime_data}/sentinel" ]] || \
  fail "cleanup changed Rime data after the ancestor swap"
[[ -f "${external_input_methods}/RadishLexInputMethod.app/sentinel" ]] || \
  fail "cleanup followed the replacement input methods ancestor"
[[ "$(grep -c '^pkill$' "${contract_log}")" == "1" ]] || \
  fail "ancestor swap case did not call the pkill stub once"
assert_no_rm
rm "${input_methods_parent}"
mv "${swapped_input_methods}" "${input_methods_parent}"

set_enabled_mode_then_zero_tis_state
echo "stopped" >"${process_state}"
: >"${contract_log}"
stopped_cleanup_output="$(run_cleanup --authorized-after-settings-removal)"
[[ ! -e "${installed_bundle}" && ! -L "${installed_bundle}" ]] || \
  fail "stopped cleanup retained the installed bundle"
[[ ! -e "${runtime_data}" && ! -L "${runtime_data}" ]] || \
  fail "stopped cleanup retained Rime runtime data"
assert_no_pkill
assert_two_tis_calls
[[ "$(grep -c '^rm:' "${contract_log}")" == "2" ]] || \
  fail "stopped cleanup did not remove exactly two fixed targets"
grep -Fq 'matches=1 enabled=1 selected=0' <<<"${stopped_cleanup_output}" || \
  fail "stopped cleanup did not expose its initial enabled selectable mode"
grep -Fq 'matches=0 enabled=0 selected=0' <<<"${stopped_cleanup_output}" || \
  fail "stopped cleanup did not expose its final zero TIS state"
grep -Fq \
  'RadishLex cleanup verified: settings removal precondition, TIS, paths and process.' \
  <<<"${stopped_cleanup_output}" || fail "stopped cleanup success result is missing"

prepare_deletion_targets
set_zero_then_residual_tis_state
: >"${contract_log}"
set +e
residual_tis_output="$(run_cleanup --authorized-after-settings-removal 2>&1)"
residual_tis_code=$?
set -e
[[ ${residual_tis_code} -eq 7 ]] || \
  fail "cleanup did not reject a residual TIS source after path removal"
grep -Fq 'matches=1 enabled=0 selected=0' <<<"${residual_tis_output}" || \
  fail "residual TIS state is missing from cleanup output"
grep -Fq 'TIS still reports RadishLex.' <<<"${residual_tis_output}" || \
  fail "residual TIS refusal reason is missing"
if grep -Fq \
  'RadishLex cleanup verified: settings removal precondition, TIS, paths and process.' \
  <<<"${residual_tis_output}"; then
  fail "residual TIS cleanup reported false success"
fi
[[ ! -e "${installed_bundle}" && ! -L "${installed_bundle}" ]] || \
  fail "residual TIS cleanup retained the installed bundle"
[[ ! -e "${runtime_data}" && ! -L "${runtime_data}" ]] || \
  fail "residual TIS cleanup retained Rime runtime data"
assert_no_pkill
assert_two_tis_calls
[[ "$(grep -c '^rm:' "${contract_log}")" == "2" ]] || \
  fail "residual TIS cleanup did not remove exactly two fixed targets"

set_zero_tis_state
rm -rf "${parent}"
mkdir -p "${installed_bundle}" "${runtime_data}"
printf 'runtime sentinel\n' >"${runtime_data}/sentinel"
printf 'parent sentinel\n' >"${parent}/parent-sentinel"
printf 'userdb sentinel\n' >"${userdb}"
printf 'wal sentinel\n' >"${userdb}-wal"
printf 'shm sentinel\n' >"${userdb}-shm"
printf 'journal sentinel\n' >"${userdb}-journal"
cp "${userdb}" "${contract_root}/userdb.expected"
cp "${userdb}-wal" "${contract_root}/wal.expected"
cp "${userdb}-shm" "${contract_root}/shm.expected"
cp "${userdb}-journal" "${contract_root}/journal.expected"
parent_mode="$(stat -f '%Lp' "${parent}")"
: >"${contract_log}"
echo "running_verified" >"${process_state}"
cleanup_output="$(run_cleanup --authorized-after-settings-removal)"

[[ ! -e "${installed_bundle}" && ! -L "${installed_bundle}" ]] || \
  fail "cleanup retained the installed bundle"
[[ ! -e "${runtime_data}" && ! -L "${runtime_data}" ]] || \
  fail "cleanup retained Rime runtime data"
[[ -d "${parent}" && ! -L "${parent}" ]] || \
  fail "cleanup removed or replaced the application support parent"
[[ -f "${parent}/parent-sentinel" ]] || fail "cleanup removed an unrelated parent entry"
[[ "$(stat -f '%Lp' "${parent}")" == "${parent_mode}" ]] || \
  fail "cleanup changed the application support parent mode"
cmp -s "${contract_root}/userdb.expected" "${userdb}" || \
  fail "cleanup changed userdb"
cmp -s "${contract_root}/wal.expected" "${userdb}-wal" || \
  fail "cleanup changed the WAL sidecar"
cmp -s "${contract_root}/shm.expected" "${userdb}-shm" || \
  fail "cleanup changed the SHM sidecar"
cmp -s "${contract_root}/journal.expected" "${userdb}-journal" || \
  fail "cleanup changed the rollback journal"
assert_two_tis_calls
[[ "$(grep -c '^pgrep$' "${contract_log}")" == "3" ]] || \
  fail "cleanup did not inspect the process before termination, deletion and exit"
[[ "$(grep -c '^ps$' "${contract_log}")" == "1" ]] || \
  fail "cleanup did not verify the running process executable"
[[ "$(grep -c '^pkill$' "${contract_log}")" == "1" ]] || \
  fail "cleanup did not use the isolated process termination stub"
[[ "$(grep -c '^rm:' "${contract_log}")" == "2" ]] || \
  fail "cleanup did not remove exactly two fixed targets"
grep -Fq \
  'RadishLex cleanup verified: settings removal precondition, TIS, paths and process.' \
  <<<"${cleanup_output}" || fail "cleanup success result is missing"

echo "cleanup user install contract passed"
