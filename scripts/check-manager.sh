#!/usr/bin/env bash
set -euo pipefail

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
repo_root="$(CDPATH= cd -- "${script_dir}/.." && pwd)"
manager_dir="${repo_root}/apps/radishlex-manager"

if ! command -v dart >/dev/null 2>&1; then
  echo "dart is required to run RadishLex manager checks." >&2
  exit 1
fi

if ! command -v flutter >/dev/null 2>&1; then
  echo "flutter is required to run RadishLex manager checks." >&2
  exit 1
fi

if [ ! -d "${manager_dir}" ]; then
  echo "RadishLex manager directory is missing: ${manager_dir}" >&2
  exit 1
fi

(
  cd "${manager_dir}"
  dart format --set-exit-if-changed .
  flutter analyze
  flutter test
)
