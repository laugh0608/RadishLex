#!/usr/bin/env python3
from __future__ import annotations

import re
from dataclasses import dataclass
from pathlib import Path
from typing import Protocol

import l6_utm_guest_network_ready as network_ready
import l6_utm_install_artifacts_staged_reactivation as reactivation_control
import l6_utm_install_artifacts_staged_runtime_resolution as runtime_control
import l6_utm_install_artifacts_staged_runtime_resolution_bindings as runtime_bindings
import l6_utm_start_once as start_control


EVIDENCE_FORMAT = (
    "radishlex-linux-l6-utm-install-artifacts-staged-"
    "boot-transport-resolution-v1"
)
CONTROL_RELATIVE_PATH = Path(
    "scripts/linux-product/"
    "l6_utm_install_artifacts_staged_boot_transport_resolution.py"
)
BINDINGS_RELATIVE_PATH = Path(
    "scripts/linux-product/"
    "l6_utm_install_artifacts_staged_boot_transport_bindings.py"
)
PROBE_RELATIVE_PATH = Path(
    "scripts/linux-product/l6_v4_boot_transport_probe.py"
)
REQUIRED_PRIOR_RUNTIME_RESOLUTION_V2_MANIFEST_SHA256 = (
    "e2e4123578bf51d28b4dad6257773fcb9cb2d7158c34ad9253ba55db4e4ec49e"
)
REQUIRED_PRIOR_RUNTIME_RESOLUTION_V2_REPOSITORY_HEAD = (
    "61b24a97aeed12280a04bd30949ab65e1bfb2039"
)
REQUIRED_PRIOR_RUNTIME_RESOLUTION_V2_ATTEMPT_ID = (
    "d75818f-v4-install-artifacts-staged-runtime-resolution-20260824-v2"
)
REQUIRED_BOOT_TRANSPORT_ATTEMPT_ID = (
    "d75818f-v4-install-artifacts-staged-boot-transport-20260825-v1"
)
HEX_64 = re.compile(r"[0-9a-f]{64}")
EMPTY_SHA256 = (
    "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
)
REQUIRED_PRIOR_RUNTIME_RESOLUTION_V2_ENTRY_NAMES = (
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
    "guest-boot-id-hash-observation.json",
    "source-bundle-postflight.json",
    "terminal.json",
)


class BootTransportBindingRequest(
    runtime_bindings.RuntimeResolutionBindingRequest, Protocol
):
    prior_runtime_resolution_v2_root: Path
    prior_runtime_resolution_v2_manifest_sha256: str
    prior_runtime_resolution_v2_attempt_id: str
    boot_transport_attempt_id: str


@dataclass(frozen=True)
class BootTransportBinding:
    evidence: dict[str, object]
    upstream: runtime_bindings.RuntimeResolutionBinding
    expected_boot_id_sha256: str
    probe_bytes: bytes


