#!/usr/bin/env python3
from __future__ import annotations

import re
from dataclasses import dataclass
from pathlib import Path
from typing import Protocol

import l6_utm_guest_network_ready as network_ready
import l6_utm_install_artifacts_staged_reactivation as reactivation_control
import l6_utm_install_artifacts_staged_reactivation_bindings as reactivation_bindings
import l6_utm_launch_transport_v2 as launch_transport
import l6_utm_start_once as start_control


EVIDENCE_FORMAT = (
    "radishlex-linux-l6-utm-install-artifacts-staged-runtime-resolution-v2"
)
PRIOR_EVIDENCE_FORMAT = (
    "radishlex-linux-l6-utm-install-artifacts-staged-runtime-resolution-v1"
)
CONTROL_RELATIVE_PATH = Path(
    "scripts/linux-product/"
    "l6_utm_install_artifacts_staged_runtime_resolution.py"
)
BINDINGS_RELATIVE_PATH = Path(
    "scripts/linux-product/"
    "l6_utm_install_artifacts_staged_runtime_resolution_bindings.py"
)
REQUIRED_PRIOR_REACTIVATION_MANIFEST_SHA256 = (
    "2a477730bc3f10167add9afee125c7aed8e8dc1df3d4aad1c8a1953f131c5c4c"
)
REQUIRED_PRIOR_REACTIVATION_REPOSITORY_HEAD = (
    "e6305e845740da216f97b0d9accd22a73d01d9d3"
)
REQUIRED_RUNTIME_RESOLUTION_ATTEMPT_ID = (
    "d75818f-v4-install-artifacts-staged-runtime-resolution-20260824-v2"
)
REQUIRED_PRIOR_RUNTIME_RESOLUTION_MANIFEST_SHA256 = (
    "bc83a806e635cb21b945524a2733263fde55dc4c53849f3be614fc606816c766"
)
REQUIRED_PRIOR_RUNTIME_RESOLUTION_REPOSITORY_HEAD = (
    "ab151a90600ce3e0dc8519a8f4efd724c357643c"
)
REQUIRED_PRIOR_RUNTIME_RESOLUTION_ATTEMPT_ID = (
    "d75818f-v4-install-artifacts-staged-runtime-resolution-20260824-v1"
)
HEX_64 = re.compile(r"[0-9a-f]{64}")
REQUIRED_PRIOR_REACTIVATION_ENTRY_NAMES = (
    "request.json",
    "binding-preflight.json",
    "source-bundle-preflight.json",
    "target-files-preflight.json",
    "host-process-preflight.json",
    "target-handles-preflight.json",
    "utmctl-list-once.json",
    "inventory-classification.json",
    *tuple(
        name
        for index in range(1, 11)
        for name in (
            f"host-process-settle-{index:03d}.json",
            f"target-handles-settle-{index:03d}.json",
        )
    ),
    "target-files-ready.json",
    "host-process-ready.json",
    "target-handles-ready.json",
    "foreground-start-once.json",
    *tuple(
        name
        for index in range(1, 31)
        for name in (
            f"host-process-runtime-{index:03d}.json",
            f"target-handles-runtime-{index:03d}.json",
        )
    ),
    "source-bundle-postflight.json",
    "terminal.json",
)
REQUIRED_PRIOR_RUNTIME_RESOLUTION_ENTRY_NAMES = (
    "request.json",
    "binding-preflight.json",
    "source-bundle-preflight.json",
    "target-files-preflight.json",
    "host-process-preflight.json",
    "utmctl-list-once.json",
    "inventory-classification.json",
    "target-handle-pid-discovery.json",
    "target-handle-pid-confirmation-001.json",
    "host-process-confirmation-001.json",
    "target-handle-pid-confirmation-002.json",
    "host-process-confirmation-002.json",
    "target-handle-pid-confirmation-003.json",
    "host-process-confirmation-003.json",
    "target-files-ready.json",
    "source-bundle-postflight.json",
    "terminal.json",
)


