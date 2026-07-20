#!/usr/bin/env bash
set -euo pipefail

case "${1:-}" in
  --status)
    action="status"
    ;;
  --monitor)
    action="monitor"
    ;;
  --authorized-stop-process)
    action="stop_process"
    ;;
  --authorized-after-settings-removal)
    action="cleanup"
    ;;
  *)
    echo "usage: $0 --status|--monitor|--authorized-stop-process|--authorized-after-settings-removal" >&2
    echo "Cleanup requires prior removal in System Settings and explicit authorization." >&2
    exit 2
    ;;
esac

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
repo_root="$(CDPATH= cd -- "${script_dir}/../.." && pwd)"
tool_dir="${repo_root}/target/macos-imk/tools"
status_tool="${tool_dir}/tis-source-status"
export CLANG_MODULE_CACHE_PATH="${tool_dir}/clang-module-cache"
bundle_id="org.radishlex.inputmethod.macos"
library_dir="${HOME}/Library"
input_methods_parent="${library_dir}/Input Methods"
application_support_root="${library_dir}/Application Support"
installed_bundle="${input_methods_parent}/RadishLexInputMethod.app"
application_support_parent="${application_support_root}/RadishLex"
runtime_data="${application_support_parent}/Rime"
userdb="${application_support_parent}/userdb.sqlite3"
process_name="RadishLex"
process_executable="${installed_bundle}/Contents/MacOS/RadishLex"
process_pattern="^$(printf '%s' "${process_executable}" | \
  sed 's/[][\\.^$*+?(){}|]/\\&/g')([[:space:]]|$)"
APPLICATION_SUPPORT_PARENT_KIND="uninspected"
APPLICATION_SUPPORT_PARENT_MODE="uninspected"
CLEANUP_PATH_ANCESTORS="uninspected"
PROCESS_STATE="uninspected"

inspect_tis() {
  local output
  local code
  set +e
  output="$("${status_tool}" "${bundle_id}" 2>&1)"
  code=$?
  set -e
  printf '%s\n' "${output}"
  if [[ ${code} -ne 0 && ${code} -ne 4 ]]; then
    echo "Unable to inspect RadishLex TIS state safely." >&2
    return "${code}"
  fi
  TIS_OUTPUT="${output}"
}

print_path_status() {
  local parent_entry

  print_presence "installed_bundle" "${installed_bundle}"
  inspect_cleanup_path_ancestors
  echo "cleanup_path_ancestors=${CLEANUP_PATH_ANCESTORS}"
  if ! path_exists "${application_support_parent}"; then
    echo "application_support_parent=absent"
    APPLICATION_SUPPORT_PARENT_KIND="absent"
    APPLICATION_SUPPORT_PARENT_MODE="not_applicable"
  elif [[ -L "${application_support_parent}" || \
    ! -d "${application_support_parent}" ]]; then
    echo "application_support_parent=present"
    APPLICATION_SUPPORT_PARENT_KIND="unsafe_type"
    APPLICATION_SUPPORT_PARENT_MODE="not_applicable"
  elif ! parent_entry="$(find "${application_support_parent}" \
    -mindepth 1 -maxdepth 1 -print -quit 2>/dev/null)" || \
    ! APPLICATION_SUPPORT_PARENT_MODE="$(stat -f '%Lp' \
      "${application_support_parent}" 2>/dev/null)"; then
    echo "application_support_parent=present"
    APPLICATION_SUPPORT_PARENT_KIND="unreadable_directory"
    APPLICATION_SUPPORT_PARENT_MODE="unavailable"
  else
    [[ -n "${parent_entry}" ]] && \
      APPLICATION_SUPPORT_PARENT_KIND="nonempty_directory" || \
      APPLICATION_SUPPORT_PARENT_KIND="empty_directory"
    echo "application_support_parent=present"
  fi
  echo "application_support_parent_kind=${APPLICATION_SUPPORT_PARENT_KIND}"
  echo "application_support_parent_mode=${APPLICATION_SUPPORT_PARENT_MODE}"
  print_presence "runtime_data" "${runtime_data}"
  print_presence "userdb" "${userdb}"
  if path_exists "${userdb}-wal" || path_exists "${userdb}-shm" || \
    path_exists "${userdb}-journal"; then
    echo "userdb_sidecars=present"
  else
    echo "userdb_sidecars=absent"
  fi
  inspect_process
  echo "process=${PROCESS_STATE}"
}

path_exists() {
  [[ -e "${1}" || -L "${1}" ]]
}

cleanup_paths_have_safe_ancestors() {
  local path

  if ! path_exists "${HOME}" || [[ -L "${HOME}" || ! -d "${HOME}" ]]; then
    return 1
  fi
  for path in "${library_dir}" "${input_methods_parent}" \
    "${application_support_root}" "${application_support_parent}"; do
    if path_exists "${path}" && [[ -L "${path}" || ! -d "${path}" ]]; then
      return 1
    fi
  done
  return 0
}

inspect_cleanup_path_ancestors() {
  if cleanup_paths_have_safe_ancestors; then
    CLEANUP_PATH_ANCESTORS="safe"
  else
    CLEANUP_PATH_ANCESTORS="unsafe"
  fi
}

