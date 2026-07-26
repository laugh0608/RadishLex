#!/usr/bin/env bash
set -euo pipefail

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
repo_root="$(CDPATH= cd -- "${script_dir}/.." && pwd)"
product_tool="${repo_root}/scripts/macos-product/product_manifest.py"
rime_data_tool="${repo_root}/scripts/rime-product/product_data.py"

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "RadishLex macOS product assembly requires macOS." >&2
  exit 1
fi

python3 "${product_tool}" validate-source
python3 "${rime_data_tool}" validate
product_version="$(python3 "${product_tool}" field product_version)"
product_build="$(python3 "${product_tool}" field build_number)"
schema="$(python3 "${rime_data_tool}" field schema_id)"
assembly_parent="${repo_root}/target/macos-product"
assembly="${assembly_parent}/${product_version}-${product_build}"

if [[ -e "${assembly}" || -L "${assembly}" ]]; then
  echo "product assembly already exists: ${assembly}" >&2
  exit 2
fi

mkdir -p "${assembly_parent}"
rime_work="$(mktemp -d "${assembly_parent}/.rime-input.XXXXXX")"
rime_data="${rime_work}/RimeData"
staging=""
cleanup_work() {
  if [[ -n "${staging}" ]]; then
    rm -rf -- "${staging}"
  fi
  rm -rf -- "${rime_work}"
}
trap cleanup_work EXIT

python3 "${rime_data_tool}" assemble --output "${rime_data}"
"${repo_root}/scripts/build-manager-macos-product.sh"
env \
  RADISHLEX_RIME_SHARED_DATA="${rime_data}" \
  RADISHLEX_RIME_SCHEMA="${schema}" \
  "${repo_root}/platforms/macos-imk/build-bundle.sh" native

manager_bundle="${repo_root}/apps/radishlex-manager/build/macos/Build/Products/Release/radishlex_manager.app"
input_method_bundle="${repo_root}/target/macos-imk/native/RadishLexInputMethod.app"
for bundle in "${manager_bundle}" "${input_method_bundle}"; do
  if [[ ! -d "${bundle}" || -L "${bundle}" ]]; then
    echo "product component is missing or invalid: ${bundle}" >&2
    exit 1
  fi
done

staging="$(mktemp -d "${assembly_parent}/.assembly.XXXXXX")"

mkdir -p "${staging}/Components"
ditto "${manager_bundle}" "${staging}/Components/radishlex_manager.app"
ditto "${input_method_bundle}" "${staging}/Components/RadishLexInputMethod.app"
install -m 644 "${repo_root}/LICENSE" "${staging}/LICENSE"

python3 "${product_tool}" create \
  --manager-bundle "${staging}/Components/radishlex_manager.app" \
  --input-method-bundle "${staging}/Components/RadishLexInputMethod.app" \
  --license "${staging}/LICENSE" \
  --manifest "${staging}/ProductManifest.json"
python3 "${product_tool}" verify \
  --manager-bundle "${staging}/Components/radishlex_manager.app" \
  --input-method-bundle "${staging}/Components/RadishLexInputMethod.app" \
  --license "${staging}/LICENSE" \
  --manifest "${staging}/ProductManifest.json"

mv "${staging}" "${assembly}"
staging=""
rm -rf -- "${rime_work}"
trap - EXIT
echo "RadishLex macOS product assembly: ${assembly}"
