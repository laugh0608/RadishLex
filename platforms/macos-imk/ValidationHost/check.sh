#!/usr/bin/env bash
set -euo pipefail

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
repo_root="$(CDPATH= cd -- "${script_dir}/../../.." && pwd)"
platform_dir="${repo_root}/platforms/macos-imk"
build_root="${repo_root}/target/macos-imk/validation-host"
host_source="${script_dir}/Sources/main.m"
export CLANG_MODULE_CACHE_PATH="${build_root}/clang-module-cache"

if [[ "$(uname -s)" != "Darwin" ]]; then
    echo "macOS is required for the context validation host contract." >&2
    exit 2
fi

"${script_dir}/build-bundle.sh" all

contract_dir="${build_root}/contract"
mkdir -p "${contract_dir}"
clang -fobjc-arc -fmodules -Wall -Wextra -Werror \
    -mmacosx-version-min=13.0 \
    -I"${platform_dir}/Sources" \
    "${platform_dir}/Sources/RadishLexLearningContext.m" \
    "${script_dir}/Tests/learning_context_contract.m" \
    -framework Foundation \
    -o "${contract_dir}/learning-context-contract"
"${contract_dir}/learning-context-contract"

rg -Fq '[[NSTextField alloc]' "${host_source}"
rg -Fq '[[NSSecureTextField alloc]' "${host_source}"
rg -Fq 'IsSecureEventInputEnabled()' "${host_source}"
if rg -n \
    'EnableSecureEventInput[[:space:]]*\(|DisableSecureEventInput[[:space:]]*\(|NSUserDefaults|NSFileManager|NSPasteboard|NSURLSession|NSLog|os_log|writeTo(File|URL)' \
    "${host_source}"; then
    echo "validation host must not mutate secure input, persist text or log content." >&2
    exit 1
fi

unknown_bundle="${build_root}/unknown/RadishLexContextValidationHost.app"
p0_bundle="${build_root}/p0/RadishLexContextValidationHost.app"
for bundle in "${unknown_bundle}" "${p0_bundle}"; do
    plutil -lint "${bundle}/Contents/Info.plist" >/dev/null
    codesign --verify --deep --strict --verbose=2 "${bundle}"
    executable="${bundle}/Contents/MacOS/RadishLexContextValidationHost"
    nm -u "${executable}" | rg -q '_IsSecureEventInputEnabled$'
    if nm -u "${executable}" | rg -q \
        '_(EnableSecureEventInput|DisableSecureEventInput)$'; then
        echo "validation host must not mutate the process-global secure input state." >&2
        exit 1
    fi
done

test "$(plutil -extract CFBundleIdentifier raw \
    "${unknown_bundle}/Contents/Info.plist")" = \
    "org.radishlex.validation.macos.unknown"
test "$(plutil -extract RadishLexValidationContext raw \
    "${unknown_bundle}/Contents/Info.plist")" = "unknown"
test "$(plutil -extract CFBundleIdentifier raw \
    "${p0_bundle}/Contents/Info.plist")" = \
    "org.radishlex.validation.macos.p0"
test "$(plutil -extract RadishLexValidationContext raw \
    "${p0_bundle}/Contents/Info.plist")" = "p0"

echo "macOS context validation host checks passed without launching either app."