require_safe_cleanup_paths() {
  if ! cleanup_paths_have_safe_ancestors; then
    echo "A RadishLex cleanup path has a symlink or non-directory ancestor." >&2
    echo "Refusing cleanup without ordinary fixed-path ancestors." >&2
    exit 10
  fi
}

print_presence() {
  local label="${1}"
  local path="${2}"

  path_exists "${path}" && echo "${label}=present" || echo "${label}=absent"
}

inspect_process() {
  local command_line
  local code
  local pid
  local pids

  set +e
  pids="$(pgrep -x "${process_name}" 2>/dev/null)"
  code=$?
  set -e
  case "${code}" in
    0)
      if [[ -z "${pids}" ]]; then
        PROCESS_STATE="unavailable"
        return
      fi
      PROCESS_STATE="running_verified"
      while IFS= read -r pid; do
        if [[ ! "${pid}" =~ ^[0-9]+$ ]] || \
          ! command_line="$(ps -ww -p "${pid}" -o command= 2>/dev/null)"; then
          PROCESS_STATE="unavailable"
          return
        fi
        command_line="${command_line#"${command_line%%[![:space:]]*}"}"
        case "${command_line}" in
          "${process_executable}"|"${process_executable} "*)
            ;;
          *)
            PROCESS_STATE="running_unverified"
            return
            ;;
        esac
      done <<<"${pids}"
      ;;
    1)
      PROCESS_STATE="stopped"
      ;;
    *)
      PROCESS_STATE="unavailable"
      ;;
  esac
}

stop_verified_process() {
  local stop_context="${1}"

  case "${PROCESS_STATE}" in
    running_verified)
      pkill -f "${process_pattern}" 2>/dev/null || true
      for _ in {1..20}; do
        inspect_process
        [[ "${PROCESS_STATE}" == "stopped" ]] && break
        [[ "${PROCESS_STATE}" != "running_verified" ]] && break
        sleep 0.1
      done
      ;;
    stopped)
      ;;
    running_unverified)
      echo "A same-name process does not match the installed RadishLex executable." >&2
      if [[ "${stop_context}" == "cleanup" ]]; then
        echo "Refusing cleanup without exact process identity." >&2
      else
        echo "Refusing process termination without exact process identity." >&2
      fi
      exit 9
      ;;
    *)
      echo "Unable to inspect the RadishLex process safely." >&2
      if [[ "${stop_context}" == "cleanup" ]]; then
        echo "Refusing cleanup while process state is unavailable." >&2
      else
        echo "Refusing process termination while process state is unavailable." >&2
      fi
      exit 8
      ;;
  esac

  if [[ "${PROCESS_STATE}" != "stopped" ]]; then
    if [[ "${stop_context}" == "cleanup" ]]; then
      echo "RadishLex process did not stop before path cleanup." >&2
    else
      echo "RadishLex process did not stop after authorized termination." >&2
    fi
    exit 6
  fi
}

if [[ "${action}" == "stop_process" ]]; then
  inspect_process
  stop_verified_process "stop_process"
  echo "RadishLex process stop verified."
  exit 0
fi

mkdir -p "${tool_dir}" "${CLANG_MODULE_CACHE_PATH}"
clang -fobjc-arc -fmodules -Wall -Wextra -Werror \
  -mmacosx-version-min=13.0 \
  "${script_dir}/Tools/tis_source_status.m" \
  -framework Carbon -framework Foundation \
  -o "${status_tool}"

if [[ "${action}" == "monitor" ]]; then
  exec "${status_tool}" --monitor
fi

inspect_tis
print_path_status
if [[ "${action}" == "status" ]]; then
  exit 0
fi

case "${APPLICATION_SUPPORT_PARENT_KIND}" in
  absent|empty_directory|nonempty_directory)
    ;;
  *)
    echo "RadishLex application support parent is not a readable ordinary directory." >&2
    echo "Refusing cleanup without a safe fixed parent path." >&2
    exit 4
    ;;
esac
require_safe_cleanup_paths

if grep -Eq '^source_id=.* selected=1 ' <<<"${TIS_OUTPUT}" ||
  grep -Eq '^source_id=.* enabled=1 selected=0 select_capable=0 ' \
    <<<"${TIS_OUTPUT}"; then
  echo "RadishLex is still selected or its non-selectable parent is enabled." >&2
  echo "Switch away and remove it in System Settings before cleanup." >&2
  exit 3
fi

stop_verified_process "cleanup"

require_safe_cleanup_paths
rm -rf "${installed_bundle}"
rm -rf "${runtime_data}"

if path_exists "${installed_bundle}" || path_exists "${runtime_data}"; then
  echo "RadishLex path cleanup is incomplete." >&2
  exit 5
fi
inspect_process
if [[ "${PROCESS_STATE}" != "stopped" ]]; then
  echo "RadishLex process state changed during path cleanup." >&2
  exit 6
fi

inspect_tis
if ! grep -q '^matches=0 enabled=0 selected=0$' <<<"${TIS_OUTPUT}"; then
  echo "TIS still reports RadishLex." >&2
  echo "Reopen System Settings, remove any returned row, and rerun cleanup." >&2
  exit 7
fi

echo "RadishLex cleanup verified: settings removal precondition, TIS, paths and process."
