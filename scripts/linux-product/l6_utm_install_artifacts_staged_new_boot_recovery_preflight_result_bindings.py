#!/usr/bin/env python3
from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
from typing import Protocol

import l6_utm_guest_network_ready as network_ready
import l6_utm_install_artifacts_staged_new_boot_recovery_preflight_bindings as recovery_bindings
import l6_utm_install_artifacts_staged_runtime_resolution as runtime_control
import l6_utm_install_artifacts_staged_runtime_resolution_bindings as runtime_bindings
import l6_utm_launch_transport_v2 as launch_transport
import l6_utm_start_once as start_control
import l6_v4_install_artifacts_staged_new_boot_recovery_preflight as guest_probe


EVIDENCE_FORMAT = (
    "radishlex-linux-l6-utm-install-artifacts-staged-"
    "new-boot-recovery-preflight-result-binding-v1"
)
BINDINGS_RELATIVE_PATH = Path(
    "scripts/linux-product/"
    "l6_utm_install_artifacts_staged_new_boot_recovery_preflight_"
    "result_bindings.py"
)
REQUIRED_PRIOR_REPOSITORY_HEAD = "eba4bae946b924ef6df0933517ea4dd4f17113fb"
REQUIRED_PRIOR_MANIFEST_SHA256 = (
    "1d593a8f7a9ce725320954257e763862703d43e89a3e82c7fa5cd94391f233c7"
)
REQUIRED_PRIOR_ATTEMPT_ID = recovery_bindings.REQUIRED_ATTEMPT_ID
REQUIRED_BACKEND_PID = 92422
REQUIRED_HANDLE_STDOUT_SHA256 = (
    "0f5a2c7fbaff52d71998816f2bcc457d0074bf7c0b8102289d2c1b78dc700856"
)
REQUIRED_CONTROL_SHA256 = (
    "7c36f02c1a2673cf39def4a5868738736b8399b1ee8eaa558a56f62ee5419d61"
)
REQUIRED_CONTROL_SIZE = 35_952
REQUIRED_BINDINGS_SHA256 = (
    "394e516412a30049b0ad446ceb20ccff1f7959a4fb456afa0fdb4cfe9aefba8f"
)
REQUIRED_BINDINGS_SIZE = 5_088
REQUIRED_PROBE_SHA256 = (
    "59cfa67ab680cc0c93db98c67785f12a91a4d3c193856c9b5db81ebb44b53136"
)
REQUIRED_PROBE_SIZE = 16_726
REQUIRED_DRIVER_SHA256 = recovery_bindings.REQUIRED_RESUME_DRIVER_SHA256
REQUIRED_DRIVER_SIZE = 37_312
REQUIRED_MARKER_SHA256 = (
    "db20eeb30eedea3ad9a01205f93aff58405d121203f29e60221441aecc76317b"
)
REQUIRED_MARKER_SIZE = 361
REQUIRED_RESULT_SHA256 = (
    "36f253b6ade513428b51ed43ef278e907af0675d3d64caff591605861a142924"
)
REQUIRED_RESULT_SIZE = 875
REQUIRED_PHASE_SHA256 = (
    "24562544ff783cde108063a5de1e30453389cfe3bd68e4ad0493242b53541648"
)
REQUIRED_PHASE_SIZE = 116
REQUIRED_ENTRY_NAMES = (
    "request.json",
    "binding-preflight.json",
    "source-bundle-preflight.json",
    "target-files-preflight.json",
    "host-process-preflight.json",
    "target-handle-pid-discovery.json",
    "target-handle-pid-confirmation-001.json",
    "host-process-confirmation-001.json",
    "target-handle-pid-confirmation-002.json",
    "host-process-confirmation-002.json",
    "target-handle-pid-confirmation-003.json",
    "host-process-confirmation-003.json",
    "guest-agent-readiness-001.json",
    "guest-agent-ready.json",
    "target-files-agent-ready.json",
    "target-handle-agent-ready.json",
    "host-process-agent-ready.json",
    "guest-recovery-root-create.json",
    "guest-frozen-resume-driver-push.json",
    "guest-frozen-resume-driver-chown.json",
    "guest-frozen-resume-driver-chmod.json",
    "guest-frozen-resume-driver-publish.json",
    "guest-frozen-resume-driver-readback.json",
    "guest-recovery-preflight-probe-push.json",
    "guest-recovery-preflight-probe-chown.json",
    "guest-recovery-preflight-probe-chmod.json",
    "guest-recovery-preflight-probe-publish.json",
    "guest-recovery-preflight-probe-readback.json",
    "target-handle-pre-probe.json",
    "host-process-pre-probe.json",
    "guest-recovery-preflight-once.json",
    "guest-recovery-marker-readback.json",
    "guest-recovery-result-readback-1.json",
    "guest-recovery-result-readback-2.json",
    "guest-recovery-result.json",
    "guest-recovery-phase-readback.json",
    "source-bundle-postflight.json",
    "terminal.json",
)


