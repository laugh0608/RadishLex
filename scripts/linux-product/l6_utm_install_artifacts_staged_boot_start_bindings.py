#!/usr/bin/env python3
from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
from typing import Protocol

import l6_utm_guest_network_ready as network_ready
import l6_utm_install_artifacts_staged_boot_transport_bindings as boot_bindings
import l6_utm_install_artifacts_staged_reactivation as reactivation_control
import l6_utm_install_artifacts_staged_runtime_resolution_bindings as runtime_bindings
import l6_utm_start_once as start_control


EVIDENCE_FORMAT = (
    "radishlex-linux-l6-utm-install-artifacts-staged-"
    "boot-start-resolution-v1"
)
CONTROL_RELATIVE_PATH = Path(
    "scripts/linux-product/"
    "l6_utm_install_artifacts_staged_boot_start_resolution.py"
)
BINDINGS_RELATIVE_PATH = Path(
    "scripts/linux-product/"
    "l6_utm_install_artifacts_staged_boot_start_bindings.py"
)
PROBE_RELATIVE_PATH = boot_bindings.PROBE_RELATIVE_PATH
REQUIRED_PRIOR_BOOT_TRANSPORT_MANIFEST_SHA256 = (
    "6a1ad09f1db4b5e0b5d9edb9c4a9d1102fa8dfbf333a819f9961e759352e5140"
)
REQUIRED_PRIOR_BOOT_TRANSPORT_REPOSITORY_HEAD = (
    "8f59825c428d42d49c0c9e5aa28d6c8b0d58e997"
)
REQUIRED_PRIOR_BOOT_TRANSPORT_ATTEMPT_ID = (
    "d75818f-v4-install-artifacts-staged-boot-transport-20260825-v1"
)
REQUIRED_BOOT_START_ATTEMPT_ID = (
    "d75818f-v4-install-artifacts-staged-boot-start-20260825-v1"
)
REQUIRED_PRIOR_BOOT_TRANSPORT_ENTRY_NAMES = (
    "request.json",
    "binding-preflight.json",
    "source-bundle-preflight.json",
    "target-files-preflight.json",
    "host-process-preflight.json",
    "utmctl-list-once.json",
    "source-bundle-postflight.json",
    "terminal.json",
)


class BootStartBindingRequest(
    boot_bindings.BootTransportBindingRequest, Protocol
):
    prior_boot_transport_root: Path
    prior_boot_transport_manifest_sha256: str
    prior_boot_transport_attempt_id: str
    boot_start_attempt_id: str


@dataclass(frozen=True)
class BootStartBinding:
    evidence: dict[str, object]
    upstream: boot_bindings.BootTransportBinding
    expected_boot_id_sha256: str
    probe_bytes: bytes