class RuntimeResolutionBindingRequest(Protocol):
    repository_root: Path
    expected_repository_head: str
    prior_v7_root: Path
    prior_v7_manifest_sha256: str
    prior_prepared_root: Path
    prior_prepared_manifest_sha256: str
    prior_network_root: Path
    prior_network_manifest_sha256: str
    prior_transfer_root: Path
    prior_transfer_manifest_sha256: str
    prior_resolution_root: Path
    prior_resolution_manifest_sha256: str
    prior_preflight_root: Path
    prior_preflight_manifest_sha256: str
    prior_checkpoint_root: Path
    prior_checkpoint_manifest_sha256: str
    prior_resume_failure_root: Path
    prior_resume_failure_manifest_sha256: str
    prior_backend_resolution_root: Path
    prior_backend_resolution_manifest_sha256: str
    prior_reactivation_root: Path
    prior_reactivation_manifest_sha256: str
    prior_runtime_resolution_root: Path
    prior_runtime_resolution_manifest_sha256: str
    source_bundle_path: Path
    source_bundle_size: int
    source_bundle_sha256: str
    transfer_attempt_id: str
    resolution_attempt_id: str
    preflight_attempt_id: str
    checkpoint_attempt_id: str
    resume_attempt_id: str
    backend_resolution_attempt_id: str
    reactivation_attempt_id: str
    prior_runtime_resolution_attempt_id: str
    runtime_resolution_attempt_id: str
    target_uuid: str
    target_name: str
    target_package_path: Path
    expected_vm_count: int


@dataclass(frozen=True)
class RuntimeResolutionBinding:
    evidence: dict[str, object]
    upstream: reactivation_bindings.ReactivationBinding
    expected_boot_id_sha256: str


def validate_runtime_resolution_bindings(
    request: RuntimeResolutionBindingRequest,
) -> RuntimeResolutionBinding:
    if (
        request.prior_runtime_resolution_attempt_id
        != REQUIRED_PRIOR_RUNTIME_RESOLUTION_ATTEMPT_ID
    ):
        raise ValueError(
            "required-prior-runtime-resolution-attempt-id-mismatch"
        )
    if (
        request.runtime_resolution_attempt_id
        != REQUIRED_RUNTIME_RESOLUTION_ATTEMPT_ID
    ):
        raise ValueError("required-runtime-resolution-attempt-id-mismatch")
    upstream = reactivation_bindings.validate_reactivation_bindings(request)
    identities: dict[str, object] = {}
    for relative_path, label in (
        (CONTROL_RELATIVE_PATH, "runtime_resolution_control"),
        (BINDINGS_RELATIVE_PATH, "runtime_resolution_bindings"),
    ):
        path = request.repository_root / relative_path
        network_ready._require_committed_regular(path, label)
        identities[f"{label}_sha256"] = network_ready._sha256_file(path)

    manifest = request.prior_reactivation_root / "files.sha256"
    if (
        request.prior_reactivation_manifest_sha256
        != REQUIRED_PRIOR_REACTIVATION_MANIFEST_SHA256
        or network_ready._sha256_file(manifest)
        != REQUIRED_PRIOR_REACTIVATION_MANIFEST_SHA256
    ):
        raise ValueError("prior-reactivation-manifest-identity-invalid")
    entries = start_control._verify_sha256_manifest(
        request.prior_reactivation_root, manifest
    )
    if entries != len(REQUIRED_PRIOR_REACTIVATION_ENTRY_NAMES):
        raise ValueError("prior-reactivation-entry-count-invalid")
    if _manifest_names(manifest) != REQUIRED_PRIOR_REACTIVATION_ENTRY_NAMES:
        raise ValueError("prior-reactivation-entry-names-invalid")
    _validate_prior_reactivation_semantics(request, upstream)

    prior_manifest = request.prior_runtime_resolution_root / "files.sha256"
    if (
        request.prior_runtime_resolution_manifest_sha256
        != REQUIRED_PRIOR_RUNTIME_RESOLUTION_MANIFEST_SHA256
        or network_ready._sha256_file(prior_manifest)
        != REQUIRED_PRIOR_RUNTIME_RESOLUTION_MANIFEST_SHA256
    ):
        raise ValueError("prior-runtime-resolution-manifest-identity-invalid")
    prior_entries = start_control._verify_sha256_manifest(
        request.prior_runtime_resolution_root, prior_manifest
    )
    if prior_entries != len(REQUIRED_PRIOR_RUNTIME_RESOLUTION_ENTRY_NAMES):
        raise ValueError("prior-runtime-resolution-entry-count-invalid")
    if (
        _manifest_names(prior_manifest)
        != REQUIRED_PRIOR_RUNTIME_RESOLUTION_ENTRY_NAMES
    ):
        raise ValueError("prior-runtime-resolution-entry-names-invalid")
    _validate_prior_runtime_resolution_semantics(request, upstream)

    expected_boot = upstream.expected_boot_id_sha256
    if not HEX_64.fullmatch(expected_boot):
        raise ValueError("expected-boot-id-sha256-invalid")
    return RuntimeResolutionBinding(
        evidence={
            "format": EVIDENCE_FORMAT,
            **identities,
            "expected_boot_id_sha256": expected_boot,
            "expected_vm_count": request.expected_vm_count,
            "prior_reactivation_entries_verified": entries,
            "prior_reactivation_manifest_sha256": (
                request.prior_reactivation_manifest_sha256
            ),
            "prior_reactivation_outcome": "state-indeterminate",
            "prior_reactivation_reason": (
                "post-start-runtime:"
                "foreground-start-without-bounded-target-runtime"
            ),
            "prior_runtime_resolution_attempt_id": (
                request.prior_runtime_resolution_attempt_id
            ),
            "prior_runtime_resolution_entries_verified": prior_entries,
            "prior_runtime_resolution_guest_boot_hash_invocations": 1,
            "prior_runtime_resolution_guest_observation_persisted": False,
            "prior_runtime_resolution_manifest_sha256": (
                request.prior_runtime_resolution_manifest_sha256
            ),
            "prior_runtime_resolution_outcome": "state-indeterminate",
            "prior_runtime_resolution_reason": (
                "guest-boot-id-hash-once:guest-boot-id-hash-output-invalid"
            ),
            "reactivation_attempt_id": request.reactivation_attempt_id,
            "repository_clean": True,
            "repository_head": request.expected_repository_head,
            "runtime_resolution_attempt_id": (
                request.runtime_resolution_attempt_id
            ),
            "target_uuid": request.target_uuid,
        },
        upstream=upstream,
        expected_boot_id_sha256=expected_boot,
    )


