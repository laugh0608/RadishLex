#!/usr/bin/env bash
set -euo pipefail

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
repo_root="$(CDPATH= cd -- "${script_dir}/../.." && pwd)"
mode="${1:-contract}"
product_tool="${repo_root}/scripts/macos-product/product_manifest.py"
rime_data_tool="${repo_root}/scripts/rime-product/product_data.py"
cd "${repo_root}"

python3 "${product_tool}" validate-source
product_version="$(python3 "${product_tool}" field product_version)"
product_build="$(python3 "${product_tool}" field build_number)"

case "${mode}" in
  contract)
    cargo_profile="debug"
    schema="contract.demo"
    compile_mode=1
    cargo build -p radishlex-ime-ffi
    ;;
  native)
    : "${RIME_INCLUDE_DIR:?native bundle requires RIME_INCLUDE_DIR}"
    : "${RIME_LIB_DIR:?native bundle requires RIME_LIB_DIR}"
    : "${RADISHLEX_RIME_SHARED_DATA:?native bundle requires isolated RADISHLEX_RIME_SHARED_DATA}"
    expected_schema="$(python3 "${rime_data_tool}" field schema_id)"
    schema="${RADISHLEX_RIME_SCHEMA:-${expected_schema}}"
    if [[ "${schema}" != "${expected_schema}" ]]; then
      echo "native bundle schema must match pinned product RimeData: ${expected_schema}." >&2
      exit 2
    fi
    if [[ ! -d "${RADISHLEX_RIME_SHARED_DATA}" ]]; then
      echo "RADISHLEX_RIME_SHARED_DATA must be an existing isolated directory." >&2
      exit 2
    fi
    if [[ ! "${schema}" =~ ^[A-Za-z0-9._-]+$ ]]; then
      echo "Rime schema id must contain only ASCII letters, digits, dot, underscore or hyphen." >&2
      exit 2
    fi
    shared_data="$(CDPATH= cd -- "${RADISHLEX_RIME_SHARED_DATA}" && pwd -P)"
    case "${shared_data}" in
      "${HOME}/Library/Rime"|"${HOME}/Library/Rime/"*|\
      "${HOME}/Library/Input Methods"|"${HOME}/Library/Input Methods/"*|\
      "${HOME}/Library/Application Support/Squirrel"|\
      "${HOME}/Library/Application Support/Squirrel/"*|\
      "${HOME}/Library/Application Support/RadishLex/Rime"|\
      "${HOME}/Library/Application Support/RadishLex/Rime/"*)
        echo "native bundle refuses a real or runtime user data directory: ${shared_data}" >&2
        exit 2
        ;;
    esac
    if [[ ! -f "${shared_data}/${schema}.schema.yaml" ]]; then
      echo "native bundle shared data must contain ${schema}.schema.yaml." >&2
      exit 2
    fi
    python3 "${rime_data_tool}" verify --data-dir "${shared_data}"
    deploy_on_start="${RADISHLEX_RIME_DEPLOY_ON_START:-1}"
    if [[ "${deploy_on_start}" != "0" && "${deploy_on_start}" != "1" ]]; then
      echo "RADISHLEX_RIME_DEPLOY_ON_START must be 0 or 1." >&2
      exit 2
    fi
    cargo_profile="release"
    compile_mode=0
    cargo build -p radishlex-ime-ffi --features native-rime --release
    ;;
  *)
    echo "usage: $0 [contract|native]" >&2
    exit 2
    ;;
esac

if [[ ! "${schema}" =~ ^[A-Za-z0-9._-]+$ ]]; then
  echo "Rime schema id must contain only ASCII letters, digits, dot, underscore or hyphen." >&2
  exit 2
fi
build_root="${repo_root}/target/macos-imk/${mode}"
bundle="${build_root}/RadishLexInputMethod.app"
contents="${bundle}/Contents"
macos_dir="${contents}/MacOS"
frameworks_dir="${contents}/Frameworks"
resources_dir="${contents}/Resources"
native_licenses_dir="${resources_dir}/NativeLicenses"
codesign_identity="${RADISHLEX_CODESIGN_IDENTITY:--}"
export CLANG_MODULE_CACHE_PATH="${repo_root}/target/macos-imk/clang-module-cache"
export SWIFT_MODULECACHE_PATH="${repo_root}/target/macos-imk/swift-module-cache"

