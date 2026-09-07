#!/usr/bin/env python3
from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
from typing import Protocol

import l6_utm_guest_network_ready as network_ready
import l6_utm_install_artifacts_staged_boot_classification_bindings as classification_bindings
import l6_utm_install_artifacts_staged_boot_classification_resolution as classification_control
import l6_utm_install_artifacts_staged_guest_agent_bindings as guest_bindings
import l6_utm_install_artifacts_staged_guest_agent_resolution as guest_control
import l6_utm_install_artifacts_staged_runtime_resolution as runtime_control
import l6_utm_install_artifacts_staged_runtime_resolution_bindings as runtime_bindings
import l6_utm_launch_transport_bindings as launch_bindings
import l6_utm_launch_transport_v2 as launch_transport
import l6_utm_start_once as start_control
import l6_v4_boot_transport_probe as guest_probe


EVIDENCE_FORMAT = (
    "radishlex-linux-l6-utm-install-artifacts-staged-"
    "boot-classification-result-binding-v1"
)
BINDINGS_RELATIVE_PATH = Path(
    "scripts/linux-product/"
    "l6_utm_install_artifacts_staged_boot_classification_result_bindings.py"
)
PROBE_RELATIVE_PATH = classification_bindings.PROBE_RELATIVE_PATH
REQUIRED_PRIOR_MANIFEST_SHA256 = (
    "a17920bd9a38c455f1151e5873f0b4261aa25237c64cf56cc8ef73e554830e17"
)
REQUIRED_PRIOR_REPOSITORY_HEAD = "8a04d02bd09f685cb4491baf7419235cd8c7808c"
REQUIRED_PRIOR_ATTEMPT_ID = (
    "d75818f-v4-install-artifacts-staged-boot-classification-20260826-v1"
)
REQUIRED_EXPECTED_BOOT_ID_SHA256 = (
    "18f1ba063ecd3087a5624e6ae52a54624540a972df33cca00a34bca4ec00022a"
)
REQUIRED_OBSERVED_BOOT_ID_SHA256 = (
    "b757c8fcc3c47b85b04be60ffc561e67cfe323d73ffe7981f6000d0c0fa44758"
)
REQUIRED_BACKEND_PID = 92422
REQUIRED_HANDLE_STDOUT_SHA256 = (
    "0f5a2c7fbaff52d71998816f2bcc457d0074bf7c0b8102289d2c1b78dc700856"
)
REQUIRED_PROBE_SHA256 = (
    "f15f1b4bd2d13fd7a1885d3fbc06a117365e6b366af3fab99312f8d556b08813"
)
REQUIRED_PROBE_SIZE = 8404
REQUIRED_ENTRY_NAMES = (
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
    "guest-agent-readiness-001.json",
    "target-handle-readiness-001.json",
    "host-process-readiness-001.json",
    "guest-agent-readiness-002.json",
    "target-handle-readiness-002.json",
    "host-process-readiness-002.json",
    "guest-agent-readiness-003.json",
    "target-handle-readiness-003.json",
    "host-process-readiness-003.json",
    "guest-agent-readiness-004.json",
    "target-handle-readiness-004.json",
    "host-process-readiness-004.json",
    "guest-agent-readiness-005.json",
    "target-handle-readiness-005.json",
    "host-process-readiness-005.json",
    "guest-agent-readiness-006.json",
    "target-handle-readiness-006.json",
    "host-process-readiness-006.json",
    "guest-agent-readiness-007.json",
    "target-handle-readiness-007.json",
    "host-process-readiness-007.json",
    "guest-agent-readiness-008.json",
    "target-handle-readiness-008.json",
    "host-process-readiness-008.json",
    "guest-agent-readiness-009.json",
    "target-handle-readiness-009.json",
    "host-process-readiness-009.json",
    "guest-agent-readiness-010.json",
    "target-handle-readiness-010.json",
    "host-process-readiness-010.json",
    "guest-agent-ready.json",
    "target-files-agent-ready.json",
    "guest-control-root-create.json",
    "guest-probe-push.json",
    "guest-probe-normalize.json",
    "guest-probe-readback.json",
    "guest-boot-transport-probe-once.json",
    "guest-marker-readback.json",
    "guest-result-readback-1.json",
    "guest-result-readback-2.json",
    "boot-classification.json",
    "target-handle-pid-terminal.json",
    "host-process-terminal.json",
    "target-files-postflight.json",
    "source-bundle-postflight.json",
    "terminal.json",
)


