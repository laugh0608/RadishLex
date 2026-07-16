#!/usr/bin/env bash
set -euo pipefail

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
repo_root="$(CDPATH= cd -- "${script_dir}/../../.." && pwd)"
build_root="${repo_root}/target/macos-imk/validation-host"
mode="${1:-all}"

if [[ "$(uname -s)" != "Darwin" ]]; then
    echo "macOS is required for the context validation host." >&2
    exit 2
fi

case "${mode}" in
    all|unknown|p0) ;;
    *)
        echo "usage: $0 [all|unknown|p0]" >&2
        exit 2
        ;;
esac

mkdir -p "${build_root}/clang-module-cache"
export CLANG_MODULE_CACHE_PATH="${build_root}/clang-module-cache"
staging_executable="${build_root}/RadishLexContextValidationHost"

clang -fobjc-arc -fmodules -Wall -Wextra -Werror \
    -mmacosx-version-min=13.0 \
    "${script_dir}/Sources/main.m" \
    -framework AppKit -framework Carbon \
    -o "${staging_executable}"

build_variant() {
    local variant="$1"
    local bundle_identifier="$2"
    local display_name="$3"
    local bundle="${build_root}/${variant}/RadishLexContextValidationHost.app"
    local contents="${bundle}/Contents"
    local executable="${contents}/MacOS/RadishLexContextValidationHost"

    rm -rf "${build_root:?}/${variant}"
    mkdir -p "${contents}/MacOS"
    cp "${staging_executable}" "${executable}"
    sed \
        -e "s|__BUNDLE_IDENTIFIER__|${bundle_identifier}|g" \
        -e "s|__DISPLAY_NAME__|${display_name}|g" \
        -e "s|__VALIDATION_CONTEXT__|${variant}|g" \
        "${script_dir}/Resources/Info.plist.in" >"${contents}/Info.plist"
    plutil -lint "${contents}/Info.plist" >/dev/null
    codesign --force --sign - --timestamp=none "${executable}"
    codesign --force --sign - --timestamp=none "${bundle}"
    codesign --verify --deep --strict --verbose=2 "${bundle}"
    echo "Built ${bundle}"
}

if [[ "${mode}" == "all" || "${mode}" == "unknown" ]]; then
    build_variant unknown \
        org.radishlex.validation.macos.unknown \
        "RadishLex 未知上下文验证宿主"
fi
if [[ "${mode}" == "all" || "${mode}" == "p0" ]]; then
    build_variant p0 \
        org.radishlex.validation.macos.p0 \
        "RadishLex P0 验证宿主"
fi
