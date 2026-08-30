#!/usr/bin/env bash
set -euo pipefail

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
repo_root="$(CDPATH= cd -- "${script_dir}/.." && pwd)"

if [[ $# -ne 0 ]]; then
  echo "Linux L6 contract check does not accept arguments" >&2
  exit 2
fi

PYTHONDONTWRITEBYTECODE=1 python3 \
  "${repo_root}/scripts/linux-product/l6_contract.py" validate
PYTHONDONTWRITEBYTECODE=1 python3 \
  "${repo_root}/scripts/linux-product/test_l6_contract.py"
PYTHONDONTWRITEBYTECODE=1 python3 \
  "${repo_root}/scripts/linux-product/l6_guest_case_contract.py" validate
PYTHONDONTWRITEBYTECODE=1 python3 \
  "${repo_root}/scripts/linux-product/test_l6_guest_case_contract.py"
PYTHONDONTWRITEBYTECODE=1 python3 \
  "${repo_root}/scripts/linux-product/l6_upgrade_quiesced_case_contract.py" validate
PYTHONDONTWRITEBYTECODE=1 python3 \
  "${repo_root}/scripts/linux-product/test_l6_upgrade_quiesced_case_contract.py"
PYTHONDONTWRITEBYTECODE=1 python3 \
  "${repo_root}/scripts/linux-product/test_l6_utm_start_once.py"
PYTHONDONTWRITEBYTECODE=1 python3 \
  "${repo_root}/scripts/linux-product/test_l6_utm_clone_once.py"
PYTHONDONTWRITEBYTECODE=1 python3 \
  "${repo_root}/scripts/linux-product/test_l6_upgrade_quiesced_registration_shell.py"
PYTHONDONTWRITEBYTECODE=1 python3 \
  "${repo_root}/scripts/linux-product/test_l6_upgrade_quiesced_clone_front_door.py"
PYTHONDONTWRITEBYTECODE=1 python3 \
  "${repo_root}/scripts/linux-product/test_l6_utm_launch_diagnostics.py"
PYTHONDONTWRITEBYTECODE=1 python3 \
  "${repo_root}/scripts/linux-product/test_l6_utm_launch_diagnostics_v5.py"
PYTHONDONTWRITEBYTECODE=1 python3 \
  "${repo_root}/scripts/linux-product/test_l6_utm_launch_diagnostics_v6.py"
PYTHONDONTWRITEBYTECODE=1 python3 \
  "${repo_root}/scripts/linux-product/test_l6_utm_launch_diagnostics_v7.py"
PYTHONDONTWRITEBYTECODE=1 python3 \
  "${repo_root}/scripts/linux-product/test_l6_utm_launch_transport_v2.py"
PYTHONDONTWRITEBYTECODE=1 python3 \
  "${repo_root}/scripts/linux-product/test_l6_utm_launch_prepared_bindings.py"
PYTHONDONTWRITEBYTECODE=1 python3 \
  "${repo_root}/scripts/linux-product/test_l6_utm_guest_network_ready.py"
PYTHONDONTWRITEBYTECODE=1 python3 \
  "${repo_root}/scripts/linux-product/test_l6_utm_canonical_input_transfer.py"
PYTHONDONTWRITEBYTECODE=1 python3 \
  "${repo_root}/scripts/linux-product/test_l6_utm_canonical_input_resolution.py"
PYTHONDONTWRITEBYTECODE=1 python3 \
  "${repo_root}/scripts/linux-product/test_l6_v4_canonical_input_resolution_probe.py"
PYTHONDONTWRITEBYTECODE=1 python3 \
  "${repo_root}/scripts/linux-product/test_l6_utm_canonical_input_preflight.py"
PYTHONDONTWRITEBYTECODE=1 python3 \
  "${repo_root}/scripts/linux-product/test_l6_v4_canonical_input_preflight_probe.py"
PYTHONDONTWRITEBYTECODE=1 python3 \
  "${repo_root}/scripts/linux-product/test_l6_utm_install_artifacts_staged_checkpoint.py"
PYTHONDONTWRITEBYTECODE=1 python3 \
  "${repo_root}/scripts/linux-product/test_l6_v4_install_artifacts_staged_checkpoint_driver.py"
PYTHONDONTWRITEBYTECODE=1 python3 \
  "${repo_root}/scripts/linux-product/test_l6_utm_install_artifacts_staged_resume.py"
PYTHONDONTWRITEBYTECODE=1 python3 \
  "${repo_root}/scripts/linux-product/test_l6_v4_install_artifacts_staged_resume_driver.py"
PYTHONDONTWRITEBYTECODE=1 python3 \
  "${repo_root}/scripts/linux-product/test_l6_utm_install_artifacts_staged_backend_resolution.py"
PYTHONDONTWRITEBYTECODE=1 python3 \
  "${repo_root}/scripts/linux-product/test_l6_utm_install_artifacts_staged_reactivation.py"
PYTHONDONTWRITEBYTECODE=1 python3 \
  "${repo_root}/scripts/linux-product/test_l6_utm_install_artifacts_staged_runtime_resolution.py"
PYTHONDONTWRITEBYTECODE=1 python3 \
  "${repo_root}/scripts/linux-product/test_l6_utm_install_artifacts_staged_boot_transport_resolution.py"
PYTHONDONTWRITEBYTECODE=1 python3 \
  "${repo_root}/scripts/linux-product/test_l6_utm_install_artifacts_staged_boot_start_resolution.py"
PYTHONDONTWRITEBYTECODE=1 python3 \
  "${repo_root}/scripts/linux-product/test_l6_utm_install_artifacts_staged_guest_agent_resolution.py"
PYTHONDONTWRITEBYTECODE=1 python3 \
  "${repo_root}/scripts/linux-product/test_l6_utm_install_artifacts_staged_guest_agent_result_bindings.py"
PYTHONDONTWRITEBYTECODE=1 python3 \
  "${repo_root}/scripts/linux-product/test_l6_utm_install_artifacts_staged_boot_classification_resolution.py"
PYTHONDONTWRITEBYTECODE=1 python3 \
  "${repo_root}/scripts/linux-product/test_l6_utm_install_artifacts_staged_boot_classification_result_bindings.py"
PYTHONDONTWRITEBYTECODE=1 python3 \
  "${repo_root}/scripts/linux-product/test_l6_utm_install_artifacts_staged_new_boot_recovery_preflight.py"
PYTHONDONTWRITEBYTECODE=1 python3 \
  "${repo_root}/scripts/linux-product/test_l6_utm_install_artifacts_staged_new_boot_recovery_preflight_result_bindings.py"
PYTHONDONTWRITEBYTECODE=1 python3 \
  "${repo_root}/scripts/linux-product/test_l6_v4_install_artifacts_staged_new_boot_recovery_preflight.py"
PYTHONDONTWRITEBYTECODE=1 python3 \
  "${repo_root}/scripts/linux-product/test_l6_utm_install_artifacts_staged_fresh_boot_classification.py"
PYTHONDONTWRITEBYTECODE=1 python3 \
  "${repo_root}/scripts/linux-product/test_l6_utm_install_artifacts_staged_fresh_boot_recovery_preflight.py"
PYTHONDONTWRITEBYTECODE=1 python3 \
  "${repo_root}/scripts/linux-product/test_l6_utm_install_artifacts_staged_fresh_boot_recovery_preflight_result_bindings.py"
PYTHONDONTWRITEBYTECODE=1 python3 \
  "${repo_root}/scripts/linux-product/test_l6_utm_install_artifacts_staged_fresh_boot_recovery_result_resolution.py"
PYTHONDONTWRITEBYTECODE=1 python3 \
  "${repo_root}/scripts/linux-product/test_l6_utm_install_artifacts_staged_fresh_boot_recovery_result_resolution_bindings.py"
PYTHONDONTWRITEBYTECODE=1 python3 \
  "${repo_root}/scripts/linux-product/test_l6_utm_install_artifacts_staged_fresh_boot_resume.py"
PYTHONDONTWRITEBYTECODE=1 python3 \
  "${repo_root}/scripts/linux-product/test_l6_utm_install_artifacts_staged_fresh_boot_resume_result_bindings.py"
PYTHONDONTWRITEBYTECODE=1 python3 \
  "${repo_root}/scripts/linux-product/test_l6_v4_install_artifacts_staged_fresh_boot_resume_driver.py"
PYTHONDONTWRITEBYTECODE=1 python3 \
  "${repo_root}/scripts/linux-product/test_l6_utm_install_artifacts_staged_transaction_state_resolution.py"
PYTHONDONTWRITEBYTECODE=1 python3 \
  "${repo_root}/scripts/linux-product/test_l6_v4_install_artifacts_staged_transaction_state_probe.py"
PYTHONDONTWRITEBYTECODE=1 python3 \
  "${repo_root}/scripts/linux-product/test_l6_utm_install_artifacts_staged_transaction_state_resolution_result_bindings.py"
PYTHONDONTWRITEBYTECODE=1 python3 \
  "${repo_root}/scripts/linux-product/test_l6_utm_install_artifacts_staged_transaction_state_result_resolution.py"
PYTHONDONTWRITEBYTECODE=1 python3 \
  "${repo_root}/scripts/linux-product/test_l6_utm_install_artifacts_staged_transaction_state_result_resolution_result_bindings.py"
PYTHONDONTWRITEBYTECODE=1 python3 \
  "${repo_root}/scripts/linux-product/test_l6_utm_install_artifacts_staged_terminal_stop.py"
PYTHONDONTWRITEBYTECODE=1 python3 \
  "${repo_root}/scripts/linux-product/test_l6_utm_install_artifacts_staged_terminal_stop_result_bindings.py"
PYTHONDONTWRITEBYTECODE=1 python3 \
  "${repo_root}/scripts/linux-product/test_l6_v4_boot_transport_probe.py"
PYTHONDONTWRITEBYTECODE=1 python3 \
  "${repo_root}/scripts/linux-product/test_l6_v4_canonical_input_bundle.py"
PYTHONDONTWRITEBYTECODE=1 python3 \
  "${repo_root}/scripts/linux-product/l6_controller_contract.py" validate
PYTHONDONTWRITEBYTECODE=1 python3 \
  "${repo_root}/scripts/linux-product/test_l6_controller_contract.py"

echo "Linux L6 matrix, guest-case, UTM start/clone/launch/network/input transfer/resolution/negative preflight/checkpoint/resume/backend resolution/reactivation/runtime, boot transport, boot start, guest-agent resolution, frozen guest-agent and boot-classification result bindings, stopped-inventory boot classification controls, new-boot and fresh-boot read-only recovery qualification, deferred fresh-boot recovery result resolution, qualified-result binding, fresh-boot resume controls and frozen result binding, all-stopped read-only transaction-state resolution, frozen transaction-state result binding, deferred transaction-state result resolution, frozen completed result binding, terminal stop control and frozen result binding, bundle builder, and compile-identity contracts passed without guest or system mutation."
