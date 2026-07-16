#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 1 || "${1}" != "--authorized-stop-process" ]]; then
  echo "usage: $0 --authorized-stop-process" >&2
  echo "This entrypoint only stops the exactly verified RadishLex input method process." >&2
  exit 2
fi

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
repo_root="$(CDPATH= cd -- "${script_dir}/.." && pwd)"

exec "${repo_root}/platforms/macos-imk/cleanup-user-install.sh" \
  --authorized-stop-process
