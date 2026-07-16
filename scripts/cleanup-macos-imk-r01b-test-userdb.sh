#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 1 || \
  ("${1}" != "--capture-baseline" && \
    "${1}" != "--authorized-delete-r01b-test-userdb") ]]; then
  echo "usage: $0 --capture-baseline|--authorized-delete-r01b-test-userdb" >&2
  echo "This entrypoint never accepts a caller-provided path." >&2
  exit 2
fi

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
repo_root="$(CDPATH= cd -- "${script_dir}/.." && pwd)"

exec "${repo_root}/platforms/macos-imk/cleanup-r01b-test-userdb.sh" "$1"
