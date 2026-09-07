#!/usr/bin/env python3
from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
from typing import Protocol

import l6_utm_guest_network_ready as network_ready
import l6_utm_install_artifacts_staged_boot_start_bindings as boot_bindings
import l6_utm_install_artifacts_staged_boot_start_resolution as boot_control
import l6_utm_install_artifacts_staged_reactivation as reactivation_control
import l6_utm_install_artifacts_staged_runtime_resolution as runtime_control
import l6_utm_install_artifacts_staged_runtime_resolution_bindings as runtime_bindings
import l6_utm_launch_transport_v2 as launch_transport
import l6_utm_start_once as start_control


EVIDENCE_FORMAT = (
    "radishlex-linux-l6-utm-install-artifacts-staged-"
    "guest-agent-transport-resolution-v1"
)
CONTROL_RELATIVE_PATH = Path(
    "scripts/linux-product/"
    "l6_utm_install_artifacts_staged_guest_agent_resolution.py"
)
BINDINGS_RELATIVE_PATH = Path(
    "scripts/linux-product/"
    "l6_utm_install_artifacts_staged_guest_agent_bindings.py"
)
PROBE_RELATIVE_PATH = boot_bindings.PROBE_RELATIVE_PATH
REQUIRED_PRIOR_BOOT_START_MANIFEST_SHA256 = (
    "97959c55c05a3562b95412e57ec93249cf6df764700f3ebb49e484cfe3d6c57d"
)
REQUIRED_PRIOR_BOOT_START_REPOSITORY_HEAD = (
    "f97237102d7f120e03d783e707e6171152172639"
)
REQUIRED_PRIOR_BOOT_START_ATTEMPT_ID = (
    "d75818f-v4-install-artifacts-staged-boot-start-20260825-v1"
)
REQUIRED_GUEST_AGENT_ATTEMPT_ID = (
    "d75818f-v4-install-artifacts-staged-guest-agent-20260825-v1"
)
REQUIRED_BACKEND_PID = 39591
REQUIRED_DISCOVERY_STDOUT_SHA256 = (
    "d16eca6cf5b24692e5281c33390a806c2ae606e5601579750d70662624546bd7"
)
REQUIRED_AGENT_UNAVAILABLE_STDERR = (
    "Error from event: The operation couldn’t be completed. "
    "(OSStatus error -2700.)\n"
    "QEMU 客户机代理没有运行或未安装在客户机上。\n"
).encode("utf-8")
REQUIRED_PRIOR_BOOT_START_ENTRY_NAMES = (
    "request.json",
    "binding-preflight.json",
    "source-bundle-preflight.json",
    "target-files-preflight.json",
    "host-process-preflight.json",
    "target-handles-preflight.json",
    "utmctl-list-once.json",
    "inventory-classification.json",
    "host-process-quiescence-001.json",
    "target-handles-quiescence-001.json",
    "host-process-quiescence-002.json",
    "target-handles-quiescence-002.json",
    "host-process-quiescence-003.json",
    "target-handles-quiescence-003.json",
    "target-files-ready.json",
    "foreground-start-once.json",
    "host-process-runtime-001.json",
    "target-handles-runtime-001.json",
    "target-handle-pid-discovery.json",
    "target-handle-pid-confirmation-001.json",
    "host-process-confirmation-001.json",
    "target-handle-pid-confirmation-002.json",
    "host-process-confirmation-002.json",
    "target-handle-pid-confirmation-003.json",
    "host-process-confirmation-003.json",
    "target-files-started.json",
    "guest-control-root-create.json",
    "source-bundle-postflight.json",
    "terminal.json",
)


class GuestAgentBindingRequest(
    boot_bindings.BootStartBindingRequest, Protocol
):
    prior_boot_start_root: Path
    prior_boot_start_manifest_sha256: str
    prior_boot_start_attempt_id: str
    guest_agent_attempt_id: str


@dataclass(frozen=True)
class GuestAgentBinding:
    evidence: dict[str, object]
    upstream: boot_bindings.BootStartBinding
    expected_boot_id_sha256: str
    probe_bytes: bytes
    prior_backend_pid: int


