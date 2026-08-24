#!/usr/bin/env python3
from __future__ import annotations

import re
from dataclasses import dataclass
from pathlib import Path
from typing import Protocol

import l6_utm_guest_network_ready as network_ready
import l6_utm_install_artifacts_staged_backend_resolution_bindings as backend_bindings
import l6_utm_launch_transport_bindings as launch_bindings
import l6_utm_launch_transport_v2 as launch_transport
import l6_utm_start_once as start_control


EVIDENCE_FORMAT = (
    "radishlex-linux-l6-utm-install-artifacts-staged-reactivation-v1"
)
CONTROL_RELATIVE_PATH = Path(
    "scripts/linux-product/l6_utm_install_artifacts_staged_reactivation.py"
)
BINDINGS_RELATIVE_PATH = Path(
    "scripts/linux-product/"
    "l6_utm_install_artifacts_staged_reactivation_bindings.py"
)
REQUIRED_PRIOR_BACKEND_RESOLUTION_MANIFEST_SHA256 = (
    "0193b4350fa97ba1d00f159fd2384e2cdbe4f790b61d6a8cd98f3740b80363e9"
)
REQUIRED_PRIOR_BACKEND_RESOLUTION_REPOSITORY_HEAD = (
    "92e82e25c25deff8aba8dbcc8ff74c876c3a8ccd"
)
REQUIRED_REACTIVATION_ATTEMPT_ID = (
    "d75818f-v4-install-artifacts-staged-reactivation-20260824-v1"
)
HEX_64 = re.compile(r"[0-9a-f]{64}")
REQUIRED_PRIOR_BACKEND_RESOLUTION_ENTRY_NAMES = (
    "request.json",
    "binding-preflight.json",
    "source-bundle-preflight.json",
    "target-files-preflight.json",
    "host-process-preflight.json",
    "target-handles-preflight.json",
    "utmctl-status-once.json",
    *tuple(
        name
        for index in range(1, 11)
        for name in (
            f"host-process-poll-{index:03d}.json",
            f"target-handles-poll-{index:03d}.json",
        )
    ),
    "target-files-postflight.json",
    "source-bundle-postflight.json",
    "terminal.json",
)


class ReactivationBindingRequest(Protocol):
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
    target_uuid: str
    target_name: str
    target_package_path: Path
    expected_vm_count: int


@dataclass(frozen=True)
class ReactivationBinding:
    evidence: dict[str, object]
    backend: backend_bindings.BackendResolutionBinding
    launch: dict[str, object]
    baseline_inventory: tuple[start_control.RegisteredVm, ...]
    expected_boot_id_sha256: str


