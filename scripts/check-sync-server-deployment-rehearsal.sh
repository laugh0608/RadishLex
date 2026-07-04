#!/usr/bin/env bash
set -euo pipefail

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
repo_root="$(CDPATH= cd -- "${script_dir}/.." && pwd)"

python_exe="${PYTHON:-python3}"
if command -v "${python_exe}" >/dev/null 2>&1; then
  exec "${python_exe}" "${repo_root}/scripts/check-sync-server-deployment-rehearsal.py" "$@"
fi

if command -v python >/dev/null 2>&1; then
  exec python "${repo_root}/scripts/check-sync-server-deployment-rehearsal.py" "$@"
fi

echo "python is required to run sync server deployment rehearsal." >&2
exit 1