def _validate_prior_reactivation_semantics(
    request: RuntimeResolutionBindingRequest,
    upstream: reactivation_bindings.ReactivationBinding,
) -> None:
    root = request.prior_reactivation_root
    prior_request = network_ready._read_json(root / "request.json")
    required_request = {
        "authorization": {
            "install_artifacts_staged_reactivation": True,
            "no_business_guest_resume_retry_stop_or_quit": True,
            "one_foreground_start_if_stopped_quiescent": True,
            "one_potential_backend_reactivation_list": True,
            "one_read_only_boot_id_hash": True,
        },
        "backend_resolution_attempt_id": (
            request.backend_resolution_attempt_id
        ),
        "checkpoint_attempt_id": request.checkpoint_attempt_id,
        "command_timeout_seconds": 60,
        "expected_repository_head": (
            REQUIRED_PRIOR_REACTIVATION_REPOSITORY_HEAD
        ),
        "expected_vm_count": 21,
        "format": reactivation_bindings.EVIDENCE_FORMAT,
        "poll_interval_seconds": 1,
        "prior_backend_resolution_manifest_sha256": (
            request.prior_backend_resolution_manifest_sha256
        ),
        "prior_checkpoint_manifest_sha256": (
            request.prior_checkpoint_manifest_sha256
        ),
        "prior_network_manifest_sha256": request.prior_network_manifest_sha256,
        "prior_preflight_manifest_sha256": (
            request.prior_preflight_manifest_sha256
        ),
        "prior_prepared_manifest_sha256": (
            request.prior_prepared_manifest_sha256
        ),
        "prior_resolution_manifest_sha256": (
            request.prior_resolution_manifest_sha256
        ),
        "prior_resume_failure_manifest_sha256": (
            request.prior_resume_failure_manifest_sha256
        ),
        "prior_transfer_manifest_sha256": (
            request.prior_transfer_manifest_sha256
        ),
        "prior_v7_manifest_sha256": request.prior_v7_manifest_sha256,
        "reactivation_attempt_id": request.reactivation_attempt_id,
        "resolution_attempt_id": request.resolution_attempt_id,
        "resume_attempt_id": request.resume_attempt_id,
        "runtime_poll_attempts": 30,
        "settle_attempts": 10,
        "source_bundle_path_sha256": network_ready._sha256_text(
            str(request.source_bundle_path)
        ),
        "source_bundle_sha256": request.source_bundle_sha256,
        "source_bundle_size": request.source_bundle_size,
        "target_name": request.target_name,
        "target_package_path_sha256": network_ready._sha256_text(
            str(request.target_package_path)
        ),
        "target_uuid": request.target_uuid,
        "transfer_attempt_id": request.transfer_attempt_id,
        "transport_id": reactivation_control.launch_bindings.TRANSPORT_ID,
        "transport_timeout_seconds": 60,
    }
    if prior_request != required_request:
        raise ValueError("prior-reactivation-request-semantics-invalid")

    prior_binding = network_ready._read_json(root / "binding-preflight.json")
    required_binding = {
        "expected_boot_id_sha256": upstream.expected_boot_id_sha256,
        "expected_vm_count": 21,
        "foreground_launch_bindings_sha256": (
            "a78ad10b6a2d8b68c2e5de9c3bf02b343370edfa8e877cd57a44903a8a8454cc"
        ),
        "foreground_launch_control_sha256": (
            "4f02422d5b153b7ac439d01ff3fe5fc139667f30a9edb79ebce0dc03aa6b07e2"
        ),
        "foreground_transport_sha256": (
            "342e72e309edd8cf0d0043d12d56782e0ed8750d6601c214d16f48109f63bca7"
        ),
        "format": reactivation_bindings.EVIDENCE_FORMAT,
        "prepared_target_config_sha256": (
            "2de7280bbf906a8e899662c8686cb108755c83263150f5bd455490b050536195"
        ),
        "prepared_target_efi_sha256": (
            "0b797641e80ea0314017006730332811a44d0170dd334910fb52088b5b521418"
        ),
        "prepared_target_qcow2_sha256": (
            "4967234b09963df4f6bcd6d44f906c66d18e782092e727a0a220381b580e4b18"
        ),
        "prior_backend_resolution_entries_verified": 30,
        "prior_backend_resolution_manifest_sha256": (
            request.prior_backend_resolution_manifest_sha256
        ),
        "prior_backend_resolution_outcome": "registered-stopped",
        "prior_prepared_manifest_sha256": (
            request.prior_prepared_manifest_sha256
        ),
        "prior_v7_manifest_sha256": request.prior_v7_manifest_sha256,
        "reactivation_attempt_id": request.reactivation_attempt_id,
        "reactivation_bindings_sha256": (
            "068998250169ace0f34d0121caac1299377c559d47ebace4637d081fcec8ff42"
        ),
        "reactivation_control_sha256": (
            "1eec34bab9b034f7f90b740f39de2c54d52c79a0ab19c71c6f6b664366e5e53a"
        ),
        "repository_clean": True,
        "repository_head": REQUIRED_PRIOR_REACTIVATION_REPOSITORY_HEAD,
        "target_uuid": request.target_uuid,
        "transport_id": reactivation_control.launch_bindings.TRANSPORT_ID,
        "v7_baseline_vm_count": 20,
    }
    if prior_binding != required_binding:
        raise ValueError("prior-reactivation-binding-semantics-invalid")

    source_preflight = network_ready._read_json(
        root / "source-bundle-preflight.json"
    )
    descriptor = source_preflight.get("descriptor")
    if (
        source_preflight.get("format")
        != "radishlex-linux-l6-utm-canonical-input-transfer-v1"
        or source_preflight.get("inventory_count") != 12
        or not isinstance(descriptor, dict)
        or descriptor.get("sha256") != request.source_bundle_sha256
        or descriptor.get("size") != request.source_bundle_size
    ):
        raise ValueError("prior-reactivation-source-preflight-invalid")
    source_postflight = network_ready._read_json(
        root / "source-bundle-postflight.json"
    )
    if source_postflight != {
        "descriptor_unchanged": True,
        "format": "radishlex-linux-l6-utm-canonical-input-transfer-v1",
        "inventory_unchanged": True,
        "sha256": request.source_bundle_sha256,
        "size": request.source_bundle_size,
    }:
        raise ValueError("prior-reactivation-source-postflight-invalid")

    target_preflight = network_ready._read_json(
        root / "target-files-preflight.json"
    )
    if target_preflight != network_ready._read_json(
        root / "target-files-ready.json"
    ):
        raise ValueError("prior-reactivation-target-drift")
    if (
        target_preflight.get("format") != network_ready.EVIDENCE_FORMAT
        or target_preflight.get("target_config_sha256")
        != network_ready.REQUIRED_TARGET_CONFIG_SHA256
        or target_preflight.get("target_package_name")
        != request.target_package_path.name
    ):
        raise ValueError("prior-reactivation-target-semantics-invalid")

    _require_process_state(root / "host-process-preflight.json", 0)
    _require_handle_state(
        root / "target-handles-preflight.json", "absent", request
    )
    list_value = network_ready._read_json(root / "utmctl-list-once.json")
    list_observation = (
        reactivation_control.launch_bindings._command_observation_from_json(
            list_value, "prior-reactivation-list"
        )
    )
    inventory = start_control.parse_utmctl_list(list_observation)
    if (
        reactivation_control._require_inventory(
            inventory, upstream.baseline_inventory, request
        )
        != "stopped"
    ):
        raise ValueError("prior-reactivation-inventory-semantics-invalid")
    if network_ready._read_json(root / "inventory-classification.json") != {
        "format": reactivation_bindings.EVIDENCE_FORMAT,
        "other_registered_vm_count": 20,
        "other_registered_vms": "all-stopped",
        "registered_vm_count": 21,
        "target_registered_status": "stopped",
        "target_uuid": request.target_uuid,
    }:
        raise ValueError("prior-reactivation-inventory-classification-invalid")

    for index in range(1, 11):
        _require_process_state(
            root / f"host-process-settle-{index:03d}.json", 0
        )
        _require_handle_state(
            root / f"target-handles-settle-{index:03d}.json",
            "absent",
            request,
        )
    _require_process_state(root / "host-process-ready.json", 0)
    _require_handle_state(root / "target-handles-ready.json", "absent", request)
    transport = network_ready._read_json(root / "foreground-start-once.json")
    transport_observation = (
        reactivation_control.launch_bindings._command_observation_from_json(
            transport, "prior-reactivation-transport"
        )
    )
    if (
        transport_observation.argv
        != reactivation_control.launch_transport.transport_argv(request)
        or transport_observation.exit_code != 0
        or transport_observation.timed_out
        or transport_observation.stdout.total_bytes != 0
        or transport_observation.stderr.total_bytes != 0
    ):
        raise ValueError("prior-reactivation-transport-semantics-invalid")

    handle_hash: str | None = None
    for index in range(1, 31):
        _require_process_state(
            root / f"host-process-runtime-{index:03d}.json", 0
        )
        value = _require_handle_state(
            root / f"target-handles-runtime-{index:03d}.json",
            "present",
            request,
        )
        observation = value.get("observation")
        if not isinstance(observation, dict):
            raise ValueError("prior-reactivation-runtime-handle-invalid")
        stdout = observation.get("stdout")
        if not isinstance(stdout, dict):
            raise ValueError("prior-reactivation-runtime-handle-invalid")
        current_hash = stdout.get("sha256")
        if not isinstance(current_hash, str) or not HEX_64.fullmatch(current_hash):
            raise ValueError("prior-reactivation-runtime-handle-invalid")
        if handle_hash is None:
            handle_hash = current_hash
        elif current_hash != handle_hash:
            raise ValueError("prior-reactivation-runtime-handle-drift")

    terminal = network_ready._read_json(root / "terminal.json")
    required_terminal = {
        "automatic_cleanup": "not-performed",
        "automatic_quit": "not-performed",
        "automatic_retry": "not-performed",
        "automatic_stop": "not-performed",
        "business_guest_action": "not-performed",
        "file_pull_invocations": 0,
        "file_push_invocations": 0,
        "foreground_start_invocations": 1,
        "format": reactivation_bindings.EVIDENCE_FORMAT,
        "guest_boot_hash_invocations": 0,
        "inventory_probe_invocations": 1,
        "maintenance_resume_invocations": 0,
        "observed_boot_id_sha256": None,
        "operation_id": "not-read-or-generated",
        "outcome": "state-indeterminate",
        "plain_utmctl_list": (
            "attempted-once-as-potential-backend-reactivation"
        ),
        "plain_utmctl_start": "not-performed",
        "plain_utmctl_status": "not-performed",
        "reactivation_attempt_id": request.reactivation_attempt_id,
        "reason": (
            "post-start-runtime:"
            "foreground-start-without-bounded-target-runtime"
        ),
        "registered_status_from_inventory": "stopped",
        "runtime_observation_source": "not-observed",
        "runtime_poll_count": 30,
        "settle_observation_count": 10,
        "target_handles_terminal": "present",
        "target_name": request.target_name,
        "target_uuid": request.target_uuid,
        "terminal_relevant_host_process_count": 0,
        "transaction": "artifacts-staged-preserved-no-resume",
    }
    if terminal != required_terminal:
        raise ValueError("prior-reactivation-terminal-semantics-invalid")