def validate_guest_agent_bindings(
    request: GuestAgentBindingRequest,
) -> GuestAgentBinding:
    if (
        request.boot_start_attempt_id
        != REQUIRED_PRIOR_BOOT_START_ATTEMPT_ID
        or request.prior_boot_start_attempt_id
        != REQUIRED_PRIOR_BOOT_START_ATTEMPT_ID
    ):
        raise ValueError("required-prior-boot-start-attempt-id-mismatch")
    if request.guest_agent_attempt_id != REQUIRED_GUEST_AGENT_ATTEMPT_ID:
        raise ValueError("required-guest-agent-attempt-id-mismatch")

    upstream = boot_bindings.validate_boot_start_bindings(request)
    identities: dict[str, object] = {}
    for relative_path, label in (
        (CONTROL_RELATIVE_PATH, "guest_agent_control"),
        (BINDINGS_RELATIVE_PATH, "guest_agent_bindings"),
        (PROBE_RELATIVE_PATH, "boot_transport_probe"),
    ):
        path = request.repository_root / relative_path
        network_ready._require_committed_regular(path, label)
        identities[f"{label}_sha256"] = network_ready._sha256_file(path)

    root = request.prior_boot_start_root
    manifest = root / "files.sha256"
    if (
        request.prior_boot_start_manifest_sha256
        != REQUIRED_PRIOR_BOOT_START_MANIFEST_SHA256
        or network_ready._sha256_file(manifest)
        != REQUIRED_PRIOR_BOOT_START_MANIFEST_SHA256
    ):
        raise ValueError("prior-boot-start-manifest-identity-invalid")
    entries = start_control._verify_sha256_manifest(root, manifest)
    if entries != len(REQUIRED_PRIOR_BOOT_START_ENTRY_NAMES):
        raise ValueError("prior-boot-start-entry-count-invalid")
    if (
        runtime_bindings._manifest_names(manifest)
        != REQUIRED_PRIOR_BOOT_START_ENTRY_NAMES
    ):
        raise ValueError("prior-boot-start-entry-names-invalid")

    _validate_prior_request(request, root)
    _validate_prior_binding(request, upstream, root)
    confirmation_sha256 = _validate_prior_runtime(request, upstream, root)
    _validate_prior_agent_failure(request, root)
    _validate_prior_terminal(request, root)

    return GuestAgentBinding(
        evidence={
            "format": EVIDENCE_FORMAT,
            **identities,
            "expected_boot_id_sha256": upstream.expected_boot_id_sha256,
            "guest_agent_attempt_id": request.guest_agent_attempt_id,
            "prior_boot_start_agent_readiness": "unavailable-at-first-exec",
            "prior_boot_start_attempt_id": request.prior_boot_start_attempt_id,
            "prior_boot_start_backend_pid": REQUIRED_BACKEND_PID,
            "prior_boot_start_entries_verified": entries,
            "prior_boot_start_guest_probe_invocations": 0,
            "prior_boot_start_handle_confirmation_sha256": confirmation_sha256,
            "prior_boot_start_manifest_sha256": (
                request.prior_boot_start_manifest_sha256
            ),
            "prior_boot_start_outcome": "state-indeterminate",
            "prior_boot_start_result_readback_invocations": 0,
            "repository_clean": True,
            "repository_head": request.expected_repository_head,
            "target_uuid": request.target_uuid,
        },
        upstream=upstream,
        expected_boot_id_sha256=upstream.expected_boot_id_sha256,
        probe_bytes=upstream.probe_bytes,
        prior_backend_pid=REQUIRED_BACKEND_PID,
    )


def _validate_prior_request(
    request: GuestAgentBindingRequest, root: Path
) -> None:
    expected = {
        "authorization": {
            "bounded_target_runtime_observation": True,
            "install_artifacts_staged_boot_start_resolution": True,
            "no_status_resume_business_guest_retry_stop_or_quit": True,
            "one_foreground_start_from_stopped": True,
            "one_potential_backend_reactivation_list": True,
            "one_private_guest_probe_delivery_and_execution": True,
            "two_independent_result_readbacks": True,
        },
        "boot_start_attempt_id": request.prior_boot_start_attempt_id,
        "command_timeout_seconds": 60,
        "expected_repository_head": REQUIRED_PRIOR_BOOT_START_REPOSITORY_HEAD,
        "expected_vm_count": 21,
        "format": boot_bindings.EVIDENCE_FORMAT,
        "guest_control_root": (
            "/var/tmp/radishlex-l6-v4-boot-start-"
            + request.prior_boot_start_attempt_id
        ),
        "identity_observations": 3,
        "poll_interval_seconds": 1,
        "prior_boot_transport_attempt_id": (
            request.prior_boot_transport_attempt_id
        ),
        "prior_boot_transport_manifest_sha256": (
            request.prior_boot_transport_manifest_sha256
        ),
        "prior_runtime_resolution_v2_manifest_sha256": (
            request.prior_runtime_resolution_v2_manifest_sha256
        ),
        "quiescence_observations": 3,
        "runtime_poll_attempts": 60,
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
        "transport_id": launch_transport.TRANSPORT_ID,
        "transport_timeout_seconds": 60,
    }
    if network_ready._read_json(root / "request.json") != expected:
        raise ValueError("prior-boot-start-request-invalid")


