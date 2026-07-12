#!/usr/bin/env bash
set -euo pipefail

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
repo_root="$(CDPATH= cd -- "${script_dir}/../../.." && pwd)"
build_root="${repo_root}/target/macos-imk/reference-probe-mode"
bundle="${build_root}/RadishLexIMKModeReferenceProbe.app"
contents="${bundle}/Contents"
macos_dir="${contents}/MacOS"
resources_dir="${contents}/Resources"
codesign_identity="${RADISHLEX_REFERENCE_PROBE_CODESIGN_IDENTITY:--}"

export CLANG_MODULE_CACHE_PATH="${build_root}/clang-module-cache"
export SWIFT_MODULECACHE_PATH="${build_root}/swift-module-cache"

rm -rf "${bundle}"
mkdir -p "${macos_dir}" "${resources_dir}" "${CLANG_MODULE_CACHE_PATH}" \
  "${SWIFT_MODULECACHE_PATH}"

clang -fobjc-arc -fmodules -Wall -Wextra -Werror \
  -mmacosx-version-min=13.0 \
  -I"${script_dir}/Sources" \
  "${script_dir}/Sources/ReferenceProbeState.m" \
  "${script_dir}/Sources/ReferenceProbeController.m" \
  "${script_dir}/Sources/main.m" \
  -framework Cocoa -framework Carbon -framework InputMethodKit \
  -o "${macos_dir}/RadishLexIMKModeReferenceProbe"

cp "${script_dir}/Resources/Info.plist" "${contents}/Info.plist"
xcrun swift "${repo_root}/scripts/macos-imk/render_icon.swift" \
  "${script_dir}/Resources/ReferenceProbeIcon.svg" \
  "${resources_dir}/ReferenceProbeIcon.tiff"
ditto "${script_dir}/Resources/zh-Hans.lproj" "${resources_dir}/zh-Hans.lproj"
ditto "${script_dir}/Resources/en.lproj" "${resources_dir}/en.lproj"
plutil -lint "${contents}/Info.plist" >/dev/null

codesign --force --sign "${codesign_identity}" --timestamp=none \
  "${macos_dir}/RadishLexIMKModeReferenceProbe"
codesign --force --sign "${codesign_identity}" --timestamp=none "${bundle}"
codesign --verify --deep --strict --verbose=2 "${bundle}"

echo "Built ${bundle}"