def _validate_prior_runtime_resolution_semantics(
    request: RuntimeResolutionBindingRequest,
    upstream: reactivation_bindings.ReactivationBinding,
) -> None:
    root = request.prior_runtime_resolution_root
    prior_request = network_ready._read_json(root / "request.json")
    required_request = {
        "authorization": {
            "install_artifacts_staged_runtime_resolution": True,
            "no_start_status_resume_business_guest_retry_stop_or_quit": True,
            "one_potential_backend_reactivation_list": True,
            "one_read_only_boot_id_hash": True,
            "stable_target_handle_pid_observations": True,
        },
        "backend_resolution_attempt_id": request.backend_resolution_attempt_id,
        "checkpoint_attempt_id": request.checkpoint_attempt_id,
        "command_timeout_seconds": 60,
        "expected_repository_head": (
            REQUIRED_PRIOR_RUNTIME_RESOLUTION_REPOSITORY_HEAD
        ),
        "expected_vm_count": 21,
        "format": PRIOR_EVIDENCE_FORMAT,
        "identity_observations": 3,
        "poll_interval_seconds": 1,
        "prior_backend_resolution_manifest_sha256": (
            request.prior_backend_resolution_manifest_sha256
        ),
        "prior_checkpoint_manifest_sha256": (
            request.prior_checkpoint_manifest_sha256
        ),
        "prior_network_manifest_sha256": request.prior_network_manifest_sha256,
        "prior_preflight_manifest_sha256": (
            request.prior_preflight_manifest_sha256
        ),
        "prior_prepared_manifest_sha256": (
            request.prior_prepared_manifest_sha256
        ),
        "prior_reactivation_manifest_sha256": (
            request.prior_reactivation_manifest_sha256
        ),
        "prior_resolution_manifest_sha256": (
            request.prior_resolution_manifest_sha256
        ),
        "prior_resume_failure_manifest_sha256": (
            request.prior_resume_failure_manifest_sha256
        ),
        "prior_transfer_manifest_sha256": (
            request.prior_transfer_manifest_sha256
        ),
        "prior_v7_manifest_sha256": request.prior_v7_manifest_sha256,
        "reactivation_attempt_id": request.reactivation_attempt_id,
        "resolution_attempt_id": request.resolution_attempt_id,
        "resume_attempt_id": request.resume_attempt_id,
        "runtime_resolution_attempt_id": (
            request.prior_runtime_resolution_attempt_id
        ),
        "source_bundle_path_sha256": network_ready._sha256_text(
            str(request.source_bundle_path)
        ),
        "source_bundle_sha256": request.source_bundle_sha256,
        "source_bundle_size": request.source_bundle_size,
        "target_name": request.target_name,
        "target_package_path_sha256": network_ready._sha256_text(
            str(request.target_package_path)
        ),
        "target_uuid": request.target_uuid,
        "transfer_attempt_id": request.transfer_attempt_id,
    }
    if prior_request != required_request:
        raise ValueError("prior-runtime-resolution-request-semantics-invalid")

    prior_binding = network_ready._read_json(root / "binding-preflight.json")
    required_binding = {
        "expected_boot_id_sha256": upstream.expected_boot_id_sha256,
        "expected_vm_count": 21,
        "format": PRIOR_EVIDENCE_FORMAT,
        "prior_reactivation_entries_verified": 94,
        "prior_reactivation_manifest_sha256": (
            request.prior_reactivation_manifest_sha256
        ),
        "prior_reactivation_outcome": "state-indeterminate",
        "prior_reactivation_reason": (
            "post-start-runtime:foreground-start-without-bounded-target-runtime"
        ),
        "reactivation_attempt_id": request.reactivation_attempt_id,
        "repository_clean": True,
        "repository_head": REQUIRED_PRIOR_RUNTIME_RESOLUTION_REPOSITORY_HEAD,
        "runtime_resolution_attempt_id": (
            request.prior_runtime_resolution_attempt_id
        ),
        "runtime_resolution_bindings_sha256": (
            "5b6da311f15fc521d1a4033e7a10da983474d85e0c4670560529a9f9c2be168a"
        ),
        "runtime_resolution_control_sha256": (
            "52898a8a2cdd8ebfab631ecf61d07e883b3a98936e23ccd27d42eae29a2ae575"
        ),
        "target_uuid": request.target_uuid,
    }
    if prior_binding != required_binding:
        raise ValueError("prior-runtime-resolution-binding-semantics-invalid")

    source_preflight = network_ready._read_json(
        root / "source-bundle-preflight.json"
    )
    descriptor = source_preflight.get("descriptor")
    if (
        source_preflight.get("format")
        != "radishlex-linux-l6-utm-canonical-input-transfer-v1"
        or source_preflight.get("inventory_count") != 12
        or not isinstance(descriptor, dict)
        or descriptor.get("sha256") != request.source_bundle_sha256
        or descriptor.get("size") != request.source_bundle_size
    ):
        raise ValueError("prior-runtime-resolution-source-preflight-invalid")

    target_preflight = network_ready._read_json(
        root / "target-files-preflight.json"
    )
    target_ready = network_ready._read_json(root / "target-files-ready.json")
    if target_preflight != target_ready:
        raise ValueError("prior-runtime-resolution-target-drift")
    if (
        target_preflight.get("format") != network_ready.EVIDENCE_FORMAT
        or target_preflight.get("target_config_sha256")
        != network_ready.REQUIRED_TARGET_CONFIG_SHA256
        or target_preflight.get("target_package_name")
        != request.target_package_path.name
        or target_preflight.get("target_package_path_sha256")
        != network_ready._sha256_text(str(request.target_package_path))
    ):
        raise ValueError("prior-runtime-resolution-target-semantics-invalid")

    _require_process_state(
        root / "host-process-preflight.json",
        0,
        expected_format=PRIOR_EVIDENCE_FORMAT,
        error_label="prior-runtime-resolution",
    )
    list_value = network_ready._read_json(root / "utmctl-list-once.json")
    list_observation = (
        reactivation_control.launch_bindings._command_observation_from_json(
            list_value, "prior-runtime-resolution-list"
        )
    )
    inventory = start_control.parse_utmctl_list(list_observation)
    if (
        reactivation_control._require_inventory(
            inventory, upstream.baseline_inventory, request
        )
        != "started"
    ):
        raise ValueError("prior-runtime-resolution-inventory-semantics-invalid")
    if network_ready._read_json(root / "inventory-classification.json") != {
        "format": PRIOR_EVIDENCE_FORMAT,
        "other_registered_vm_count": 20,
        "other_registered_vms": "all-stopped",
        "registered_vm_count": 21,
        "target_registered_status": "started",
        "target_uuid": request.target_uuid,
    }:
        raise ValueError(
            "prior-runtime-resolution-inventory-classification-invalid"
        )

    discovery = network_ready._read_json(
        root / "target-handle-pid-discovery.json"
    )
    backend_pid = discovery.get("backend_pid")
    if not isinstance(backend_pid, int) or backend_pid <= 1:
        raise ValueError("prior-runtime-resolution-backend-pid-invalid")
    handle_hash = _require_runtime_handle_state(
        discovery, network_ready._lsof_argv(request), backend_pid
    )
    targeted_argv = _targeted_lsof_argv(request, backend_pid)
    for index in range(1, 4):
        confirmation = network_ready._read_json(
            root / f"target-handle-pid-confirmation-{index:03d}.json"
        )
        if (
            _require_runtime_handle_state(
                confirmation, targeted_argv, backend_pid
            )
            != handle_hash
        ):
            raise ValueError("prior-runtime-resolution-handle-output-drift")
        _require_process_state(
            root / f"host-process-confirmation-{index:03d}.json",
            0,
            expected_format=PRIOR_EVIDENCE_FORMAT,
            error_label="prior-runtime-resolution",
        )

    source_postflight = network_ready._read_json(
        root / "source-bundle-postflight.json"
    )
    if source_postflight != {
        "descriptor_unchanged": True,
        "format": "radishlex-linux-l6-utm-canonical-input-transfer-v1",
        "inventory_unchanged": True,
        "sha256": request.source_bundle_sha256,
        "size": request.source_bundle_size,
    }:
        raise ValueError("prior-runtime-resolution-source-postflight-invalid")

    terminal = network_ready._read_json(root / "terminal.json")
    if terminal != {
        "automatic_cleanup": "not-performed",
        "automatic_quit": "not-performed",
        "automatic_retry": "not-performed",
        "automatic_stop": "not-performed",
        "backend_pid": backend_pid,
        "business_guest_action": "not-performed",
        "file_pull_invocations": 0,
        "file_push_invocations": 0,
        "foreground_start_invocations": 0,
        "format": PRIOR_EVIDENCE_FORMAT,
        "guest_boot_hash_invocations": 1,
        "identity_observation_count": 3,
        "inventory_probe_invocations": 1,
        "maintenance_resume_invocations": 0,
        "observed_boot_id_sha256": None,
        "operation_id": "not-read-or-generated",
        "outcome": "state-indeterminate",
        "plain_utmctl_list": (
            "attempted-once-as-potential-backend-reactivation"
        ),
        "plain_utmctl_start": "not-performed",
        "plain_utmctl_status": "not-performed",
        "reason": (
            "guest-boot-id-hash-once:guest-boot-id-hash-output-invalid"
        ),
        "runtime_resolution_attempt_id": (
            request.prior_runtime_resolution_attempt_id
        ),
        "target_handles_terminal": "present",
        "target_name": request.target_name,
        "target_uuid": request.target_uuid,
        "terminal_relevant_host_process_count": 0,
        "transaction": "artifacts-staged-preserved-no-resume",
    }:
        raise ValueError("prior-runtime-resolution-terminal-semantics-invalid")