class BootClassificationResultBindingRequest(
    classification_bindings.BootClassificationBindingRequest, Protocol
):
    prior_boot_classification_root: Path
    prior_boot_classification_manifest_sha256: str
    prior_boot_classification_attempt_id: str


@dataclass(frozen=True)
class BootClassificationResultBinding:
    evidence: dict[str, object]
    upstream: classification_bindings.BootClassificationBinding
    expected_boot_id_sha256: str
    observed_boot_id_sha256: str
    probe_bytes: bytes
    prior_backend_pid: int


def validate_boot_classification_result_bindings(
    request: BootClassificationResultBindingRequest,
) -> BootClassificationResultBinding:
    if (
        request.prior_boot_classification_attempt_id
        != REQUIRED_PRIOR_ATTEMPT_ID
    ):
        raise ValueError("required-prior-boot-classification-attempt-id-mismatch")

    upstream = classification_bindings.validate_boot_classification_bindings(
        _PriorBootClassificationRequestView(request)
    )
    identities: dict[str, object] = {}
    for relative_path, label in (
        (BINDINGS_RELATIVE_PATH, "boot_classification_result_bindings"),
        (PROBE_RELATIVE_PATH, "current_boot_transport_probe"),
    ):
        path = request.repository_root / relative_path
        network_ready._require_committed_regular(path, label)
        identities[f"{label}_sha256"] = network_ready._sha256_file(path)

    root = request.prior_boot_classification_root
    manifest = root / "files.sha256"
    if (
        request.prior_boot_classification_manifest_sha256
        != REQUIRED_PRIOR_MANIFEST_SHA256
        or network_ready._sha256_file(manifest)
        != REQUIRED_PRIOR_MANIFEST_SHA256
    ):
        raise ValueError("prior-boot-classification-manifest-identity-invalid")
    entries = start_control._verify_sha256_manifest(root, manifest)
    if entries != len(REQUIRED_ENTRY_NAMES):
        raise ValueError("prior-boot-classification-entry-count-invalid")
    if runtime_bindings._manifest_names(manifest) != REQUIRED_ENTRY_NAMES:
        raise ValueError("prior-boot-classification-entry-names-invalid")

    _validate_prior_request(request, root)
    _validate_prior_binding(request, root)
    _validate_prior_source_target(request, root)
    _validate_prior_inventory_and_start(request, upstream, root)
    _validate_prior_runtime(request, root)
    _validate_prior_readiness(request, root)
    _validate_prior_probe_and_result(request, root)
    _validate_prior_terminal(request, root)

    return BootClassificationResultBinding(
        evidence={
            "format": EVIDENCE_FORMAT,
            **identities,
            "expected_boot_id_sha256": REQUIRED_EXPECTED_BOOT_ID_SHA256,
            "observed_boot_id_sha256": REQUIRED_OBSERVED_BOOT_ID_SHA256,
            "prior_boot_classification_attempt_id": (
                request.prior_boot_classification_attempt_id
            ),
            "prior_boot_classification_backend_pid": REQUIRED_BACKEND_PID,
            "prior_boot_classification_entries_verified": entries,
            "prior_boot_classification_manifest_sha256": (
                request.prior_boot_classification_manifest_sha256
            ),
            "prior_boot_classification_outcome": "new-boot-started",
            "repository_clean": True,
            "repository_head": request.expected_repository_head,
            "target_uuid": request.target_uuid,
        },
        upstream=upstream,
        expected_boot_id_sha256=REQUIRED_EXPECTED_BOOT_ID_SHA256,
        observed_boot_id_sha256=REQUIRED_OBSERVED_BOOT_ID_SHA256,
        probe_bytes=upstream.probe_bytes,
        prior_backend_pid=REQUIRED_BACKEND_PID,
    )