class RecoveryPreflightResultBindingRequest(
    recovery_bindings.RecoveryPreflightBindingRequest, Protocol
):
    prior_recovery_preflight_root: Path
    prior_recovery_preflight_manifest_sha256: str
    prior_recovery_preflight_attempt_id: str


@dataclass(frozen=True)
class RecoveryPreflightResultBinding:
    evidence: dict[str, object]
    upstream: object
    prior_backend_pid: int
    guest_recovery_outcome: str
    guest_probe_transport_exit_code: int


def validate_recovery_preflight_result_bindings(
    request: RecoveryPreflightResultBindingRequest,
) -> RecoveryPreflightResultBinding:
    if request.prior_recovery_preflight_attempt_id != REQUIRED_PRIOR_ATTEMPT_ID:
        raise ValueError("required-prior-recovery-preflight-attempt-id-mismatch")

    upstream = (
        recovery_bindings.result_bindings.validate_boot_classification_result_bindings(
            request
        )
    )
    binding_path = request.repository_root / BINDINGS_RELATIVE_PATH
    network_ready._require_committed_regular(
        binding_path, "recovery_preflight_result_bindings"
    )

    root = request.prior_recovery_preflight_root
    manifest = root / "files.sha256"
    if (
        request.prior_recovery_preflight_manifest_sha256
        != REQUIRED_PRIOR_MANIFEST_SHA256
        or network_ready._sha256_file(manifest) != REQUIRED_PRIOR_MANIFEST_SHA256
    ):
        raise ValueError("prior-recovery-preflight-manifest-identity-invalid")
    entries = start_control._verify_sha256_manifest(root, manifest)
    if entries != len(REQUIRED_ENTRY_NAMES):
        raise ValueError("prior-recovery-preflight-entry-count-invalid")
    if runtime_bindings._manifest_names(manifest) != REQUIRED_ENTRY_NAMES:
        raise ValueError("prior-recovery-preflight-entry-names-invalid")

    _validate_request(request, root)
    _validate_binding(request, root)
    _validate_source_target(request, root)
    _validate_host_identity(request, root)
    _validate_delivery(request, root)
    _validate_guest_result(request, root)
    _validate_terminal(request, root)
    _validate_no_forbidden_commands(root)

    return RecoveryPreflightResultBinding(
        evidence={
            "format": EVIDENCE_FORMAT,
            "guest_probe_transport_exit_code": 0,
            "guest_recovery_outcome": "recovery-rejected",
            "host_recorded_outcome": "state-indeterminate",
            "prior_recovery_preflight_attempt_id": (
                request.prior_recovery_preflight_attempt_id
            ),
            "prior_recovery_preflight_backend_pid": REQUIRED_BACKEND_PID,
            "prior_recovery_preflight_entries_verified": entries,
            "prior_recovery_preflight_manifest_sha256": (
                request.prior_recovery_preflight_manifest_sha256
            ),
            "repository_clean": True,
            "repository_head": request.expected_repository_head,
            "result_authority": "guest-marker-double-result-and-phase",
            "target_uuid": request.target_uuid,
        },
        upstream=upstream,
        prior_backend_pid=REQUIRED_BACKEND_PID,
        guest_recovery_outcome="recovery-rejected",
        guest_probe_transport_exit_code=0,
    )


