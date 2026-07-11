#!/usr/bin/env bash
set -euo pipefail

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
repo_root="$(CDPATH= cd -- "${script_dir}/.." && pwd)"
platform_dir="${repo_root}/platforms/macos-imk"

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "macOS is required for the native InputMethodKit bundle check." >&2
  exit 2
fi

: "${RIME_INCLUDE_DIR:?native check requires RIME_INCLUDE_DIR}"
: "${RIME_LIB_DIR:?native check requires RIME_LIB_DIR}"
: "${RADISHLEX_RIME_SHARED_DATA:?native check requires isolated RADISHLEX_RIME_SHARED_DATA}"
: "${RADISHLEX_RIME_SCHEMA:?native check requires RADISHLEX_RIME_SCHEMA}"
: "${RADISHLEX_RIME_DATA_LICENSE:?native check requires RADISHLEX_RIME_DATA_LICENSE}"

shared_data="$(CDPATH= cd -- "${RADISHLEX_RIME_SHARED_DATA}" && pwd -P)"
case "${shared_data}" in
  "${HOME}/Library/Rime"|"${HOME}/Library/Rime/"*|\
  "${HOME}/Library/Input Methods"|"${HOME}/Library/Input Methods/"*|\
  "${HOME}/Library/Application Support/Squirrel"|\
  "${HOME}/Library/Application Support/Squirrel/"*|\
  "${HOME}/Library/Application Support/RadishLex/Rime"|\
  "${HOME}/Library/Application Support/RadishLex/Rime/"*)
    echo "native check refuses a real or runtime user data directory: ${shared_data}" >&2
    exit 2
    ;;
esac

"${platform_dir}/build-bundle.sh" native

bundle="${repo_root}/target/macos-imk/native/RadishLex.inputmethod"
contents="${bundle}/Contents"
executable="${contents}/MacOS/RadishLex"
ffi_dylib="${contents}/Frameworks/libradishlex_ime_ffi.dylib"
resources="${contents}/Resources"
manifest="${resources}/RimeData.manifest.plist"
architecture="$(uname -m)"
deploy_on_start="${RADISHLEX_RIME_DEPLOY_ON_START:-1}"

check_dependencies() {
  local binary="$1"
  local dependency
  while IFS= read -r dependency; do
    case "${dependency}" in
      /System/*|/usr/lib/*)
        ;;
      @rpath/*)
        local name="${dependency#@rpath/}"
        if [[ ! -f "${contents}/Frameworks/${name}" && ! -f "${RIME_LIB_DIR}/${name}" ]]; then
          echo "unresolved @rpath dependency for ${binary}: ${dependency}" >&2
          return 1
        fi
        ;;
      @loader_path/*)
        local loader_relative="${dependency#@loader_path/}"
        if [[ ! -f "$(dirname -- "${binary}")/${loader_relative}" ]]; then
          echo "unresolved @loader_path dependency for ${binary}: ${dependency}" >&2
          return 1
        fi
        ;;
      @executable_path/*)
        local executable_relative="${dependency#@executable_path/}"
        if [[ ! -f "${contents}/MacOS/${executable_relative}" ]]; then
          echo "unresolved @executable_path dependency for ${binary}: ${dependency}" >&2
          return 1
        fi
        ;;
      /*)
        if [[ ! -f "${dependency}" ]]; then
          echo "missing absolute dependency for ${binary}: ${dependency}" >&2
          return 1
        fi
        ;;
      *)
        echo "unsupported dependency path for ${binary}: ${dependency}" >&2
        return 1
        ;;
    esac
  done < <(otool -L "${binary}" | tail -n +2 | awk '{print $1}')
}

test -x "${executable}"
test -f "${ffi_dylib}"
test -f "${resources}/RimeData/default.yaml"
test -f "${resources}/RimeData/${RADISHLEX_RIME_SCHEMA}.schema.yaml"
test -s "${resources}/RimeData.LICENSE"
plutil -lint "${contents}/Info.plist" "${manifest}" >/dev/null
test "$(plutil -extract RadishLexRimeSchema raw "${contents}/Info.plist")" = \
  "${RADISHLEX_RIME_SCHEMA}"
test "$(plutil -extract schema_id raw "${manifest}")" = "${RADISHLEX_RIME_SCHEMA}"
test "$(plutil -extract RadishLexRimeDeployOnStart raw "${contents}/Info.plist")" = \
  "$([[ "${deploy_on_start}" == "1" ]] && echo true || echo false)"
test "$(plutil -extract deploy_on_start raw "${manifest}")" = \
  "$([[ "${deploy_on_start}" == "1" ]] && echo true || echo false)"

lipo -archs "${executable}" | tr ' ' '\n' | grep -qx "${architecture}"
lipo -archs "${ffi_dylib}" | tr ' ' '\n' | grep -qx "${architecture}"
otool -L "${executable}" | grep -q "@rpath/libradishlex_ime_ffi.dylib"
otool -L "${ffi_dylib}" | grep -q "librime"
otool -l "${ffi_dylib}" | grep -A2 LC_RPATH | grep -q "${RIME_LIB_DIR}"

rime_dependency="$(otool -L "${ffi_dylib}" | awk '/librime/{print $1; exit}')"
case "${rime_dependency}" in
  @rpath/*) rime_binary="${RIME_LIB_DIR}/${rime_dependency#@rpath/}" ;;
  /*) rime_binary="${rime_dependency}" ;;
  *) echo "unsupported librime dependency path: ${rime_dependency}" >&2; exit 1 ;;
esac
test -f "${rime_binary}"
check_dependencies "${executable}"
check_dependencies "${ffi_dylib}"
check_dependencies "${rime_binary}"

for symbol in session_new_rime session_handle_key_event rime_runtime_shutdown; do
  nm -gU "${ffi_dylib}" | grep -q "_radishlex_${symbol}$"
done

python3 "${repo_root}/scripts/macos-imk/native_manifest.py" verify \
  --data-dir "${resources}/RimeData" \
  --license "${resources}/RimeData.LICENSE" \
  --schema "${RADISHLEX_RIME_SCHEMA}" \
  --deploy-on-start "${deploy_on_start}" \
  --manifest "${manifest}"

echo "Native macOS InputMethodKit bundle checks passed without installation."
