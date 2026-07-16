#!/usr/bin/env bash
set -euo pipefail

case "${1:-}" in
  --capture-baseline|--authorized-delete-r01b-test-userdb)
    action="${1}"
    ;;
  *)
    echo "usage: $0 --capture-baseline|--authorized-delete-r01b-test-userdb" >&2
    echo "Deletion is restricted to the fixed R01B test userdb family and requires explicit authorization." >&2
    exit 2
    ;;
esac
if [[ $# -ne 1 ]]; then
  echo "R01B userdb cleanup does not accept caller-provided paths or extra arguments." >&2
  exit 2
fi

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
repo_root="$(CDPATH= cd -- "${script_dir}/../.." && pwd)"
tool_dir="${repo_root}/target/macos-imk/tools"
state_dir="${repo_root}/target/macos-imk/r01b"
helper_source="${script_dir}/Tools/r01b_test_userdb_cleanup.c"
helper="${tool_dir}/r01b-test-userdb-cleanup"
status_entrypoint="${script_dir}/cleanup-user-install.sh"
application_support_parent="${HOME}/Library/Application Support/RadishLex"
userdb="${application_support_parent}/userdb.sqlite3"
userdb_files=(
  "${userdb}"
  "${userdb}-wal"
  "${userdb}-shm"
  "${userdb}-journal"
)

mkdir -p "${tool_dir}" "${state_dir}"
clang -std=c11 -Wall -Wextra -Werror \
  -mmacosx-version-min=13.0 \
  "-DRLX_R01B_STATE_DIR=\"${state_dir}\"" \
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

require_status_value "matches" "0 enabled=0 selected=0"
require_status_value "installed_bundle" "absent"
require_status_value "cleanup_path_ancestors" "safe"
require_status_value "application_support_parent" "present"
require_status_value "runtime_data" "absent"
require_status_value "process" "stopped"

if [[ "${action}" == "--capture-baseline" ]]; then
  require_status_value "application_support_parent_kind" "empty_directory"
  require_status_value "application_support_parent_mode" "755"
  require_status_value "userdb" "absent"
  require_status_value "userdb_sidecars" "absent"
  "${helper}" --capture-baseline
  exit 0
fi

parent_kind="$(read_status_value "application_support_parent_kind")"
parent_mode="$(read_status_value "application_support_parent_mode")"
userdb_status="$(read_status_value "userdb")"
sidecar_status="$(read_status_value "userdb_sidecars")"
delete_state="${parent_kind}:${parent_mode}:${userdb_status}:${sidecar_status}"
case "${delete_state}" in
  nonempty_directory:700:present:present|\
  nonempty_directory:700:present:absent|\
  empty_directory:700:absent:absent|\
  empty_directory:755:absent:absent)
    ;;
  *)
    echo "Cleanup status is outside normal and recoverable R01B userdb states." >&2
    exit 3
    ;;
esac

for path in "${userdb_files[@]}"; do
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
    echo "An R01B userdb file still has an open handle; refusing deletion." >&2
    exit 4
  fi
  if [[ ${lsof_code} -ne 1 || -n "${lsof_output}" ]]; then
    echo "An R01B userdb file could not be proven closed by lsof." >&2
    exit 4
  fi
done

"${helper}" --authorized-delete-r01b-test-userdb
