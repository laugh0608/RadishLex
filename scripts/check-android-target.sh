#!/usr/bin/env bash
set -euo pipefail

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
repo_root="$(CDPATH= cd -- "${script_dir}/.." && pwd)"

python_exe="${PYTHON:-python3}"
if command -v "${python_exe}" >/dev/null 2>&1; then
  exec "${python_exe}" "${repo_root}/scripts/check-android-target.py" "$@"
fi

if command -v python >/dev/null 2>&1; then
  exec python "${repo_root}/scripts/check-android-target.py" "$@"
fi

echo "python is required to run Android target checks." >&2
exit 1
