#!/usr/bin/env bash
set -euo pipefail

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
repo_root="$(CDPATH= cd -- "${script_dir}/.." && pwd -P)"
if [[ $# -ne 0 ]]; then
  echo "source-first repository check does not accept arguments" >&2
  exit 2
fi

# This new root and all case/failure materials are retained, including on timeout.
evidence_root="$(mktemp -d "${TMPDIR:-/tmp}/radishlex-source-first-v12.XXXXXX")"
evidence_root="$(CDPATH= cd -- "${evidence_root}" && pwd -P)"
echo "Source-first V1/V2 evidence: ${evidence_root}"
PYTHONDONTWRITEBYTECODE=1 python3 \
  "${repo_root}/scripts/linux-product/source_first.py" validate-candidate
PYTHONDONTWRITEBYTECODE=1 python3 \
  "${repo_root}/scripts/linux-product/test_source_first.py" \
  --test-root "${evidence_root}/cases" >"${evidence_root}/tests.log" 2>&1
cat "${evidence_root}/tests.log"
echo "Source-first V1/V2 synthetic admission passed; no build, VM or installation."
