#!/usr/bin/env bash
set -euo pipefail

case "${1:-}" in
  --capture-baseline|--authorized-delete-m2-manager-test-data)
    action="${1}"
    ;;
  *)
    echo "usage: $0 --capture-baseline|--authorized-delete-m2-manager-test-data" >&2
    echo "Deletion is restricted to fixed M2 manager test data and requires explicit authorization." >&2
    exit 2
    ;;
esac
if [[ $# -ne 1 ]]; then
  echo "M2 manager test-data cleanup does not accept caller-provided paths or extra arguments." >&2
  exit 2
fi

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
repo_root="$(CDPATH= cd -- "${script_dir}/../.." && pwd)"
tool_dir="${repo_root}/target/macos-imk/tools"
state_parent="${repo_root}/target/macos-manager"
state_dir="${repo_root}/target/macos-manager/m2"
helper_source="${script_dir}/Tools/test_data_cleanup.c"
helper="${tool_dir}/m2-manager-test-data-cleanup"
status_entrypoint="${script_dir}/cleanup-user-install.sh"
application_support_parent="${HOME}/Library/Application Support/RadishLex"
test_data_files=(
  "${application_support_parent}/userdb.sqlite3"
  "${application_support_parent}/userdb.sqlite3-wal"
  "${application_support_parent}/userdb.sqlite3-shm"
  "${application_support_parent}/userdb.sqlite3-journal"
  "${application_support_parent}/manager-settings.json"
  "${application_support_parent}/manager-settings.json.tmp"
)

mkdir -p "${tool_dir}" "${state_parent}"
if [[ -e "${state_dir}" || -L "${state_dir}" ]]; then
  if [[ -L "${state_dir}" || ! -d "${state_dir}" || ! -O "${state_dir}" || \
    "$(stat -f '%Lp' "${state_dir}" 2>/dev/null)" != "700" ]]; then
    echo "M2 manager baseline state directory is unsafe." >&2
    exit 3
  fi
else
  mkdir -m 0700 "${state_dir}"
fi
clang -std=c11 -Wall -Wextra -Werror \
  -mmacosx-version-min=13.0 \
  -DRLX_TEST_DATA_PROFILE_MANAGER=1 \
  "-DRLX_TEST_DATA_STATE_DIR=\"${state_dir}\"" \
  "${helper_source}" \
  -o "${helper}"

status_output=""
status_code=0
set +e
status_output="$("${status_entrypoint}" --status 2>&1)"
status_code=$?
set -e
if [[ ${status_code} -ne 0 ]]; then
  echo "Unable to obtain the complete fixed-path cleanup status." >&2
  exit 3
fi

read_status_value() {
  local key="${1}"
  local count
  count="$(grep -Ec -- "^${key}=" <<<"${status_output}" || true)"
  if [[ "${count}" != "1" ]]; then
    echo "Required cleanup status key is missing or ambiguous: ${key}" >&2
    exit 3
  fi
  local line
  line="$(grep -E -- "^${key}=" <<<"${status_output}")"
  printf '%s\n' "${line#*=}"
}

require_status_value() {
  local key="${1}"
  local expected="${2}"
  local actual
  actual="$(read_status_value "${key}")"
  if [[ "${actual}" != "${expected}" ]]; then
    echo "Required cleanup status does not match: ${key}=${expected}" >&2
    exit 3
  fi
}

require_manager_stopped() {
  local code
  local pids
  set +e
  pids="$(pgrep -x radishlex_manager 2>/dev/null)"
  code=$?
  set -e
  case "${code}" in
    1)
      ;;
    0)
      echo "RadishLex manager is still running; refusing test-data capture or deletion." >&2
      exit 4
      ;;
    *)
      echo "RadishLex manager process state is unavailable." >&2
      exit 4
      ;;
  esac
  if [[ -n "${pids}" ]]; then
    echo "RadishLex manager process inspection returned an inconsistent result." >&2
    exit 4
  fi
}

require_status_value "matches" "0 enabled=0 selected=0"
require_status_value "installed_bundle" "absent"
require_status_value "cleanup_path_ancestors" "safe"
require_status_value "application_support_parent" "present"
require_status_value "runtime_data" "absent"
require_status_value "process" "stopped"
require_manager_stopped

parent_kind="$(read_status_value "application_support_parent_kind")"
parent_mode="$(read_status_value "application_support_parent_mode")"

if [[ "${action}" == "--capture-baseline" ]]; then
  require_status_value "application_support_parent_kind" "empty_directory"
  require_status_value "application_support_parent_mode" "755"
  require_status_value "userdb" "absent"
  require_status_value "userdb_sidecars" "absent"
  for path in "${test_data_files[@]:4}"; do
    if [[ -e "${path}" || -L "${path}" ]]; then
      echo "M2 manager settings baseline is not empty." >&2
      exit 3
    fi
  done
  "${helper}" --capture-baseline
  exit 0
fi

case "${parent_kind}:${parent_mode}" in
  nonempty_directory:700|empty_directory:700|empty_directory:755)
    ;;
  *)
    echo "Cleanup status is outside normal and recoverable M2 manager states." >&2
    exit 3
    ;;
esac

for path in "${test_data_files[@]}"; do
  if [[ ! -e "${path}" && ! -L "${path}" ]]; then
    continue
  fi
  lsof_output=""
  lsof_code=0
  set +e
  lsof_output="$(lsof -n -P -- "${path}" 2>&1)"
  lsof_code=$?
  set -e
  if [[ ${lsof_code} -eq 0 ]]; then
    echo "An M2 manager test-data file still has an open handle; refusing deletion." >&2
    exit 4
  fi
  if [[ ${lsof_code} -ne 1 || -n "${lsof_output}" ]]; then
    echo "An M2 manager test-data file could not be proven closed by lsof." >&2
    exit 4
  fi
done

"${helper}" --authorized-delete-m2-manager-test-data