def _validate_prior_request(
    request: BootClassificationResultBindingRequest, root: Path
) -> None:
    expected = {
        "agent_readiness_attempts": 60,
        "authorization": {
            "bound_prior_guest_agent_result": True,
            "bounded_read_only_guest_agent_readiness": True,
            "bounded_target_runtime_observation": True,
            "install_artifacts_staged_boot_classification_resolution": True,
            "install_artifacts_staged_boot_start_resolution": True,
            "no_status_resume_business_guest_retry_stop_or_quit": True,
            "one_foreground_start_from_stopped": True,
            "one_potential_backend_reactivation_list": True,
            "one_private_guest_probe_delivery_and_execution": True,
            "two_independent_result_readbacks": True,
        },
        "boot_classification_attempt_id": REQUIRED_PRIOR_ATTEMPT_ID,
        "boot_start_attempt_id": REQUIRED_PRIOR_ATTEMPT_ID,
        "command_timeout_seconds": 60,
        "expected_repository_head": REQUIRED_PRIOR_REPOSITORY_HEAD,
        "expected_vm_count": 21,
        "format": classification_bindings.EVIDENCE_FORMAT,
        "guest_agent_attempt_id": request.guest_agent_attempt_id,
        "guest_control_root": _guest_control_root(),
        "identity_observations": 3,
        "poll_interval_seconds": 1,
        "prior_boot_start_attempt_id": request.prior_boot_start_attempt_id,
        "prior_boot_start_manifest_sha256": (
            request.prior_boot_start_manifest_sha256
        ),
        "prior_boot_transport_attempt_id": (
            request.prior_boot_transport_attempt_id
        ),
        "prior_boot_transport_manifest_sha256": (
            request.prior_boot_transport_manifest_sha256
        ),
        "prior_guest_agent_attempt_id": request.prior_guest_agent_attempt_id,
        "prior_guest_agent_manifest_sha256": (
            request.prior_guest_agent_manifest_sha256
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
        raise ValueError("prior-boot-classification-request-invalid")


def _validate_prior_binding(
    request: BootClassificationResultBindingRequest, root: Path
) -> None:
    expected = {
        "boot_classification_attempt_id": REQUIRED_PRIOR_ATTEMPT_ID,
        "boot_classification_bindings_sha256": (
            "84ee057ae76d03ed4a608d5ce31229b8654ce3a94be65a5c5b9dabdd0b461108"
        ),
        "boot_classification_control_sha256": (
            "e464064ef1a6cedf5404abcd1af8012d673e77622104a1f03dd125b00b439728"
        ),
        "boot_transport_probe_sha256": REQUIRED_PROBE_SHA256,
        "expected_boot_id_sha256": REQUIRED_EXPECTED_BOOT_ID_SHA256,
        "format": classification_bindings.EVIDENCE_FORMAT,
        "prior_guest_agent_attempt_id": request.prior_guest_agent_attempt_id,
        "prior_guest_agent_backend_pid": 39591,
        "prior_guest_agent_entries_verified": 26,
        "prior_guest_agent_manifest_sha256": (
            request.prior_guest_agent_manifest_sha256
        ),
        "prior_guest_agent_marker": "absent-after-probe-exec",
        "prior_guest_agent_outcome": "state-indeterminate",
        "prior_guest_agent_result_binding_sha256": (
            "97db210c03c950d584ba602d14d72d1406443dadd931ab15350ac69c81cf3671"
        ),
        "repository_clean": True,
        "repository_head": REQUIRED_PRIOR_REPOSITORY_HEAD,
        "target_uuid": request.target_uuid,
    }
    if network_ready._read_json(root / "binding-preflight.json") != expected:
        raise ValueError("prior-boot-classification-binding-invalid")


def _validate_prior_source_target(
    request: BootClassificationResultBindingRequest, root: Path
) -> None:
    source = network_ready._read_json(root / "source-bundle-preflight.json")
    descriptor = source.get("descriptor")
    if (
        source.get("format")
        != "radishlex-linux-l6-utm-canonical-input-transfer-v1"
        or source.get("inventory_count") != 12
        or not isinstance(descriptor, dict)
        or descriptor.get("sha256") != request.source_bundle_sha256
        or descriptor.get("size") != request.source_bundle_size
    ):
        raise ValueError("prior-boot-classification-source-invalid")
    if network_ready._read_json(root / "source-bundle-postflight.json") != {
        "descriptor_unchanged": True,
        "format": "radishlex-linux-l6-utm-canonical-input-transfer-v1",
        "inventory_unchanged": True,
        "sha256": request.source_bundle_sha256,
        "size": request.source_bundle_size,
    }:
        raise ValueError("prior-boot-classification-source-postflight-invalid")

    names = (
        "target-files-preflight.json",
        "target-files-ready.json",
        "target-files-started.json",
        "target-files-agent-ready.json",
        "target-files-postflight.json",
    )
    targets = [network_ready._read_json(root / name) for name in names]
    target = targets[0]
    if (
        any(value != target for value in targets[1:])
        or target.get("format") != network_ready.EVIDENCE_FORMAT
        or target.get("target_config_sha256")
        != network_ready.REQUIRED_TARGET_CONFIG_SHA256
        or target.get("target_package_name") != request.target_package_path.name
        or target.get("target_package_path_sha256")
        != network_ready._sha256_text(str(request.target_package_path))
    ):
        raise ValueError("prior-boot-classification-target-invalid")


def _validate_prior_inventory_and_start(
    request: BootClassificationResultBindingRequest,
    upstream: classification_bindings.BootClassificationBinding,
    root: Path,
) -> None:
    for name in (
        "host-process-preflight.json",
        "host-process-quiescence-001.json",
        "host-process-quiescence-002.json",
        "host-process-quiescence-003.json",
    ):
        _require_process_state(root / name)
    for name in (
        "target-handles-preflight.json",
        "target-handles-quiescence-001.json",
        "target-handles-quiescence-002.json",
        "target-handles-quiescence-003.json",
    ):
        _require_absent_handle(request, root / name)

    list_observation = launch_bindings._command_observation_from_json(
        network_ready._read_json(root / "utmctl-list-once.json"),
        "prior-boot-classification-list",
    )
    inventory = start_control.parse_utmctl_list(list_observation)
    baseline = _baseline_inventory(upstream)
    if (
        classification_control.boot_control.reactivation_control._require_inventory(
            inventory, baseline, request
        )
        != "stopped"
    ):
        raise ValueError("prior-boot-classification-inventory-invalid")
    if network_ready._read_json(root / "inventory-classification.json") != {
        "format": classification_bindings.EVIDENCE_FORMAT,
        "other_registered_vm_count": 20,
        "other_registered_vms": "all-stopped",
        "registered_vm_count": 21,
        "target_registered_status": "stopped",
        "target_uuid": request.target_uuid,
    }:
        raise ValueError("prior-boot-classification-inventory-evidence-invalid")

    observation = launch_bindings._command_observation_from_json(
        network_ready._read_json(root / "foreground-start-once.json"),
        "prior-boot-classification-start",
    )
    _require_successful_empty_observation(
        observation,
        launch_transport.transport_argv(request),
        "prior-boot-classification-start",
    )


def _validate_prior_runtime(
    request: BootClassificationResultBindingRequest, root: Path
) -> None:
    for name in (
        "host-process-runtime-001.json",
        "host-process-confirmation-001.json",
        "host-process-confirmation-002.json",
        "host-process-confirmation-003.json",
        "host-process-terminal.json",
    ):
        _require_process_state(root / name)

    global_argv = network_ready._lsof_argv(request)
    _require_present_handle(
        network_ready._read_json(root / "target-handles-runtime-001.json"),
        global_argv,
        classification_bindings.EVIDENCE_FORMAT,
        expected_backend_pid=None,
    )
    _require_present_handle(
        network_ready._read_json(root / "target-handle-pid-discovery.json"),
        global_argv,
        runtime_bindings.EVIDENCE_FORMAT,
    )
    targeted_argv = runtime_control.targeted_lsof_argv(
        request, REQUIRED_BACKEND_PID
    )
    for name in (
        "target-handle-pid-confirmation-001.json",
        "target-handle-pid-confirmation-002.json",
        "target-handle-pid-confirmation-003.json",
        "target-handle-pid-terminal.json",
    ):
        _require_present_handle(
            network_ready._read_json(root / name),
            targeted_argv,
            runtime_bindings.EVIDENCE_FORMAT,
        )


def _baseline_inventory(
    binding: classification_bindings.BootClassificationBinding,
) -> tuple[start_control.RegisteredVm, ...]:
    boot_transport = binding.upstream
    runtime_resolution = boot_transport.upstream
    reactivation = runtime_resolution.upstream
    return reactivation.baseline_inventory


def _validate_prior_readiness(
    request: BootClassificationResultBindingRequest, root: Path
) -> None:
    targeted_argv = runtime_control.targeted_lsof_argv(
        request, REQUIRED_BACKEND_PID
    )
    for index in range(1, 11):
        name = f"guest-agent-readiness-{index:03d}.json"
        observation = launch_bindings._command_observation_from_json(
            network_ready._read_json(root / name), f"prior-{name}"
        )
        if index < 10:
            _require_unavailable_readiness(observation, request, name)
        else:
            _require_successful_empty_observation(
                observation, guest_control.readiness_argv(request), name
            )
        _require_present_handle(
            network_ready._read_json(
                root / f"target-handle-readiness-{index:03d}.json"
            ),
            targeted_argv,
            classification_bindings.EVIDENCE_FORMAT,
        )
        _require_process_state(
            root / f"host-process-readiness-{index:03d}.json"
        )
    if network_ready._read_json(root / "guest-agent-ready.json") != {
        "attempt": 10,
        "format": classification_bindings.EVIDENCE_FORMAT,
        "predicate": "canonical-boot-id-readable",
        "state": "ready",
        "target_uuid": request.target_uuid,
    }:
        raise ValueError("prior-boot-classification-ready-invalid")


def _validate_prior_probe_and_result(
    request: BootClassificationResultBindingRequest, root: Path
) -> None:
    control_root = _guest_control_root()
    observations = (
        (
            "guest-control-root-create.json",
            (
                "utmctl",
                "exec",
                request.target_uuid,
                "--cmd",
                "/bin/mkdir",
                "-m",
                "0700",
                control_root,
            ),
        ),
        (
            "guest-probe-push.json",
            (
                "utmctl",
                "file",
                "push",
                request.target_uuid,
                f"{control_root}/boot-transport-probe.incoming.py",
            ),
        ),
        (
            "guest-probe-normalize.json",
            (
                "utmctl",
                "exec",
                request.target_uuid,
                "--cmd",
                "/usr/bin/install",
                "-o",
                "root",
                "-g",
                "root",
                "-m",
                "0600",
                "--",
                f"{control_root}/boot-transport-probe.incoming.py",
                f"{control_root}/boot-transport-probe.py",
            ),
        ),
    )
    for name, argv in observations:
        observation = launch_bindings._command_observation_from_json(
            network_ready._read_json(root / name), f"prior-{name}"
        )
        _require_successful_empty_observation(observation, argv, name)

    readback = launch_bindings._command_observation_from_json(
        network_ready._read_json(root / "guest-probe-readback.json"),
        "prior-guest-probe-readback",
    )
    _require_exact_readback(
        readback,
        (
            "utmctl",
            "file",
            "pull",
            request.target_uuid,
            f"{control_root}/boot-transport-probe.py",
        ),
        REQUIRED_PROBE_SHA256,
        REQUIRED_PROBE_SIZE,
        "prior-guest-probe-readback",
    )

    probe_value = network_ready._read_json(
        root / "guest-boot-transport-probe-once.json"
    )
    probe_observation = probe_value.get("observation")
    if (
        probe_value.get("format") != classification_bindings.EVIDENCE_FORMAT
        or probe_value.get("raw_output_persisted") is not False
        or not isinstance(probe_observation, dict)
        or probe_observation.get("argv")
        != list(_probe_argv(request, control_root))
        or probe_observation.get("exit_code") != 0
        or probe_observation.get("timed_out") is not False
        or not runtime_bindings._empty_stream(probe_observation.get("stdout"))
        or not runtime_bindings._empty_stream(probe_observation.get("stderr"))
    ):
        raise ValueError("prior-boot-classification-probe-exec-invalid")

    marker = guest_probe.marker_bytes(REQUIRED_PRIOR_ATTEMPT_ID, REQUIRED_PROBE_SHA256)
    marker_observation = launch_bindings._command_observation_from_json(
        network_ready._read_json(root / "guest-marker-readback.json"),
        "prior-guest-marker-readback",
    )
    _require_exact_payload(
        marker_observation,
        (
            "utmctl",
            "file",
            "pull",
            request.target_uuid,
            f"{control_root}/attempt.marker.json",
        ),
        marker,
        "prior-guest-marker-readback",
    )

    result = guest_probe.result_bytes(
        REQUIRED_PRIOR_ATTEMPT_ID,
        request.target_uuid,
        REQUIRED_PROBE_SHA256,
        REQUIRED_OBSERVED_BOOT_ID_SHA256,
    )
    for index in (1, 2):
        result_observation = launch_bindings._command_observation_from_json(
            network_ready._read_json(
                root / f"guest-result-readback-{index}.json"
            ),
            f"prior-guest-result-readback-{index}",
        )
        _require_exact_payload(
            result_observation,
            (
                "utmctl",
                "file",
                "pull",
                request.target_uuid,
                f"{control_root}/boot-identity.evidence.json",
            ),
            result,
            f"prior-guest-result-readback-{index}",
        )
    if network_ready._read_json(root / "boot-classification.json") != {
        "classification": "new-boot-started",
        "expected_boot_id_sha256": REQUIRED_EXPECTED_BOOT_ID_SHA256,
        "format": classification_bindings.EVIDENCE_FORMAT,
        "observed_boot_id_sha256": REQUIRED_OBSERVED_BOOT_ID_SHA256,
        "transport": "foreground-start-private-double-readback",
    }:
        raise ValueError("prior-boot-classification-result-invalid")


def _validate_prior_terminal(
    request: BootClassificationResultBindingRequest, root: Path
) -> None:
    expected = {
        "agent_readiness_invocations": 10,
        "agent_readiness_state": "ready",
        "automatic_cleanup": "not-performed",
        "automatic_quit": "not-performed",
        "automatic_retry": "not-performed",
        "automatic_stop": "not-performed",
        "backend_pid": REQUIRED_BACKEND_PID,
        "boot_classification": "new-boot-started",
        "boot_start_attempt_id": REQUIRED_PRIOR_ATTEMPT_ID,
        "business_guest_action": "not-performed",
        "file_pull_invocations": 4,
        "file_push_invocations": 1,
        "foreground_start_invocations": 1,
        "format": classification_bindings.EVIDENCE_FORMAT,
        "guest_exec_invocations": 13,
        "guest_probe_invocations": 1,
        "identity_observation_count": 3,
        "inventory_probe_invocations": 1,
        "maintenance_resume_invocations": 0,
        "observed_boot_id_sha256": REQUIRED_OBSERVED_BOOT_ID_SHA256,
        "operation_id": "not-read-or-generated",
        "outcome": "new-boot-started",
        "plain_utmctl_list": "attempted-once-as-potential-backend-reactivation",
        "plain_utmctl_start": "not-performed",
        "plain_utmctl_status": "not-performed",
        "readiness_identity_observation_count": 10,
        "reason": (
            "single-start-private-double-readback-new-boot-"
            "and-terminal-target-stable"
        ),
        "result_readback_invocations": 2,
        "runtime_poll_count": 1,
        "target_handles_terminal": "present",
        "target_name": request.target_name,
        "target_uuid": request.target_uuid,
        "terminal_relevant_host_process_count": 0,
        "transaction": "artifacts-staged-preserved-no-resume",
    }
    if network_ready._read_json(root / "terminal.json") != expected:
        raise ValueError("prior-boot-classification-terminal-invalid")


def _require_process_state(path: Path) -> None:
    runtime_bindings._require_process_state(
        path,
        0,
        expected_format=runtime_bindings.EVIDENCE_FORMAT,
        error_label="prior-boot-classification",
    )


def _require_absent_handle(
    request: BootClassificationResultBindingRequest, path: Path
) -> None:
    value = network_ready._read_json(path)
    observation = value.get("observation")
    if (
        value.get("format") != classification_bindings.EVIDENCE_FORMAT
        or value.get("state") != "absent"
        or value.get("backend_command") is not None
        or value.get("backend_pid") is not None
        or value.get("efi_handle_count") != 0
        or value.get("process_record_count") != 0
        or value.get("qcow2_handle_count") != 0
        or not isinstance(observation, dict)
        or observation.get("argv") != list(network_ready._lsof_argv(request))
        or observation.get("exit_code") != 1
        or observation.get("timed_out") is not False
        or not runtime_bindings._empty_stream(observation.get("stdout"))
        or not runtime_bindings._empty_stream(observation.get("stderr"))
    ):
        raise ValueError("prior-boot-classification-absent-handle-invalid")


def _require_present_handle(
    value: dict[str, object],
    expected_argv: tuple[str, ...],
    expected_format: str,
    *,
    expected_backend_pid: int | None = REQUIRED_BACKEND_PID,
) -> None:
    observation = value.get("observation")
    stdout = observation.get("stdout") if isinstance(observation, dict) else None
    backend_pid_valid = (
        "backend_pid" not in value
        if expected_backend_pid is None
        else value.get("backend_pid") == expected_backend_pid
    )
    if (
        value.get("format") != expected_format
        or value.get("state") != "present"
        or value.get("backend_command") != "QEMULauncher"
        or not backend_pid_valid
        or value.get("efi_handle_count") != 1
        or value.get("process_record_count") != 1
        or value.get("qcow2_handle_count") != 1
        or not isinstance(observation, dict)
        or observation.get("argv") != list(expected_argv)
        or observation.get("exit_code") != 0
        or observation.get("timed_out") is not False
        or not runtime_bindings._empty_stream(observation.get("stderr"))
        or not runtime_bindings._complete_stream(stdout)
        or not isinstance(stdout, dict)
        or stdout.get("sha256") != REQUIRED_HANDLE_STDOUT_SHA256
        or stdout.get("total_bytes") != 378
    ):
        raise ValueError("prior-boot-classification-present-handle-invalid")


def _require_unavailable_readiness(
    observation: start_control.CommandObservation,
    request: BootClassificationResultBindingRequest,
    label: str,
) -> None:
    if (
        observation.argv != guest_control.readiness_argv(request)
        or observation.exit_code != 0
        or observation.timed_out
        or observation.stdout.total_bytes != 0
        or observation.stdout.truncated
        or observation.stderr.prefix
        != guest_bindings.REQUIRED_AGENT_UNAVAILABLE_STDERR
        or observation.stderr.total_bytes
        != len(guest_bindings.REQUIRED_AGENT_UNAVAILABLE_STDERR)
        or observation.stderr.truncated
    ):
        raise ValueError(f"prior-boot-classification-{label}-invalid")


def _require_successful_empty_observation(
    observation: start_control.CommandObservation,
    expected_argv: tuple[str, ...],
    label: str,
) -> None:
    if (
        observation.argv != expected_argv
        or observation.exit_code != 0
        or observation.timed_out
        or observation.stdout.total_bytes != 0
        or observation.stdout.truncated
        or observation.stderr.total_bytes != 0
        or observation.stderr.truncated
    ):
        raise ValueError(f"{label}-invalid")


def _require_exact_readback(
    observation: start_control.CommandObservation,
    expected_argv: tuple[str, ...],
    expected_sha256: str,
    expected_size: int,
    label: str,
) -> None:
    if (
        observation.argv != expected_argv
        or observation.exit_code != 0
        or observation.timed_out
        or observation.stderr.total_bytes != 0
        or observation.stderr.truncated
        or observation.stdout.sha256 != expected_sha256
        or observation.stdout.total_bytes != expected_size
        or observation.stdout.truncated
    ):
        raise ValueError(f"{label}-invalid")


def _require_exact_payload(
    observation: start_control.CommandObservation,
    expected_argv: tuple[str, ...],
    payload: bytes,
    label: str,
) -> None:
    if (
        observation.argv != expected_argv
        or observation.exit_code != 0
        or observation.timed_out
        or observation.stdout.prefix != payload
        or observation.stdout.total_bytes != len(payload)
        or observation.stdout.truncated
        or observation.stderr.total_bytes != 0
        or observation.stderr.truncated
    ):
        raise ValueError(f"{label}-invalid")


def _probe_argv(
    request: BootClassificationResultBindingRequest, control_root: str
) -> tuple[str, ...]:
    return (
        "utmctl",
        "exec",
        request.target_uuid,
        "--cmd",
        "/usr/bin/python3",
        "-I",
        "-B",
        f"{control_root}/boot-transport-probe.py",
        "--attempt-id",
        REQUIRED_PRIOR_ATTEMPT_ID,
        "--target-uuid",
        request.target_uuid,
        "--expected-probe-sha256",
        REQUIRED_PROBE_SHA256,
        "--control-scope",
        guest_probe.CONTROL_SCOPE_BOOT_START,
        "--control-root",
        control_root,
    )


def _guest_control_root() -> str:
    return (
        "/var/tmp/radishlex-l6-v4-boot-start-" + REQUIRED_PRIOR_ATTEMPT_ID
    )


class _PriorBootClassificationRequestView:
    def __init__(self, request: BootClassificationResultBindingRequest) -> None:
        self._request = request

    @property
    def boot_classification_attempt_id(self) -> str:
        return self._request.prior_boot_classification_attempt_id

    @property
    def boot_start_attempt_id(self) -> str:
        return self._request.prior_boot_classification_attempt_id

    def __getattr__(self, name: str) -> object:
        return getattr(self._request, name)