def validate_boot_transport_bindings(
    request: BootTransportBindingRequest,
) -> BootTransportBinding:
    if (
        request.prior_runtime_resolution_v2_attempt_id
        != REQUIRED_PRIOR_RUNTIME_RESOLUTION_V2_ATTEMPT_ID
        or request.runtime_resolution_attempt_id
        != REQUIRED_PRIOR_RUNTIME_RESOLUTION_V2_ATTEMPT_ID
    ):
        raise ValueError(
            "required-prior-runtime-resolution-v2-attempt-id-mismatch"
        )
    if request.boot_transport_attempt_id != REQUIRED_BOOT_TRANSPORT_ATTEMPT_ID:
        raise ValueError("required-boot-transport-attempt-id-mismatch")

    upstream = runtime_bindings.validate_runtime_resolution_bindings(request)
    identities: dict[str, object] = {}
    for relative_path, label in (
        (CONTROL_RELATIVE_PATH, "boot_transport_control"),
        (BINDINGS_RELATIVE_PATH, "boot_transport_bindings"),
        (PROBE_RELATIVE_PATH, "boot_transport_probe"),
    ):
        path = request.repository_root / relative_path
        network_ready._require_committed_regular(path, label)
        identities[f"{label}_sha256"] = network_ready._sha256_file(path)

    manifest = request.prior_runtime_resolution_v2_root / "files.sha256"
    if (
        request.prior_runtime_resolution_v2_manifest_sha256
        != REQUIRED_PRIOR_RUNTIME_RESOLUTION_V2_MANIFEST_SHA256
        or network_ready._sha256_file(manifest)
        != REQUIRED_PRIOR_RUNTIME_RESOLUTION_V2_MANIFEST_SHA256
    ):
        raise ValueError(
            "prior-runtime-resolution-v2-manifest-identity-invalid"
        )
    entries = start_control._verify_sha256_manifest(
        request.prior_runtime_resolution_v2_root, manifest
    )
    if entries != len(REQUIRED_PRIOR_RUNTIME_RESOLUTION_V2_ENTRY_NAMES):
        raise ValueError("prior-runtime-resolution-v2-entry-count-invalid")
    if (
        runtime_bindings._manifest_names(manifest)
        != REQUIRED_PRIOR_RUNTIME_RESOLUTION_V2_ENTRY_NAMES
    ):
        raise ValueError("prior-runtime-resolution-v2-entry-names-invalid")
    _validate_prior_runtime_resolution_v2_semantics(request, upstream)

    expected_boot = upstream.expected_boot_id_sha256
    if not HEX_64.fullmatch(expected_boot):
        raise ValueError("expected-boot-id-sha256-invalid")
    probe_bytes = (request.repository_root / PROBE_RELATIVE_PATH).read_bytes()
    return BootTransportBinding(
        evidence={
            "format": EVIDENCE_FORMAT,
            **identities,
            "boot_transport_attempt_id": request.boot_transport_attempt_id,
            "expected_boot_id_sha256": expected_boot,
            "expected_vm_count": request.expected_vm_count,
            "prior_runtime_resolution_v1_entries_verified": (
                upstream.evidence[
                    "prior_runtime_resolution_entries_verified"
                ]
            ),
            "prior_runtime_resolution_v1_manifest_sha256": (
                request.prior_runtime_resolution_manifest_sha256
            ),
            "prior_runtime_resolution_v2_attempt_id": (
                request.prior_runtime_resolution_v2_attempt_id
            ),
            "prior_runtime_resolution_v2_classification": "not-generated",
            "prior_runtime_resolution_v2_entries_verified": entries,
            "prior_runtime_resolution_v2_exec_exit_code": 0,
            "prior_runtime_resolution_v2_manifest_sha256": (
                request.prior_runtime_resolution_v2_manifest_sha256
            ),
            "prior_runtime_resolution_v2_observation": (
                "persisted-before-parse"
            ),
            "prior_runtime_resolution_v2_outcome": "state-indeterminate",
            "prior_runtime_resolution_v2_stdout_bytes": 0,
            "reactivation_entries_verified": (
                upstream.evidence["prior_reactivation_entries_verified"]
            ),
            "repository_clean": True,
            "repository_head": request.expected_repository_head,
            "target_uuid": request.target_uuid,
        },
        upstream=upstream,
        expected_boot_id_sha256=expected_boot,
        probe_bytes=probe_bytes,
    )