def validate_boot_start_bindings(
    request: BootStartBindingRequest,
) -> BootStartBinding:
    if (
        request.boot_transport_attempt_id
        != REQUIRED_PRIOR_BOOT_TRANSPORT_ATTEMPT_ID
        or request.prior_boot_transport_attempt_id
        != REQUIRED_PRIOR_BOOT_TRANSPORT_ATTEMPT_ID
    ):
        raise ValueError("required-prior-boot-transport-attempt-id-mismatch")
    if request.boot_start_attempt_id != REQUIRED_BOOT_START_ATTEMPT_ID:
        raise ValueError("required-boot-start-attempt-id-mismatch")

    upstream = boot_bindings.validate_boot_transport_bindings(request)
    identities: dict[str, object] = {}
    for relative_path, label in (
        (CONTROL_RELATIVE_PATH, "boot_start_control"),
        (BINDINGS_RELATIVE_PATH, "boot_start_bindings"),
        (PROBE_RELATIVE_PATH, "boot_transport_probe"),
    ):
        path = request.repository_root / relative_path
        network_ready._require_committed_regular(path, label)
        identities[f"{label}_sha256"] = network_ready._sha256_file(path)

    manifest = request.prior_boot_transport_root / "files.sha256"
    if (
        request.prior_boot_transport_manifest_sha256
        != REQUIRED_PRIOR_BOOT_TRANSPORT_MANIFEST_SHA256
        or network_ready._sha256_file(manifest)
        != REQUIRED_PRIOR_BOOT_TRANSPORT_MANIFEST_SHA256
    ):
        raise ValueError("prior-boot-transport-manifest-identity-invalid")
    entries = start_control._verify_sha256_manifest(
        request.prior_boot_transport_root, manifest
    )
    if entries != len(REQUIRED_PRIOR_BOOT_TRANSPORT_ENTRY_NAMES):
        raise ValueError("prior-boot-transport-entry-count-invalid")
    if (
        runtime_bindings._manifest_names(manifest)
        != REQUIRED_PRIOR_BOOT_TRANSPORT_ENTRY_NAMES
    ):
        raise ValueError("prior-boot-transport-entry-names-invalid")
    _validate_prior_boot_transport(request, upstream)

    return BootStartBinding(
        evidence={
            "format": EVIDENCE_FORMAT,
            **identities,
            "boot_start_attempt_id": request.boot_start_attempt_id,
            "expected_boot_id_sha256": upstream.expected_boot_id_sha256,
            "expected_vm_count": request.expected_vm_count,
            "prior_boot_transport_attempt_id": (
                request.prior_boot_transport_attempt_id
            ),
            "prior_boot_transport_entries_verified": entries,
            "prior_boot_transport_guest_probe_invocations": 0,
            "prior_boot_transport_inventory": "target-and-peers-stopped",
            "prior_boot_transport_manifest_sha256": (
                request.prior_boot_transport_manifest_sha256
            ),
            "prior_boot_transport_outcome": "state-indeterminate",
            "prior_boot_transport_result_readback_invocations": 0,
            "repository_clean": True,
            "repository_head": request.expected_repository_head,
            "target_uuid": request.target_uuid,
        },
        upstream=upstream,
        expected_boot_id_sha256=upstream.expected_boot_id_sha256,
        probe_bytes=upstream.probe_bytes,
    )


def _validate_prior_boot_transport(
    request: BootStartBindingRequest,
    upstream: boot_bindings.BootTransportBinding,
) -> None:
    root = request.prior_boot_transport_root
    _validate_prior_request(request, root)
    _validate_prior_binding(request, upstream, root)

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
        raise ValueError("prior-boot-transport-source-invalid")

    target = network_ready._read_json(root / "target-files-preflight.json")
    if (
        target.get("format") != network_ready.EVIDENCE_FORMAT
        or target.get("target_config_sha256")
        != network_ready.REQUIRED_TARGET_CONFIG_SHA256
        or target.get("target_package_name")
        != request.target_package_path.name
        or target.get("target_package_path_sha256")
        != network_ready._sha256_text(str(request.target_package_path))
    ):
        raise ValueError("prior-boot-transport-target-invalid")

    runtime_bindings._require_process_state(
        root / "host-process-preflight.json",
        0,
        expected_format=runtime_bindings.EVIDENCE_FORMAT,
        error_label="prior-boot-transport",
    )
    list_value = network_ready._read_json(root / "utmctl-list-once.json")
    list_observation = (
        reactivation_control.launch_bindings._command_observation_from_json(
            list_value, "prior-boot-transport-list"
        )
    )
    inventory = start_control.parse_utmctl_list(list_observation)
    baseline = upstream.upstream.upstream.baseline_inventory
    if (
        reactivation_control._require_inventory(
            inventory, baseline, request
        )
        != "stopped"
    ):
        raise ValueError("prior-boot-transport-inventory-invalid")

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
        raise ValueError("prior-boot-transport-source-postflight-invalid")
    _validate_prior_terminal(request, root)