def _validate_prior_binding(
    request: GuestAgentBindingRequest,
    upstream: boot_bindings.BootStartBinding,
    root: Path,
) -> None:
    expected = {
        "boot_start_attempt_id": request.prior_boot_start_attempt_id,
        "boot_start_bindings_sha256": (
            "b0f82db6fed45469e2065ea2d8d45d7f01dbd58ab3cf9c51592db9c0b81528a0"
        ),
        "boot_start_control_sha256": (
            "64512ba07ee651cf3310b092e4c96ca98671e5b376e92f36a9ae673c0f837588"
        ),
        "boot_transport_probe_sha256": (
            "27ea59b4061134030f2f88bbe6b21c5787f3a27385aea3f486f2e4881981b8ba"
        ),
        "expected_boot_id_sha256": upstream.expected_boot_id_sha256,
        "expected_vm_count": 21,
        "format": boot_bindings.EVIDENCE_FORMAT,
        "prior_boot_transport_attempt_id": (
            request.prior_boot_transport_attempt_id
        ),
        "prior_boot_transport_entries_verified": 8,
        "prior_boot_transport_guest_probe_invocations": 0,
        "prior_boot_transport_inventory": "target-and-peers-stopped",
        "prior_boot_transport_manifest_sha256": (
            request.prior_boot_transport_manifest_sha256
        ),
        "prior_boot_transport_outcome": "state-indeterminate",
        "prior_boot_transport_result_readback_invocations": 0,
        "repository_clean": True,
        "repository_head": REQUIRED_PRIOR_BOOT_START_REPOSITORY_HEAD,
        "target_uuid": request.target_uuid,
    }
    if network_ready._read_json(root / "binding-preflight.json") != expected:
        raise ValueError("prior-boot-start-binding-invalid")


def _validate_prior_runtime(
    request: GuestAgentBindingRequest,
    upstream: boot_bindings.BootStartBinding,
    root: Path,
) -> str:
    list_value = network_ready._read_json(root / "utmctl-list-once.json")
    list_observation = (
        reactivation_control.launch_bindings._command_observation_from_json(
            list_value, "prior-boot-start-list"
        )
    )
    inventory = start_control.parse_utmctl_list(list_observation)
    baseline = upstream.upstream.upstream.upstream.baseline_inventory
    if (
        reactivation_control._require_inventory(inventory, baseline, request)
        != "stopped"
    ):
        raise ValueError("prior-boot-start-inventory-invalid")
    classification = network_ready._read_json(
        root / "inventory-classification.json"
    )
    if classification != {
        "format": boot_bindings.EVIDENCE_FORMAT,
        "other_registered_vm_count": 20,
        "other_registered_vms": "all-stopped",
        "registered_vm_count": 21,
        "target_registered_status": "stopped",
        "target_uuid": request.target_uuid,
    }:
        raise ValueError("prior-boot-start-inventory-classification-invalid")

    for name in (
        "host-process-preflight.json",
        "host-process-quiescence-001.json",
        "host-process-quiescence-002.json",
        "host-process-quiescence-003.json",
        "host-process-runtime-001.json",
        "host-process-confirmation-001.json",
        "host-process-confirmation-002.json",
        "host-process-confirmation-003.json",
    ):
        runtime_bindings._require_process_state(
            root / name,
            0,
            expected_format=runtime_control.EVIDENCE_FORMAT,
            error_label="prior-boot-start",
        )

    start_value = network_ready._read_json(root / "foreground-start-once.json")
    start_observation = (
        reactivation_control.launch_bindings._command_observation_from_json(
            start_value, "prior-boot-start-foreground-start"
        )
    )
    if start_observation.argv != launch_transport.transport_argv(request):
        raise ValueError("prior-boot-start-foreground-argv-invalid")
    start_control._require_successful_observation(
        start_observation, "prior-boot-start-foreground-start"
    )

    discovery = network_ready._read_json(
        root / "target-handle-pid-discovery.json"
    )
    discovery_sha256 = _require_prior_handle_state(
        discovery,
        network_ready._lsof_argv(request),
        REQUIRED_BACKEND_PID,
    )
    if discovery_sha256 != REQUIRED_DISCOVERY_STDOUT_SHA256:
        raise ValueError("prior-boot-start-discovery-hash-invalid")

    targeted_argv = runtime_control.targeted_lsof_argv(
        request, REQUIRED_BACKEND_PID
    )
    confirmation_hashes = []
    for index in range(1, 4):
        value = network_ready._read_json(
            root / f"target-handle-pid-confirmation-{index:03d}.json"
        )
        confirmation_hashes.append(
            _require_prior_handle_state(
                value, targeted_argv, REQUIRED_BACKEND_PID
            )
        )
    if len(set(confirmation_hashes)) != 1:
        raise ValueError("prior-boot-start-confirmation-hash-drift")
    return confirmation_hashes[0]