def _validate_request(
    request: RecoveryPreflightResultBindingRequest, root: Path
) -> None:
    value = network_ready._read_json(root / "request.json")
    expected_authorization = {
        "bound_prior_boot_classification_result": True,
        "bounded_read_only_guest_agent_readiness": True,
        "install_artifacts_staged_new_boot_recovery_preflight": True,
        "no_inventory_start_status_resume_dpkg_mutation_retry_stop_or_quit": True,
        "one_private_recovery_probe_delivery_and_execution": True,
        "two_independent_result_readbacks": True,
    }
    expected = {
        "agent_readiness_attempts": 60,
        "authorization": expected_authorization,
        "command_timeout_seconds": 60,
        "expected_repository_head": REQUIRED_PRIOR_REPOSITORY_HEAD,
        "identity_observations": 3,
        "prior_boot_classification_attempt_id": (
            recovery_bindings.REQUIRED_PRIOR_ATTEMPT_ID
        ),
        "prior_boot_classification_manifest_sha256": (
            recovery_bindings.REQUIRED_PRIOR_MANIFEST_SHA256
        ),
        "recovery_preflight_attempt_id": REQUIRED_PRIOR_ATTEMPT_ID,
        "target_uuid": request.target_uuid,
    }
    if any(value.get(key) != expected_value for key, expected_value in expected.items()):
        raise ValueError("prior-recovery-preflight-request-invalid")


def _validate_binding(
    request: RecoveryPreflightResultBindingRequest, root: Path
) -> None:
    expected = {
        "current_boot_id_sha256": recovery_bindings.REQUIRED_CURRENT_BOOT_ID_SHA256,
        "format": recovery_bindings.EVIDENCE_FORMAT,
        "frozen_resume_driver_sha256": REQUIRED_DRIVER_SHA256,
        "frozen_resume_driver_size": REQUIRED_DRIVER_SIZE,
        "prior_boot_classification_attempt_id": (
            recovery_bindings.REQUIRED_PRIOR_ATTEMPT_ID
        ),
        "prior_boot_classification_backend_pid": REQUIRED_BACKEND_PID,
        "prior_boot_classification_entries_verified": 72,
        "prior_boot_classification_manifest_sha256": (
            recovery_bindings.REQUIRED_PRIOR_MANIFEST_SHA256
        ),
        "prior_boot_classification_outcome": "new-boot-started",
        "prior_boot_id_sha256": recovery_bindings.REQUIRED_PRIOR_BOOT_ID_SHA256,
        "recovery_preflight_attempt_id": REQUIRED_PRIOR_ATTEMPT_ID,
        "recovery_preflight_bindings_sha256": REQUIRED_BINDINGS_SHA256,
        "recovery_preflight_bindings_size": REQUIRED_BINDINGS_SIZE,
        "recovery_preflight_control_sha256": REQUIRED_CONTROL_SHA256,
        "recovery_preflight_control_size": REQUIRED_CONTROL_SIZE,
        "recovery_preflight_probe_sha256": REQUIRED_PROBE_SHA256,
        "recovery_preflight_probe_size": REQUIRED_PROBE_SIZE,
        "repository_clean": True,
        "repository_head": REQUIRED_PRIOR_REPOSITORY_HEAD,
        "target_uuid": request.target_uuid,
    }
    if network_ready._read_json(root / "binding-preflight.json") != expected:
        raise ValueError("prior-recovery-preflight-binding-invalid")


def _validate_source_target(
    request: RecoveryPreflightResultBindingRequest, root: Path
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
        or network_ready._read_json(root / "source-bundle-postflight.json")
        != {
            "descriptor_unchanged": True,
            "format": "radishlex-linux-l6-utm-canonical-input-transfer-v1",
            "inventory_unchanged": True,
            "sha256": request.source_bundle_sha256,
            "size": request.source_bundle_size,
        }
    ):
        raise ValueError("prior-recovery-preflight-source-invalid")
    before = network_ready._read_json(root / "target-files-preflight.json")
    ready = network_ready._read_json(root / "target-files-agent-ready.json")
    if (
        before != ready
        or before.get("format") != network_ready.EVIDENCE_FORMAT
        or before.get("target_package_name") != request.target_package_path.name
        or before.get("target_package_path_sha256")
        != network_ready._sha256_text(str(request.target_package_path))
    ):
        raise ValueError("prior-recovery-preflight-target-invalid")