def _require_process_state(
    path: Path,
    expected_count: int,
    *,
    expected_format: str = reactivation_bindings.EVIDENCE_FORMAT,
    error_label: str = "prior-reactivation",
) -> dict[str, object]:
    value = network_ready._read_json(path)
    observation = value.get("observation")
    if (
        value.get("format") != expected_format
        or value.get("relevant_process_count") != expected_count
        or value.get("relevant_processes") != []
        or not isinstance(observation, dict)
        or observation.get("argv") != list(launch_transport.PROCESS_COMMAND)
        or observation.get("exit_code") != 0
        or observation.get("timed_out") is not False
        or not _empty_stream(observation.get("stderr"))
        or not _complete_stream(observation.get("stdout"))
    ):
        raise ValueError(f"{error_label}-process-invalid:{path.name}")
    return value


def _require_runtime_handle_state(
    value: dict[str, object],
    expected_argv: tuple[str, ...],
    expected_pid: int,
) -> str:
    observation = value.get("observation")
    if (
        value.get("format") != PRIOR_EVIDENCE_FORMAT
        or value.get("state") != "present"
        or value.get("backend_command") != "QEMULauncher"
        or value.get("backend_pid") != expected_pid
        or value.get("efi_handle_count") != 1
        or value.get("process_record_count") != 1
        or value.get("qcow2_handle_count") != 1
        or not isinstance(observation, dict)
        or observation.get("argv") != list(expected_argv)
        or observation.get("exit_code") != 0
        or observation.get("timed_out") is not False
        or not _empty_stream(observation.get("stderr"))
        or not _complete_stream(observation.get("stdout"))
    ):
        raise ValueError("prior-runtime-resolution-handle-semantics-invalid")
    stdout = observation.get("stdout")
    if not isinstance(stdout, dict):
        raise ValueError("prior-runtime-resolution-handle-stdout-invalid")
    digest = stdout.get("sha256")
    if not isinstance(digest, str):
        raise ValueError("prior-runtime-resolution-handle-hash-invalid")
    return digest