def _validate_prior_runtime_resolution_v2_semantics(
    request: BootTransportBindingRequest,
    upstream: runtime_bindings.RuntimeResolutionBinding,
) -> None:
    root = request.prior_runtime_resolution_v2_root
    _validate_prior_v2_request(request, root)
    _validate_prior_v2_binding(request, upstream, root)

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
        raise ValueError("prior-runtime-resolution-v2-source-invalid")

    target_preflight = network_ready._read_json(
        root / "target-files-preflight.json"
    )
    target_ready = network_ready._read_json(root / "target-files-ready.json")
    if target_preflight != target_ready:
        raise ValueError("prior-runtime-resolution-v2-target-drift")
    if (
        target_preflight.get("format") != network_ready.EVIDENCE_FORMAT
        or target_preflight.get("target_config_sha256")
        != network_ready.REQUIRED_TARGET_CONFIG_SHA256
        or target_preflight.get("target_package_name")
        != request.target_package_path.name
        or target_preflight.get("target_package_path_sha256")
        != network_ready._sha256_text(str(request.target_package_path))
    ):
        raise ValueError("prior-runtime-resolution-v2-target-invalid")

    runtime_bindings._require_process_state(
        root / "host-process-preflight.json",
        0,
        expected_format=runtime_bindings.EVIDENCE_FORMAT,
        error_label="prior-runtime-resolution-v2",
    )
    list_value = network_ready._read_json(root / "utmctl-list-once.json")
    list_observation = (
        reactivation_control.launch_bindings._command_observation_from_json(
            list_value, "prior-runtime-resolution-v2-list"
        )
    )
    inventory = start_control.parse_utmctl_list(list_observation)
    if (
        reactivation_control._require_inventory(
            inventory, upstream.upstream.baseline_inventory, request
        )
        != "started"
    ):
        raise ValueError("prior-runtime-resolution-v2-inventory-invalid")
    if network_ready._read_json(root / "inventory-classification.json") != {
        "format": runtime_bindings.EVIDENCE_FORMAT,
        "other_registered_vm_count": 20,
        "other_registered_vms": "all-stopped",
        "registered_vm_count": 21,
        "target_registered_status": "started",
        "target_uuid": request.target_uuid,
    }:
        raise ValueError(
            "prior-runtime-resolution-v2-inventory-classification-invalid"
        )

    discovery = network_ready._read_json(
        root / "target-handle-pid-discovery.json"
    )
    backend_pid = discovery.get("backend_pid")
    if not isinstance(backend_pid, int) or backend_pid <= 1:
        raise ValueError("prior-runtime-resolution-v2-backend-pid-invalid")
    handle_hash = _require_v2_runtime_handle_state(
        discovery, network_ready._lsof_argv(request), backend_pid
    )
    targeted_argv = runtime_control.targeted_lsof_argv(request, backend_pid)
    for index in range(1, 4):
        confirmation = network_ready._read_json(
            root / f"target-handle-pid-confirmation-{index:03d}.json"
        )
        if (
            _require_v2_runtime_handle_state(
                confirmation, targeted_argv, backend_pid
            )
            != handle_hash
        ):
            raise ValueError(
                "prior-runtime-resolution-v2-handle-output-drift"
            )
        runtime_bindings._require_process_state(
            root / f"host-process-confirmation-{index:03d}.json",
            0,
            expected_format=runtime_bindings.EVIDENCE_FORMAT,
            error_label="prior-runtime-resolution-v2",
        )

    _validate_empty_boot_observation(request, root)
    if (root / "guest-boot-id-hash-classification.json").exists():
        raise ValueError(
            "prior-runtime-resolution-v2-classification-unexpected"
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
        raise ValueError("prior-runtime-resolution-v2-source-postflight-invalid")
    _validate_prior_v2_terminal(request, root, backend_pid)


def _validate_prior_v2_request(
    request: BootTransportBindingRequest, root: Path
) -> None:
    prior_request = network_ready._read_json(root / "request.json")
    expected = {
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
            REQUIRED_PRIOR_RUNTIME_RESOLUTION_V2_REPOSITORY_HEAD
        ),
        "expected_vm_count": 21,
        "format": runtime_bindings.EVIDENCE_FORMAT,
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
        "prior_runtime_resolution_attempt_id": (
            request.prior_runtime_resolution_attempt_id
        ),
        "prior_runtime_resolution_manifest_sha256": (
            request.prior_runtime_resolution_manifest_sha256
        ),
        "prior_transfer_manifest_sha256": (
            request.prior_transfer_manifest_sha256
        ),
        "prior_v7_manifest_sha256": request.prior_v7_manifest_sha256,
        "reactivation_attempt_id": request.reactivation_attempt_id,
        "resolution_attempt_id": request.resolution_attempt_id,
        "resume_attempt_id": request.resume_attempt_id,
        "runtime_resolution_attempt_id": (
            request.prior_runtime_resolution_v2_attempt_id
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
    if prior_request != expected:
        raise ValueError("prior-runtime-resolution-v2-request-invalid")


def _validate_prior_v2_binding(
    request: BootTransportBindingRequest,
    upstream: runtime_bindings.RuntimeResolutionBinding,
    root: Path,
) -> None:
    prior_binding = network_ready._read_json(root / "binding-preflight.json")
    expected = {
        "expected_boot_id_sha256": upstream.expected_boot_id_sha256,
        "expected_vm_count": 21,
        "format": runtime_bindings.EVIDENCE_FORMAT,
        "prior_reactivation_entries_verified": 94,
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
        "prior_runtime_resolution_entries_verified": 17,
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
        "repository_head": REQUIRED_PRIOR_RUNTIME_RESOLUTION_V2_REPOSITORY_HEAD,
        "runtime_resolution_attempt_id": (
            request.prior_runtime_resolution_v2_attempt_id
        ),
        "runtime_resolution_bindings_sha256": (
            "948520116dd014448c7848c40c44dc028bcc34a59ad4b4c2ffea34f4c3cc854d"
        ),
        "runtime_resolution_control_sha256": (
            "61d878731b1fd1199cb7838068d19b66b6fc7faac09e2cca30f8e35c36d71423"
        ),
        "target_uuid": request.target_uuid,
    }
    if prior_binding != expected:
        raise ValueError("prior-runtime-resolution-v2-binding-invalid")


def _validate_empty_boot_observation(
    request: BootTransportBindingRequest, root: Path
) -> None:
    value = network_ready._read_json(
        root / "guest-boot-id-hash-observation.json"
    )
    expected_observation = {
        "argv": list(reactivation_control.boot_id_hash_argv(request)),
        "exit_code": 0,
        "stderr": {
            "sha256": EMPTY_SHA256,
            "total_bytes": 0,
            "truncated": False,
        },
        "stdout": {
            "sha256": EMPTY_SHA256,
            "total_bytes": 0,
            "truncated": False,
        },
        "timed_out": False,
    }
    if value != {
        "format": runtime_bindings.EVIDENCE_FORMAT,
        "invocation_count": 1,
        "observation": expected_observation,
        "raw_output_persisted": False,
    }:
        raise ValueError(
            "prior-runtime-resolution-v2-empty-observation-invalid"
        )


def _validate_prior_v2_terminal(
    request: BootTransportBindingRequest, root: Path, backend_pid: int
) -> None:
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
        "format": runtime_bindings.EVIDENCE_FORMAT,
        "guest_boot_classification": "not-performed",
        "guest_boot_hash_invocations": 1,
        "guest_boot_observation": "persisted-before-parse",
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
            request.prior_runtime_resolution_v2_attempt_id
        ),
        "target_handles_terminal": "present",
        "target_name": request.target_name,
        "target_uuid": request.target_uuid,
        "terminal_relevant_host_process_count": 0,
        "transaction": "artifacts-staged-preserved-no-resume",
    }:
        raise ValueError("prior-runtime-resolution-v2-terminal-invalid")


def _require_v2_runtime_handle_state(
    value: dict[str, object],
    expected_argv: tuple[str, ...],
    expected_pid: int,
) -> str:
    observation = value.get("observation")
    if (
        value.get("format") != runtime_bindings.EVIDENCE_FORMAT
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
        or not runtime_bindings._empty_stream(observation.get("stderr"))
        or not runtime_bindings._complete_stream(observation.get("stdout"))
    ):
        raise ValueError(
            "prior-runtime-resolution-v2-handle-semantics-invalid"
        )
    stdout = observation.get("stdout")
    if not isinstance(stdout, dict):
        raise ValueError("prior-runtime-resolution-v2-handle-stdout-invalid")
    digest = stdout.get("sha256")
    if not isinstance(digest, str) or not HEX_64.fullmatch(digest):
        raise ValueError("prior-runtime-resolution-v2-handle-hash-invalid")
    return digest