def _validate_host_identity(
    request: RecoveryPreflightResultBindingRequest, root: Path
) -> None:
    process_names = (
        "host-process-preflight.json",
        "host-process-confirmation-001.json",
        "host-process-confirmation-002.json",
        "host-process-confirmation-003.json",
        "host-process-agent-ready.json",
        "host-process-pre-probe.json",
    )
    for name in process_names:
        value = network_ready._read_json(root / name)
        observation = value.get("observation")
        if (
            value.get("format") != runtime_bindings.EVIDENCE_FORMAT
            or value.get("relevant_process_count") != 0
            or value.get("relevant_processes") != []
            or not isinstance(observation, dict)
            or observation.get("argv") != list(launch_transport.PROCESS_COMMAND)
            or not _successful_observation(observation)
        ):
            raise ValueError("prior-recovery-preflight-process-invalid")

    handle_names = (
        "target-handle-pid-discovery.json",
        "target-handle-pid-confirmation-001.json",
        "target-handle-pid-confirmation-002.json",
        "target-handle-pid-confirmation-003.json",
        "target-handle-agent-ready.json",
        "target-handle-pre-probe.json",
    )
    for name in handle_names:
        value = network_ready._read_json(root / name)
        observation = value.get("observation")
        stdout = observation.get("stdout") if isinstance(observation, dict) else None
        if (
            value.get("format") != runtime_bindings.EVIDENCE_FORMAT
            or value.get("state") != "present"
            or value.get("backend_pid") != REQUIRED_BACKEND_PID
            or value.get("backend_command") != "QEMULauncher"
            or value.get("process_record_count") != 1
            or value.get("efi_handle_count") != 1
            or value.get("qcow2_handle_count") != 1
            or not _successful_observation(observation)
            or not isinstance(stdout, dict)
            or stdout.get("sha256") != REQUIRED_HANDLE_STDOUT_SHA256
            or stdout.get("total_bytes") != 378
        ):
            raise ValueError("prior-recovery-preflight-handle-invalid")

    readiness = network_ready._read_json(root / "guest-agent-readiness-001.json")
    if (
        readiness.get("argv")
        != [
            "utmctl",
            "exec",
            request.target_uuid,
            "--cmd",
            "/usr/bin/test",
            "-r",
            "/proc/sys/kernel/random/boot_id",
        ]
        or not _successful_empty_observation(readiness)
        or network_ready._read_json(root / "guest-agent-ready.json")
        != {
            "attempt": 1,
            "format": recovery_bindings.EVIDENCE_FORMAT.replace(
                "binding-v1", "v1"
            ),
            "predicate": "canonical-boot-id-readable",
            "state": "ready",
            "target_uuid": request.target_uuid,
        }
    ):
        raise ValueError("prior-recovery-preflight-readiness-invalid")


def _validate_delivery(
    request: RecoveryPreflightResultBindingRequest, root: Path
) -> None:
    control_root = _guest_control_root()
    root_create = network_ready._read_json(root / "guest-recovery-root-create.json")
    if (
        root_create.get("argv")
        != [
            "utmctl",
            "exec",
            request.target_uuid,
            "--cmd",
            "/bin/mkdir",
            "-m",
            "0700",
            control_root,
        ]
        or not _successful_empty_observation(root_create)
    ):
        raise ValueError("prior-recovery-preflight-root-create-invalid")

    for label, final, sha256, size in (
        (
            "frozen-resume-driver",
            "frozen-resume-driver.py",
            REQUIRED_DRIVER_SHA256,
            REQUIRED_DRIVER_SIZE,
        ),
        (
            "recovery-preflight-probe",
            "recovery-preflight.py",
            REQUIRED_PROBE_SHA256,
            REQUIRED_PROBE_SIZE,
        ),
    ):
        incoming = f"{control_root}/{final.removesuffix('.py')}.incoming.py"
        final_path = f"{control_root}/{final}"
        commands = {
            "push": ["utmctl", "file", "push", request.target_uuid, incoming],
            "chown": [
                "utmctl",
                "exec",
                request.target_uuid,
                "--cmd",
                "/bin/chown",
                "root:root",
                incoming,
            ],
            "chmod": [
                "utmctl",
                "exec",
                request.target_uuid,
                "--cmd",
                "/bin/chmod",
                "0600",
                incoming,
            ],
            "publish": [
                "utmctl",
                "exec",
                request.target_uuid,
                "--cmd",
                "/bin/mv",
                "--",
                incoming,
                final_path,
            ],
        }
        for action, argv in commands.items():
            value = network_ready._read_json(root / f"guest-{label}-{action}.json")
            if value.get("argv") != argv or not _successful_empty_observation(value):
                raise ValueError(
                    f"prior-recovery-preflight-{label}-{action}-invalid"
                )
        readback = network_ready._read_json(root / f"guest-{label}-readback.json")
        stdout = readback.get("stdout")
        if (
            readback.get("argv")
            != ["utmctl", "file", "pull", request.target_uuid, final_path]
            or not _successful_observation(readback)
            or not isinstance(stdout, dict)
            or stdout.get("sha256") != sha256
            or stdout.get("total_bytes") != size
            or stdout.get("truncated") is not False
        ):
            raise ValueError(f"prior-recovery-preflight-{label}-readback-invalid")


