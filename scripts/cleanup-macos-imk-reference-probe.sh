#!/usr/bin/env bash
set -euo pipefail

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
repo_root="$(CDPATH= cd -- "${script_dir}/.." && pwd)"
exec "${repo_root}/platforms/macos-imk/ReferenceProbe/cleanup-user-install.sh" "$@"
