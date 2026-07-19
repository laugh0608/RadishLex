#!/usr/bin/env bash
set -euo pipefail

if [ "$(uname -s)" != "Darwin" ]; then
  echo "manager native library embedding is supported only on macOS." >&2
  exit 1
fi

if [ -z "${PROJECT_DIR:-}" ] || [ -z "${TARGET_BUILD_DIR:-}" ] || [ -z "${FRAMEWORKS_FOLDER_PATH:-}" ]; then
  echo "manager native library embedding must run from the macOS Xcode build." >&2
  exit 1
fi

repo_root="$(CDPATH= cd -- "${PROJECT_DIR}/../../.." && pwd)"
target_dir="${RADISHLEX_CARGO_TARGET_DIR:-${repo_root}/target}"
profile="debug"
cargo_args=(build --locked -p radishlex-ime-ffi)
if [ "$(uname -s)" = "Darwin" ]; then
  cargo_args+=(--features apple-keychain)
fi
if [ "${CONFIGURATION:-Debug}" != "Debug" ]; then
  profile="release"
  cargo_args+=(--release)
fi

(
  cd "${repo_root}"
  CARGO_TARGET_DIR="${target_dir}" cargo "${cargo_args[@]}"
)

source_library="${target_dir}/${profile}/libradishlex_ime_ffi.dylib"
frameworks_dir="${TARGET_BUILD_DIR}/${FRAMEWORKS_FOLDER_PATH}"
bundled_library="${frameworks_dir}/libradishlex_ime_ffi.dylib"

if [ ! -f "${source_library}" ]; then
  echo "RadishLex manager native library is missing: ${source_library}" >&2
  exit 1
fi

install -d -m 755 "${frameworks_dir}"
install -m 755 "${source_library}" "${bundled_library}"
install_name_tool -id "@rpath/libradishlex_ime_ffi.dylib" "${bundled_library}"

required_symbols=(
  _radishlex_apple_p256_product_smoke
  _radishlex_apple_p256_product_status
  _radishlex_apple_secure_enclave_key_agreement_product_smoke
  _radishlex_apple_secure_enclave_key_agreement_product_status
  _radishlex_ffi_contract
  _radishlex_manager_sync_product_status
  _radishlex_userdb_terms_new
  _radishlex_userdb_deleted_terms_new
  _radishlex_userdb_delete_term
  _radishlex_userdb_restore_term
  _radishlex_userdb_dictionary_import
  _radishlex_userdb_dictionary_export
  _radishlex_userdb_learning_status
  _radishlex_userdb_sync_preflight
  _radishlex_userdb_rank_explain_new
)
exported_symbols="$(nm -gU "${bundled_library}")"
for symbol in "${required_symbols[@]}"; do
  if ! grep -Eq "(^|[[:space:]])${symbol}$" <<<"${exported_symbols}"; then
    echo "bundled manager native library is missing required symbol: ${symbol}" >&2
    exit 1
  fi
done

expected_arch="${CURRENT_ARCH:-$(uname -m)}"
if [ "${expected_arch}" = "undefined_arch" ]; then
  expected_arch="$(uname -m)"
fi
bundled_arches="$(lipo -archs "${bundled_library}")"
if ! grep -Eq "(^|[[:space:]])${expected_arch}([[:space:]]|$)" <<<"${bundled_arches}"; then
  echo "bundled manager native library does not contain ${expected_arch}: ${bundled_arches}" >&2
  exit 1
fi

install_id="$(otool -D "${bundled_library}" | tail -n 1)"
if [ "${install_id}" != "@rpath/libradishlex_ime_ffi.dylib" ]; then
  echo "bundled manager native library has an unexpected install name: ${install_id}" >&2
  exit 1
fi

if otool -L "${bundled_library}" | tail -n +2 | grep -Fq "${target_dir}"; then
  echo "bundled manager native library retains a build-directory dependency." >&2
  exit 1
fi

codesign_identity="${EXPANDED_CODE_SIGN_IDENTITY:--}"
codesign --force --sign "${codesign_identity}" --timestamp=none "${bundled_library}"