def _validate_prior_request(
    request: BootStartBindingRequest, root: Path
) -> None:
    expected = {
        "authorization": {
            "install_artifacts_staged_boot_transport_resolution": True,
            "no_start_status_resume_business_guest_retry_stop_or_quit": True,
            "one_potential_backend_reactivation_list": True,
            "one_private_guest_probe_delivery_and_execution": True,
            "stable_target_handle_pid_observations": True,
            "two_independent_result_readbacks": True,
        },
        "boot_transport_attempt_id": request.prior_boot_transport_attempt_id,
        "command_timeout_seconds": 60,
        "expected_repository_head": (
            REQUIRED_PRIOR_BOOT_TRANSPORT_REPOSITORY_HEAD
        ),
        "expected_vm_count": 21,
        "format": boot_bindings.EVIDENCE_FORMAT,
        "guest_control_root": (
            "/var/tmp/radishlex-l6-v4-boot-transport-"
            + request.prior_boot_transport_attempt_id
        ),
        "identity_observations": 3,
        "poll_interval_seconds": 1,
        "prior_reactivation_manifest_sha256": (
            request.prior_reactivation_manifest_sha256
        ),
        "prior_runtime_resolution_manifest_sha256": (
            request.prior_runtime_resolution_manifest_sha256
        ),
        "prior_runtime_resolution_v2_attempt_id": (
            request.prior_runtime_resolution_v2_attempt_id
        ),
        "prior_runtime_resolution_v2_manifest_sha256": (
            request.prior_runtime_resolution_v2_manifest_sha256
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
    }
    if network_ready._read_json(root / "request.json") != expected:
        raise ValueError("prior-boot-transport-request-invalid")


def _validate_prior_binding(
    request: BootStartBindingRequest,
    upstream: boot_bindings.BootTransportBinding,
    root: Path,
) -> None:
    expected = {
        "boot_transport_attempt_id": request.prior_boot_transport_attempt_id,
        "boot_transport_bindings_sha256": (
            "29113c603638d8c8b5f7a8060f76d8eb94cc116d908af4e0e3f54e03600b214e"
        ),
        "boot_transport_control_sha256": (
            "5af3e7201e1106943bc0bdfc5cce02b69bc8eb20e82fa2034357513ada2ee70b"
        ),
        "boot_transport_probe_sha256": (
            "27ea59b4061134030f2f88bbe6b21c5787f3a27385aea3f486f2e4881981b8ba"
        ),
        "expected_boot_id_sha256": upstream.expected_boot_id_sha256,
        "expected_vm_count": 21,
        "format": boot_bindings.EVIDENCE_FORMAT,
        "prior_runtime_resolution_v1_entries_verified": 17,
        "prior_runtime_resolution_v1_manifest_sha256": (
            request.prior_runtime_resolution_manifest_sha256
        ),
        "prior_runtime_resolution_v2_attempt_id": (
            request.prior_runtime_resolution_v2_attempt_id
        ),
        "prior_runtime_resolution_v2_classification": "not-generated",
        "prior_runtime_resolution_v2_entries_verified": 18,
        "prior_runtime_resolution_v2_exec_exit_code": 0,
        "prior_runtime_resolution_v2_manifest_sha256": (
            request.prior_runtime_resolution_v2_manifest_sha256
        ),
        "prior_runtime_resolution_v2_observation": "persisted-before-parse",
        "prior_runtime_resolution_v2_outcome": "state-indeterminate",
        "prior_runtime_resolution_v2_stdout_bytes": 0,
        "reactivation_entries_verified": 94,
        "repository_clean": True,
        "repository_head": REQUIRED_PRIOR_BOOT_TRANSPORT_REPOSITORY_HEAD,
        "target_uuid": request.target_uuid,
    }
    if network_ready._read_json(root / "binding-preflight.json") != expected:
        raise ValueError("prior-boot-transport-binding-invalid")


def _validate_prior_terminal(
    request: BootStartBindingRequest, root: Path
) -> None:
    expected = {
        "automatic_cleanup": "not-performed",
        "automatic_quit": "not-performed",
        "automatic_retry": "not-performed",
        "automatic_stop": "not-performed",
        "backend_pid": None,
        "boot_classification": "not-generated",
        "boot_transport_attempt_id": request.prior_boot_transport_attempt_id,
        "business_guest_action": "not-performed",
        "file_pull_invocations": 0,
        "file_push_invocations": 0,
        "foreground_start_invocations": 0,
        "format": boot_bindings.EVIDENCE_FORMAT,
        "guest_exec_invocations": 0,
        "guest_probe_invocations": 0,
        "identity_observation_count": 0,
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
            "utmctl-list-once:"
            "target-not-registered-started-for-boot-transport"
        ),
        "result_readback_invocations": 0,
        "target_handles_terminal": "not-observed",
        "target_name": request.target_name,
        "target_uuid": request.target_uuid,
        "terminal_relevant_host_process_count": 0,
        "transaction": "artifacts-staged-preserved-no-resume",
    }
    if network_ready._read_json(root / "terminal.json") != expected:
        raise ValueError("prior-boot-transport-terminal-invalid")