def _targeted_lsof_argv(
    request: RuntimeResolutionBindingRequest, backend_pid: int
) -> tuple[str, ...]:
    data = request.target_package_path / "Data"
    return (
        "/usr/sbin/lsof",
        "-n",
        "-P",
        "-a",
        "-p",
        str(backend_pid),
        "-F",
        "pctfn",
        str(data / "efi_vars.fd"),
        str(data / network_ready.REQUIRED_QCOW2_NAME),
    )


def _require_handle_state(
    path: Path,
    expected_state: str,
    request: RuntimeResolutionBindingRequest,
) -> dict[str, object]:
    value = network_ready._read_json(path)
    observation = value.get("observation")
    expected = {
        "absent": (None, 0, 0, 0, 1, _empty_stream),
        "present": ("QEMULauncher", 1, 1, 1, 0, _complete_stream),
    }[expected_state]
    command, efi_count, process_count, qcow2_count, exit_code, stdout_check = expected
    if (
        value.get("format") != reactivation_bindings.EVIDENCE_FORMAT
        or value.get("state") != expected_state
        or value.get("backend_command") != command
        or value.get("efi_handle_count") != efi_count
        or value.get("process_record_count") != process_count
        or value.get("qcow2_handle_count") != qcow2_count
        or not isinstance(observation, dict)
        or observation.get("argv") != list(network_ready._lsof_argv(request))
        or observation.get("exit_code") != exit_code
        or observation.get("timed_out") is not False
        or not _empty_stream(observation.get("stderr"))
        or not stdout_check(observation.get("stdout"))
    ):
        raise ValueError(f"prior-reactivation-handles-invalid:{path.name}")
    return value


def _empty_stream(value: object) -> bool:
    return (
        isinstance(value, dict)
        and value.get("sha256")
        == "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        and value.get("total_bytes") == 0
        and value.get("truncated") is False
    )


def _complete_stream(value: object) -> bool:
    return (
        isinstance(value, dict)
        and isinstance(value.get("sha256"), str)
        and HEX_64.fullmatch(str(value.get("sha256"))) is not None
        and value.get("total_bytes", 0) > 0
        and value.get("truncated") is False
    )


def _manifest_names(path: Path) -> tuple[str, ...]:
    try:
        lines = path.read_text(encoding="ascii").splitlines()
    except (OSError, UnicodeDecodeError) as exc:
        raise ValueError("prior-reactivation-manifest-unreadable") from exc
    names: list[str] = []
    for line in lines:
        parts = line.split("  ", 1)
        if len(parts) != 2:
            raise ValueError("prior-reactivation-manifest-line-invalid")
        names.append(parts[1])
    return tuple(names)