def validate_reactivation_bindings(
    request: ReactivationBindingRequest,
) -> ReactivationBinding:
    if request.reactivation_attempt_id != REQUIRED_REACTIVATION_ATTEMPT_ID:
        raise ValueError("required-reactivation-attempt-id-mismatch")
    upstream = backend_bindings.validate_backend_resolution_bindings(request)
    launch = launch_bindings.validate_launch_bindings(request)
    identities: dict[str, object] = {}
    for relative_path, label in (
        (CONTROL_RELATIVE_PATH, "reactivation_control"),
        (BINDINGS_RELATIVE_PATH, "reactivation_bindings"),
        (launch_bindings.TRANSPORT_RELATIVE_PATH, "foreground_transport"),
    ):
        path = request.repository_root / relative_path
        network_ready._require_committed_regular(path, label)
        identities[f"{label}_sha256"] = network_ready._sha256_file(path)
    for source_key, evidence_key in (
        ("control_sha256", "foreground_launch_control_sha256"),
        ("binding_control_sha256", "foreground_launch_bindings_sha256"),
        ("prepared_target_config_sha256", "prepared_target_config_sha256"),
        ("prepared_target_efi_sha256", "prepared_target_efi_sha256"),
        ("prepared_target_qcow2_sha256", "prepared_target_qcow2_sha256"),
    ):
        value = launch.get(source_key)
        if not isinstance(value, str) or not HEX_64.fullmatch(value):
            raise ValueError(f"launch-{source_key.replace('_', '-')}-invalid")
        identities[evidence_key] = value

    manifest = request.prior_backend_resolution_root / "files.sha256"
    if (
        request.prior_backend_resolution_manifest_sha256
        != REQUIRED_PRIOR_BACKEND_RESOLUTION_MANIFEST_SHA256
        or network_ready._sha256_file(manifest)
        != REQUIRED_PRIOR_BACKEND_RESOLUTION_MANIFEST_SHA256
    ):
        raise ValueError("prior-backend-resolution-manifest-identity-invalid")
    entries = start_control._verify_sha256_manifest(
        request.prior_backend_resolution_root, manifest
    )
    if entries != len(REQUIRED_PRIOR_BACKEND_RESOLUTION_ENTRY_NAMES):
        raise ValueError("prior-backend-resolution-entry-count-invalid")
    _validate_prior_backend_resolution_semantics(request)

    baseline_inventory = _baseline_inventory(launch, request)
    expected_boot = upstream.resume.boot_id_sha256
    if not HEX_64.fullmatch(expected_boot):
        raise ValueError("expected-boot-id-sha256-invalid")
    return ReactivationBinding(
        evidence={
            "format": EVIDENCE_FORMAT,
            **identities,
            "expected_boot_id_sha256": expected_boot,
            "expected_vm_count": request.expected_vm_count,
            "prior_backend_resolution_entries_verified": entries,
            "prior_backend_resolution_manifest_sha256": (
                request.prior_backend_resolution_manifest_sha256
            ),
            "prior_backend_resolution_outcome": "registered-stopped",
            "prior_prepared_manifest_sha256": (
                request.prior_prepared_manifest_sha256
            ),
            "prior_v7_manifest_sha256": request.prior_v7_manifest_sha256,
            "reactivation_attempt_id": request.reactivation_attempt_id,
            "repository_clean": True,
            "repository_head": request.expected_repository_head,
            "target_uuid": request.target_uuid,
            "transport_id": launch_bindings.TRANSPORT_ID,
            "v7_baseline_vm_count": len(baseline_inventory),
        },
        backend=upstream,
        launch=launch,
        baseline_inventory=baseline_inventory,
        expected_boot_id_sha256=expected_boot,
    )


