#!/usr/bin/env bash
set -euo pipefail

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
# Keep standalone and repository phase commands in one reviewed inventory.
exec python3 "${script_dir}/validation_suites.py" product-install-core "$@"
