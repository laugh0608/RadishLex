#!/usr/bin/env bash
set -euo pipefail

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
repo_root="$(CDPATH= cd -- "${script_dir}/.." && pwd)"
adapter_dir="${repo_root}/platforms/macos-product/UpgradeCoordinatorAdapter"
product_tool="${repo_root}/scripts/macos-product/product_manifest.py"
rime_data_tool="${repo_root}/scripts/rime-product/product_data.py"
manager_bundle="${repo_root}/apps/radishlex-manager/build/macos/Build/Products/Release/radishlex_manager.app"
input_method_bundle="${repo_root}/target/macos-imk/native/RadishLexInputMethod.app"
source_metadata="${adapter_dir}/fixtures/source-product.json"
marker_source="${adapter_dir}/fixtures/qualification.marker"
qualification_root="$(mktemp -d "${TMPDIR:-/tmp}/radishlex-upgrade-product-qualification.XXXXXX")"
source_product="${qualification_root}/source-product"
target_product="${qualification_root}/target-product"
rime_data="${qualification_root}/RimeData"

cleanup() {
  rm -rf "${qualification_root}"
}
trap cleanup EXIT

if [[ $# -ne 0 ]]; then
  echo "macOS upgrade product coordination check does not accept arguments" >&2
  exit 2
fi
if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "macOS is required for the upgrade product coordination check" >&2
  exit 2
fi

chmod 700 "${qualification_root}"
install -m 600 "${marker_source}" \
  "${qualification_root}/radishlex-upgrade-qualification.marker"

"${repo_root}/scripts/build-manager-macos-product.sh"
if [[ -n "${RIME_INCLUDE_DIR:-}" && -n "${RIME_LIB_DIR:-}" ]]; then
  rime_include_dir="${RIME_INCLUDE_DIR}"
  rime_lib_dir="${RIME_LIB_DIR}"
elif command -v brew >/dev/null 2>&1; then
  rime_prefix="$(brew --prefix librime)"
  rime_include_dir="${rime_prefix}/include"
  rime_lib_dir="${rime_prefix}/lib"
else
  echo "RIME_INCLUDE_DIR and RIME_LIB_DIR are required without Homebrew librime" >&2
  exit 1
fi
if [[ ! -f "${rime_include_dir}/rime_api.h" || ! -d "${rime_lib_dir}" ]]; then
  echo "librime headers or libraries are unavailable for native product build" >&2
  exit 1
fi
python3 "${rime_data_tool}" assemble --output "${rime_data}"
rime_schema="$(python3 "${rime_data_tool}" field schema_id)"
env \
  RIME_INCLUDE_DIR="${rime_include_dir}" \
  RIME_LIB_DIR="${rime_lib_dir}" \
  RADISHLEX_RIME_SHARED_DATA="${rime_data}" \
  RADISHLEX_RIME_SCHEMA="${rime_schema}" \
  "${repo_root}/platforms/macos-imk/build-bundle.sh" native

codesign --verify --deep --strict "${manager_bundle}"
codesign --verify --deep --strict "${input_method_bundle}"

install -d -m 700 "${source_product}/Components" "${target_product}/Components"
ditto "${manager_bundle}" "${target_product}/Components/radishlex_manager.app"
ditto "${input_method_bundle}" \
  "${target_product}/Components/RadishLexInputMethod.app"
install -m 644 "${repo_root}/LICENSE" "${target_product}/LICENSE"
python3 "${product_tool}" create \
  --manager-bundle "${target_product}/Components/radishlex_manager.app" \
  --input-method-bundle "${target_product}/Components/RadishLexInputMethod.app" \
  --license "${target_product}/LICENSE" \
  --manifest "${target_product}/ProductManifest.json"
python3 "${product_tool}" verify \
  --manager-bundle "${target_product}/Components/radishlex_manager.app" \
  --input-method-bundle "${target_product}/Components/RadishLexInputMethod.app" \
  --license "${target_product}/LICENSE" \
  --manifest "${target_product}/ProductManifest.json"

ditto "${manager_bundle}" "${source_product}/Components/radishlex_manager.app"
ditto "${input_method_bundle}" \
  "${source_product}/Components/RadishLexInputMethod.app"
install -m 644 "${repo_root}/LICENSE" "${source_product}/LICENSE"
for bundle in \
  "${source_product}/Components/radishlex_manager.app" \
  "${source_product}/Components/RadishLexInputMethod.app"; do
  plutil -replace CFBundleShortVersionString -string 0.0.9 \
    "${bundle}/Contents/Info.plist"
  plutil -replace CFBundleVersion -string 34 "${bundle}/Contents/Info.plist"
  codesign --force --deep --sign - --timestamp=none "${bundle}"
  codesign --verify --deep --strict "${bundle}"
done
python3 "${product_tool}" create \
  --metadata "${source_metadata}" \
  --manager-bundle "${source_product}/Components/radishlex_manager.app" \
  --input-method-bundle "${source_product}/Components/RadishLexInputMethod.app" \
  --license "${source_product}/LICENSE" \
  --manifest "${source_product}/ProductManifest.json"
python3 "${product_tool}" verify \
  --metadata "${source_metadata}" \
  --manager-bundle "${source_product}/Components/radishlex_manager.app" \
  --input-method-bundle "${source_product}/Components/RadishLexInputMethod.app" \
  --license "${source_product}/LICENSE" \
  --manifest "${source_product}/ProductManifest.json"

chmod 700 "${source_product}" "${target_product}"
(
  cd "${repo_root}"
  env \
    RADISHLEX_UPGRADE_QUALIFICATION_ROOT="${qualification_root}" \
    RADISHLEX_UPGRADE_QUALIFICATION_SOURCE_PRODUCT="${source_product}" \
    RADISHLEX_UPGRADE_QUALIFICATION_TARGET_PRODUCT="${target_product}" \
    cargo test --locked -p radishlex-macos-upgrade-coordinator \
      --features qualification-harness \
      --test product_coordination \
      -- --nocapture
)

echo "macOS upgrade product coordination qualification passed."
