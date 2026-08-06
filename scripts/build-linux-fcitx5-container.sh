#!/usr/bin/env bash
set -euo pipefail

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
repo_root="$(CDPATH= cd -- "${script_dir}/.." && pwd)"
dockerfile="${repo_root}/platforms/linux-fcitx5/dev/Dockerfile"
image="radishlex-linux-fcitx5-dev:debian13-arm64"

if [[ "$(uname -m)" != "arm64" ]]; then
  echo "This development baseline currently targets an ARM64 host." >&2
  exit 1
fi
if ! command -v docker >/dev/null 2>&1; then
  echo "Docker is required for the Linux Fcitx5 development build." >&2
  exit 1
fi
if ! docker info >/dev/null 2>&1; then
  echo "Docker Desktop is not running or is unavailable." >&2
  exit 1
fi

docker build \
  --platform linux/arm64 \
  --file "${dockerfile}" \
  --tag "${image}" \
  "${repo_root}/platforms/linux-fcitx5/dev"

docker run --rm \
  --platform linux/arm64 \
  --mount "type=bind,source=${repo_root},target=/workspace/RadishLex,readonly" \
  --mount "type=volume,source=radishlex-linux-fcitx5-cargo,target=/tmp/radishlex-cargo-home" \
  --mount "type=volume,source=radishlex-linux-fcitx5-target,target=/tmp/radishlex-target" \
  --workdir /workspace/RadishLex \
  "${image}" \
  bash -lc '
    set -euo pipefail
    test "$(uname -s)" = "Linux"
    test "$(uname -m)" = "aarch64"
    rustc --version
    cargo --version
    cmake --version | head -1
    dpkg-query -W -f="\${Package} \${Version}\n" \
      libfcitx5core-dev librime-dev
    [[ "$(rustc --version)" == rustc\ 1.85.0\ * ]]
    [[ "$(cargo --version)" == cargo\ 1.85.0\ * ]]
    test "$(cmake --version | head -1)" = "cmake version 3.31.6"
    test "$(dpkg-query -W -f="\${Version}" libfcitx5core-dev)" = "5.1.12-2"
    test "$(dpkg-query -W -f="\${Version}" librime-dev)" = "1.13.1+dfsg1-1"
    cargo build --locked \
      -p radishlex-ime-ffi \
      --release \
      --features native-rime
    ffi_library="${CARGO_TARGET_DIR}/release/libradishlex_ime_ffi.so"
    test -f "${ffi_library}"
    file "${ffi_library}"
    ldd "${ffi_library}"
    RADISHLEX_IME_FFI_LIBRARY="${ffi_library}" \
      ./scripts/check-linux-fcitx5.sh --require-fcitx
    ./scripts/check-linux-product-layout.sh
    ./scripts/build-linux-product-addon-stage.sh \
      --ffi-library "${ffi_library}" \
      --output /tmp/radishlex-product-addon-stage
  '