rm -rf "${bundle}"
mkdir -p "${macos_dir}" "${frameworks_dir}" "${resources_dir}" \
  "${CLANG_MODULE_CACHE_PATH}" "${SWIFT_MODULECACHE_PATH}"

ffi_dylib="${repo_root}/target/${cargo_profile}/libradishlex_ime_ffi.dylib"
cp "${ffi_dylib}" "${frameworks_dir}/"
install_name_tool -id "@rpath/libradishlex_ime_ffi.dylib" \
  "${frameworks_dir}/libradishlex_ime_ffi.dylib"
if [[ "${mode}" == "native" ]]; then
  python3 "${repo_root}/scripts/macos-imk/bundle_dylibs.py" bundle \
    --root-binary "${frameworks_dir}/libradishlex_ime_ffi.dylib" \
    --frameworks-dir "${frameworks_dir}" \
    --licenses-dir "${native_licenses_dir}" \
    --search-dir "${RIME_LIB_DIR}"
fi

clang -fobjc-arc -fmodules -Wall -Wextra -Werror \
  -mmacosx-version-min=13.0 \
  -DRADISHLEX_CONTRACT_SMOKE="${compile_mode}" \
  -I"${script_dir}/Sources" \
  -I"${repo_root}/crates/ime-ffi/include" \
  "${script_dir}/Sources/RadishLexBridge.m" \
  "${script_dir}/Sources/RadishLexCandidatePanel.m" \
  "${script_dir}/Sources/RadishLexLearningContext.m" \
  "${script_dir}/Sources/RadishLexRuntime.m" \
  "${script_dir}/Sources/RadishLexInputController.m" \
  "${script_dir}/Sources/main.m" \
  -L"${frameworks_dir}" -lradishlex_ime_ffi \
  -Wl,-rpath,@executable_path/../Frameworks \
  -framework Cocoa -framework Carbon -framework InputMethodKit \
  -o "${macos_dir}/RadishLex"

sed \
  -e "s/__RADISHLEX_RIME_SCHEMA__/${schema}/g" \
  -e "s/__RADISHLEX_PRODUCT_VERSION__/${product_version}/g" \
  -e "s/__RADISHLEX_PRODUCT_BUILD__/${product_build}/g" \
  "${script_dir}/Resources/Info.plist.in" >"${contents}/Info.plist"
xcrun swift "${repo_root}/scripts/macos-imk/render_icon.swift" \
  "${script_dir}/Resources/RadishLexInputIcon.svg" \
  "${resources_dir}/RadishLexInputIcon.tiff"
ditto "${script_dir}/Resources/zh-Hans.lproj" "${resources_dir}/zh-Hans.lproj"
ditto "${script_dir}/Resources/en.lproj" "${resources_dir}/en.lproj"
plutil -lint "${contents}/Info.plist" >/dev/null

if [[ "${mode}" == "native" ]]; then
  ditto "${shared_data}" "${resources_dir}/RimeData"
  cp "${repo_root}/LICENSE" "${resources_dir}/RadishLex.LICENSE"
  if [[ "${deploy_on_start}" == "1" ]]; then
    plutil -replace RadishLexRimeDeployOnStart -bool true "${contents}/Info.plist"
  fi
  manifest="${resources_dir}/RimeData.manifest.plist"
  python3 "${repo_root}/scripts/macos-imk/native_manifest.py" create \
    --data-dir "${resources_dir}/RimeData" \
    --schema "${schema}" \
    --deploy-on-start "${deploy_on_start}" \
    --output "${manifest}"
fi

# Sign every bundled native library before sealing the main executable and
# outer bundle. The development default is ad-hoc; callers may provide a named
# identity without changing assembly or verification.
while IFS= read -r -d '' dylib; do
  codesign --force --sign "${codesign_identity}" --timestamp=none "${dylib}"
done < <(find "${frameworks_dir}" -type f -name '*.dylib' -print0 | sort -z)
codesign --force --sign "${codesign_identity}" --timestamp=none \
  "${macos_dir}/RadishLex"
if [[ "${mode}" == "native" ]]; then
  python3 "${repo_root}/scripts/macos-imk/bundle_dylibs.py" manifest \
    --frameworks-dir "${frameworks_dir}" \
    --licenses-dir "${native_licenses_dir}" \
    --output "${resources_dir}/NativeLibraries.manifest.plist"
fi
codesign --force --sign "${codesign_identity}" --timestamp=none "${bundle}"
codesign --verify --deep --strict --verbose=2 "${bundle}"

echo "Built ${bundle}"