def _validate_guest_result(
    request: RecoveryPreflightResultBindingRequest, root: Path
) -> None:
    probe_value = network_ready._read_json(root / "guest-recovery-preflight-once.json")
    probe_observation = probe_value.get("observation")
    if (
        probe_value.get("format")
        != recovery_bindings.EVIDENCE_FORMAT.replace("binding-v1", "v1")
        or probe_value.get("raw_output_persisted") is not False
        or not isinstance(probe_observation, dict)
        or probe_observation.get("argv") != _probe_argv(request)
        or not _successful_empty_observation(probe_observation)
    ):
        raise ValueError("prior-recovery-preflight-probe-transport-invalid")

    control_root = _guest_control_root()
    for name, guest_name, sha256, size in (
        (
            "guest-recovery-marker-readback.json",
            "attempt.marker.json",
            REQUIRED_MARKER_SHA256,
            REQUIRED_MARKER_SIZE,
        ),
        (
            "guest-recovery-result-readback-1.json",
            "preflight.evidence.json",
            REQUIRED_RESULT_SHA256,
            REQUIRED_RESULT_SIZE,
        ),
        (
            "guest-recovery-result-readback-2.json",
            "preflight.evidence.json",
            REQUIRED_RESULT_SHA256,
            REQUIRED_RESULT_SIZE,
        ),
        (
            "guest-recovery-phase-readback.json",
            "phase.json",
            REQUIRED_PHASE_SHA256,
            REQUIRED_PHASE_SIZE,
        ),
    ):
        value = network_ready._read_json(root / name)
        stdout = value.get("stdout")
        if (
            value.get("argv")
            != [
                "utmctl",
                "file",
                "pull",
                request.target_uuid,
                f"{control_root}/{guest_name}",
            ]
            or not _successful_observation(value)
            or not isinstance(stdout, dict)
            or stdout.get("sha256") != sha256
            or stdout.get("total_bytes") != size
            or stdout.get("truncated") is not False
        ):
            raise ValueError("prior-recovery-preflight-readback-invalid")

    terminal = network_ready._read_json(root / "guest-recovery-result.json")
    expected = {
        "automatic_cleanup": "not-performed",
        "automatic_quit": "not-performed",
        "automatic_retry": "not-performed",
        "automatic_stop": "not-performed",
        "current_boot_id_sha256": recovery_bindings.REQUIRED_CURRENT_BOOT_ID_SHA256,
        "dpkg_mutation_executed": False,
        "format": guest_probe.EVIDENCE_FORMAT,
        "guard_profile": "not-observed",
        "maintenance_resume_invocations": 0,
        "operation_id": "existing-receipt-hash-only",
        "operation_id_sha256": guest_probe.EXPECTED_OPERATION_ID_SHA256,
        "outcome": "recovery-rejected",
        "phase": "persistent-transaction",
        "prior_boot_id_sha256": recovery_bindings.REQUIRED_PRIOR_BOOT_ID_SHA256,
        "reason": "ResumeDriverError:directory-identity-invalid:state-root",
        "target_uuid": request.target_uuid,
        "transaction": "artifacts-staged-preserved-no-resume",
    }
    if terminal != expected:
        raise ValueError("prior-recovery-preflight-guest-result-invalid")