def _validate_prior_backend_resolution_semantics(
    request: ReactivationBindingRequest,
) -> None:
    root = request.prior_backend_resolution_root
    manifest = root / "files.sha256"
    if _manifest_names(manifest) != REQUIRED_PRIOR_BACKEND_RESOLUTION_ENTRY_NAMES:
        raise ValueError("prior-backend-resolution-entry-names-invalid")

    prior_request = network_ready._read_json(root / "request.json")
    required_request = {
        "authorization": {
            "install_artifacts_staged_backend_resolution": True,
            "no_list_start_resume_guest_retry_stop_or_quit": True,
            "one_potential_backend_reactivation_status": True,
        },
        "backend_resolution_attempt_id": (
            request.backend_resolution_attempt_id
        ),
        "checkpoint_attempt_id": request.checkpoint_attempt_id,
        "command_timeout_seconds": 60,
        "expected_repository_head": (
            REQUIRED_PRIOR_BACKEND_RESOLUTION_REPOSITORY_HEAD
        ),
        "format": backend_bindings.EVIDENCE_FORMAT,
        "poll_attempts": 10,
        "poll_interval_seconds": 1,
        "prior_checkpoint_manifest_sha256": (
            request.prior_checkpoint_manifest_sha256
        ),
        "prior_network_manifest_sha256": request.prior_network_manifest_sha256,
        "prior_preflight_manifest_sha256": (
            request.prior_preflight_manifest_sha256
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
        "resume_attempt_id": request.resume_attempt_id,
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
    }
    if prior_request != required_request:
        raise ValueError("prior-backend-resolution-request-semantics-invalid")

    prior_binding = network_ready._read_json(root / "binding-preflight.json")
    required_binding = {
        "backend_resolution_attempt_id": (
            request.backend_resolution_attempt_id
        ),
        "backend_resolution_bindings_sha256": (
            "cfef16a7b21c347a4ec0862d74fa23b8217b2e163620a137821d9cae035f57e2"
        ),
        "backend_resolution_control_sha256": (
            "e0476f29b88e318f84dc4db560709d1f2c8961d6364eef13d30d57a44dc61812"
        ),
        "format": backend_bindings.EVIDENCE_FORMAT,
        "prior_resume_failure_entries_verified": 7,
        "prior_resume_failure_manifest_sha256": (
            request.prior_resume_failure_manifest_sha256
        ),
        "prior_resume_failure_outcome": "precondition-rejected",
        "prior_resume_failure_reason": (
            "target-handles-preflight:target-handles-preflight-exit-1"
        ),
        "repository_clean": True,
        "repository_head": REQUIRED_PRIOR_BACKEND_RESOLUTION_REPOSITORY_HEAD,
        "resume_attempt_id": request.resume_attempt_id,
        "source_bundle_sha256": request.source_bundle_sha256,
        "source_bundle_size": request.source_bundle_size,
        "target_uuid": request.target_uuid,
    }
    if prior_binding != required_binding:
        raise ValueError("prior-backend-resolution-binding-semantics-invalid")

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
        raise ValueError("prior-backend-resolution-source-preflight-invalid")

    target_preflight = network_ready._read_json(
        root / "target-files-preflight.json"
    )
    target_postflight = network_ready._read_json(
        root / "target-files-postflight.json"
    )
    if (
        target_preflight != target_postflight
        or target_preflight.get("format")
        != network_ready.EVIDENCE_FORMAT
        or target_preflight.get("target_config_sha256")
        != network_ready.REQUIRED_TARGET_CONFIG_SHA256
        or target_preflight.get("target_package_name")
        != request.target_package_path.name
        or target_preflight.get("target_package_path_sha256")
        != network_ready._sha256_text(str(request.target_package_path))
    ):
        raise ValueError("prior-backend-resolution-target-semantics-invalid")

    _require_prior_process_quiescent(
        root / "host-process-preflight.json", "preflight"
    )
    _require_prior_handles_absent(
        root / "target-handles-preflight.json", request, "preflight"
    )
    for index in range(1, 11):
        label = f"poll-{index:03d}"
        _require_prior_process_quiescent(
            root / f"host-process-poll-{index:03d}.json", label
        )
        _require_prior_handles_absent(
            root / f"target-handles-poll-{index:03d}.json", request, label
        )

    status_value = network_ready._read_json(root / "utmctl-status-once.json")
    status_observation = launch_bindings._command_observation_from_json(
        status_value, "prior-backend-resolution-status"
    )
    if (
        status_observation.argv != ("utmctl", "status", request.target_uuid)
        or start_control.parse_utmctl_status(status_observation) != "stopped"
    ):
        raise ValueError("prior-backend-resolution-status-semantics-invalid")

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
        raise ValueError("prior-backend-resolution-source-postflight-invalid")

    terminal = network_ready._read_json(root / "terminal.json")
    required_terminal = {
        "automatic_cleanup": "not-performed",
        "automatic_quit": "not-performed",
        "automatic_retry": "not-performed",
        "automatic_stop": "not-performed",
        "backend_reactivation_probe_invocations": 1,
        "backend_resolution_attempt_id": (
            request.backend_resolution_attempt_id
        ),
        "file_pull_invocations": 0,
        "file_push_invocations": 0,
        "format": backend_bindings.EVIDENCE_FORMAT,
        "guest_exec_invocations": 0,
        "maintenance_resume_invocations": 0,
        "observation_poll_count": 10,
        "operation_id": "not-read-or-generated",
        "outcome": "registered-stopped",
        "plain_utmctl_list": "not-performed",
        "plain_utmctl_start": "not-performed",
        "plain_utmctl_status": (
            "attempted-once-as-potential-backend-reactivation"
        ),
        "reason": "single-status-stopped-and-bounded-host-quiescence",
        "registered_status": "stopped",
        "saved_state": "registered-stopped-saved-state-not-inferred",
        "target_handles_terminal": "absent",
        "target_name": request.target_name,
        "target_uuid": request.target_uuid,
        "terminal_relevant_host_process_count": 0,
        "transaction": "artifacts-staged-preserved-no-resume",
    }
    if terminal != required_terminal:
        raise ValueError("prior-backend-resolution-terminal-semantics-invalid")


def _require_prior_process_quiescent(path: Path, label: str) -> None:
    value = network_ready._read_json(path)
    observation = value.get("observation")
    if (
        value.get("format") != backend_bindings.EVIDENCE_FORMAT
        or value.get("relevant_process_count") != 0
        or value.get("relevant_processes") != []
        or not isinstance(observation, dict)
        or observation.get("argv") != list(launch_transport.PROCESS_COMMAND)
        or observation.get("exit_code") != 0
        or observation.get("timed_out") is not False
        or not _empty_stream(observation.get("stderr"))
        or not _complete_stream(observation.get("stdout"))
    ):
        raise ValueError(
            f"prior-backend-resolution-process-{label}-invalid"
        )


def _require_prior_handles_absent(
    path: Path, request: ReactivationBindingRequest, label: str
) -> None:
    value = network_ready._read_json(path)
    observation = value.get("observation")
    if (
        value.get("format") != backend_bindings.EVIDENCE_FORMAT
        or value.get("state") != "absent"
        or value.get("backend_command") is not None
        or value.get("efi_handle_count") != 0
        or value.get("process_record_count") != 0
        or value.get("qcow2_handle_count") != 0
        or not isinstance(observation, dict)
        or observation.get("argv")
        != list(network_ready._lsof_argv(request))
        or observation.get("exit_code") != 1
        or observation.get("timed_out") is not False
        or not _empty_stream(observation.get("stderr"))
        or not _empty_stream(observation.get("stdout"))
    ):
        raise ValueError(
            f"prior-backend-resolution-handles-{label}-invalid"
        )


def _baseline_inventory(
    launch: dict[str, object], request: ReactivationBindingRequest
) -> tuple[start_control.RegisteredVm, ...]:
    value = launch.get("baseline_inventory")
    if not isinstance(value, list):
        raise ValueError("launch-baseline-inventory-invalid")
    inventory: list[start_control.RegisteredVm] = []
    seen: set[str] = set()
    for item in value:
        if not isinstance(item, dict):
            raise ValueError("launch-baseline-inventory-invalid")
        vm_uuid = item.get("uuid")
        name = item.get("name")
        status = item.get("status")
        if (
            not isinstance(vm_uuid, str)
            or not isinstance(name, str)
            or status != "stopped"
            or vm_uuid in seen
        ):
            raise ValueError("launch-baseline-inventory-invalid")
        seen.add(vm_uuid)
        inventory.append(start_control.RegisteredVm(vm_uuid, status, name))
    if (
        len(inventory) + 1 != request.expected_vm_count
        or launch.get("expected_current_vm_count") != request.expected_vm_count
    ):
        raise ValueError("launch-baseline-inventory-count-invalid")
    return tuple(inventory)


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
        raise ValueError("prior-backend-resolution-manifest-unreadable") from exc
    names: list[str] = []
    for line in lines:
        parts = line.split("  ", 1)
        if len(parts) != 2:
            raise ValueError("prior-backend-resolution-manifest-line-invalid")
        names.append(parts[1])
    return tuple(names)
