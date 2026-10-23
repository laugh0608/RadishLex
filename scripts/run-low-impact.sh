#!/usr/bin/env bash
set -euo pipefail

if [[ $# -eq 0 ]]; then
  echo "Usage: $0 command [argument ...]" >&2
  exit 2
fi

# Bound ordinary tool concurrency; child processes inherit this environment.
# Tool-specific flags can still override it. This is not a CPU or RSS quota.
export CARGO_BUILD_JOBS=1
export RUST_TEST_THREADS=1
export GOMAXPROCS=1

# Keep CPU courtesy without Darwin background disk/network throttling.
# Apply only to this command and descendants; never change existing processes.
exec nice -n 19 "$@"