def _validate_terminal(
    request: RecoveryPreflightResultBindingRequest, root: Path
) -> None:
    expected = {
        "agent_readiness_invocations": 1,
        "automatic_cleanup": "not-performed",
        "automatic_quit": "not-performed",
        "automatic_retry": "not-performed",
        "automatic_stop": "not-performed",
        "backend_pid": REQUIRED_BACKEND_PID,
        "business_guest_action": "not-performed",
        "file_pull_invocations": 6,
        "file_push_invocations": 2,
        "format": recovery_bindings.EVIDENCE_FORMAT.replace("binding-v1", "v1"),
        "guest_exec_invocations": 8,
        "guest_probe_invocations": 1,
        "identity_observation_count": 3,
        "inventory_probe_invocations": 0,
        "maintenance_resume_invocations": 0,
        "operation_id": "existing-receipt-hash-only",
        "outcome": "state-indeterminate",
        "plain_utmctl_list": "not-performed",
        "plain_utmctl_start": "not-performed",
        "plain_utmctl_status": "not-performed",
        "reason": "guest-recovery-phase-readback:probe-exit-evidence-mismatch",
        "recovery_preflight_attempt_id": REQUIRED_PRIOR_ATTEMPT_ID,
        "result_readback_invocations": 2,
        "target_name": request.target_name,
        "target_uuid": request.target_uuid,
        "terminal_relevant_host_process_count": 0,
        "transaction": "artifacts-staged-preserved-no-resume",
    }
    if network_ready._read_json(root / "terminal.json") != expected:
        raise ValueError("prior-recovery-preflight-terminal-invalid")


def _validate_no_forbidden_commands(root: Path) -> None:
    forbidden_guest_tokens = {"dpkg", "dpkg-query", "quit", "resume", "retry", "stop"}
    for name in REQUIRED_ENTRY_NAMES:
        value = network_ready._read_json(root / name)
        observations = [value]
        nested = value.get("observation")
        if isinstance(nested, dict):
            observations.append(nested)
        for observation in observations:
            argv = observation.get("argv")
            if not isinstance(argv, list) or not all(isinstance(item, str) for item in argv):
                continue
            if argv[:2] in (["utmctl", "list"], ["utmctl", "start"], ["utmctl", "status"]):
                raise ValueError("prior-recovery-preflight-forbidden-command")
            if any(token in forbidden_guest_tokens for token in argv):
                raise ValueError("prior-recovery-preflight-forbidden-command")


def _successful_observation(value: object) -> bool:
    if not isinstance(value, dict):
        return False
    return (
        value.get("exit_code") == 0
        and value.get("timed_out") is False
        and runtime_bindings._empty_stream(value.get("stderr"))
    )


def _successful_empty_observation(value: object) -> bool:
    return _successful_observation(value) and runtime_bindings._empty_stream(
        value.get("stdout") if isinstance(value, dict) else None
    )


def _guest_control_root() -> str:
    return (
        "/var/tmp/radishlex-l6-v4-install-artifacts-staged-"
        "new-boot-recovery-preflight-" + REQUIRED_PRIOR_ATTEMPT_ID
    )


def _probe_argv(request: RecoveryPreflightResultBindingRequest) -> list[str]:
    control_root = _guest_control_root()
    return [
        "utmctl",
        "exec",
        request.target_uuid,
        "--cmd",
        "/usr/bin/python3",
        "-I",
        "-B",
        f"{control_root}/recovery-preflight.py",
        "--attempt-id",
        REQUIRED_PRIOR_ATTEMPT_ID,
        "--target-uuid",
        request.target_uuid,
        "--prior-boot-id-sha256",
        recovery_bindings.REQUIRED_PRIOR_BOOT_ID_SHA256,
        "--current-boot-id-sha256",
        recovery_bindings.REQUIRED_CURRENT_BOOT_ID_SHA256,
        "--resume-driver",
        f"{control_root}/frozen-resume-driver.py",
        "--expected-resume-driver-sha256",
        REQUIRED_DRIVER_SHA256,
        "--expected-probe-sha256",
        REQUIRED_PROBE_SHA256,
        "--control-root",
        control_root,
    ]