def _require_prior_handle_state(
    value: dict[str, object],
    expected_argv: tuple[str, ...],
    expected_pid: int,
) -> str:
    observation = value.get("observation")
    if (
        value.get("format") != runtime_control.EVIDENCE_FORMAT
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
        raise ValueError("prior-boot-start-handle-semantics-invalid")
    stdout = observation.get("stdout")
    if not isinstance(stdout, dict):
        raise ValueError("prior-boot-start-handle-stdout-invalid")
    digest = stdout.get("sha256")
    if not isinstance(digest, str):
        raise ValueError("prior-boot-start-handle-hash-invalid")
    return digest


def _validate_prior_agent_failure(
    request: GuestAgentBindingRequest, root: Path
) -> None:
    value = network_ready._read_json(root / "guest-control-root-create.json")
    observation = (
        reactivation_control.launch_bindings._command_observation_from_json(
            value, "prior-boot-start-guest-root"
        )
    )
    expected_argv = (
        "utmctl",
        "exec",
        request.target_uuid,
        "--cmd",
        "/bin/mkdir",
        "-m",
        "0700",
        "/var/tmp/radishlex-l6-v4-boot-start-"
        + request.prior_boot_start_attempt_id,
    )
    if (
        observation.argv != expected_argv
        or observation.exit_code != 0
        or observation.timed_out
        or observation.stdout.total_bytes != 0
        or observation.stderr.prefix != REQUIRED_AGENT_UNAVAILABLE_STDERR
        or observation.stderr.total_bytes
        != len(REQUIRED_AGENT_UNAVAILABLE_STDERR)
        or observation.stderr.truncated
    ):
        raise ValueError("prior-boot-start-agent-failure-invalid")
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
        raise ValueError("prior-boot-start-source-postflight-invalid")


def _validate_prior_terminal(
    request: GuestAgentBindingRequest, root: Path
) -> None:
    expected = {
        "automatic_cleanup": "not-performed",
        "automatic_quit": "not-performed",
        "automatic_retry": "not-performed",
        "automatic_stop": "not-performed",
        "backend_pid": REQUIRED_BACKEND_PID,
        "boot_classification": "not-generated",
        "boot_start_attempt_id": request.prior_boot_start_attempt_id,
        "business_guest_action": "not-performed",
        "file_pull_invocations": 0,
        "file_push_invocations": 0,
        "foreground_start_invocations": 1,
        "format": boot_bindings.EVIDENCE_FORMAT,
        "guest_exec_invocations": 1,
        "guest_probe_invocations": 0,
        "identity_observation_count": 3,
        "inventory_probe_invocations": 1,
        "maintenance_resume_invocations": 0,
        "observed_boot_id_sha256": None,
        "operation_id": "not-read-or-generated",
        "outcome": "state-indeterminate",
        "plain_utmctl_list": "attempted-once-as-potential-backend-reactivation",
        "plain_utmctl_start": "not-performed",
        "plain_utmctl_status": "not-performed",
        "reason": (
            "guest-control-root-create:"
            "guest-control-root-create-stderr-not-empty"
        ),
        "result_readback_invocations": 0,
        "runtime_poll_count": 1,
        "target_handles_terminal": "present",
        "target_name": request.target_name,
        "target_uuid": request.target_uuid,
        "terminal_relevant_host_process_count": 0,
        "transaction": "artifacts-staged-preserved-no-resume",
    }
    if network_ready._read_json(root / "terminal.json") != expected:
        raise ValueError("prior-boot-start-terminal-invalid")
