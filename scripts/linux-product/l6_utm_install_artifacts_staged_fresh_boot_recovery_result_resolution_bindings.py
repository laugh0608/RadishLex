#!/usr/bin/env python3
from __future__ import annotations

import hashlib
import re
import stat
from dataclasses import dataclass
from pathlib import Path
from typing import Protocol

import l6_utm_guest_network_ready as network_ready
import l6_utm_install_artifacts_staged_fresh_boot_recovery_preflight_result_bindings as prior_result_bindings
import l6_utm_install_artifacts_staged_fresh_boot_recovery_result_resolution as resolution_control
import l6_utm_install_artifacts_staged_runtime_resolution as runtime_control
import l6_utm_install_artifacts_staged_runtime_resolution_bindings as runtime_bindings
import l6_utm_launch_transport_v2 as launch_transport
import l6_utm_start_once as start_control
import l6_v4_install_artifacts_staged_new_boot_recovery_preflight as guest_probe


EVIDENCE_FORMAT = (
    "radishlex-linux-l6-utm-install-artifacts-staged-"
    "fresh-boot-recovery-result-resolution-binding-v1"
)
BINDINGS_RELATIVE_PATH = Path(
    "scripts/linux-product/"
    "l6_utm_install_artifacts_staged_fresh_boot_recovery_"
    "result_resolution_bindings.py"
)
REQUIRED_PRIOR_MANIFEST_SHA256 = (
    "181147204f3104319ef44a7d742063a7b81c6310e51f40caaee12d8358f3989b"
)
REQUIRED_PRIOR_ATTEMPT_ID = resolution_control.REQUIRED_ATTEMPT_ID
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
    "existing-recovery-result-readback-1.json",
    "existing-recovery-result-readback-2.json",
    "existing-recovery-result.json",
    "existing-recovery-phase-readback.json",
    "target-handle-pid-terminal.json",
    "host-process-terminal.json",
    "target-files-postflight.json",
    "source-bundle-postflight.json",
    "terminal.json",
)
HEX_40 = re.compile(r"[0-9a-f]{40}")


class FreshBootRecoveryResultResolutionBindingRequest(Protocol):
    repository_root: Path
    expected_repository_head: str
    target_uuid: str
    target_name: str
    guest_terminal_path: str
    guest_phase_path: str
    prior_fresh_boot_recovery_result_resolution_root: Path
    prior_fresh_boot_recovery_result_resolution_manifest_sha256: str
    prior_fresh_boot_recovery_result_resolution_attempt_id: str


@dataclass(frozen=True)
class FreshBootRecoveryResultResolutionResultBinding:
    evidence: dict[str, object]
    upstream: resolution_control.FreshBootRecoveryResultResolutionBinding
    prior_repository_head: str
    current_boot_id_sha256: str
    prior_boot_id_sha256: str
    prior_backend_pid: int


def validate_fresh_boot_recovery_result_resolution_result_bindings(
    request: FreshBootRecoveryResultResolutionBindingRequest,
) -> FreshBootRecoveryResultResolutionResultBinding:
    if (
        request.prior_fresh_boot_recovery_result_resolution_attempt_id
        != REQUIRED_PRIOR_ATTEMPT_ID
        or request.prior_fresh_boot_recovery_result_resolution_manifest_sha256
        != REQUIRED_PRIOR_MANIFEST_SHA256
    ):
        raise ValueError("required-prior-recovery-result-resolution-mismatch")

    upstream = resolution_control.validate_result_resolution_bindings(request)
    binding_path = request.repository_root / BINDINGS_RELATIVE_PATH
    network_ready._require_committed_regular(
        binding_path, "fresh_boot_recovery_result_resolution_bindings"
    )

    root = request.prior_fresh_boot_recovery_result_resolution_root
    manifest = root / "files.sha256"
    if network_ready._sha256_file(manifest) != REQUIRED_PRIOR_MANIFEST_SHA256:
        raise ValueError("prior-recovery-result-resolution-manifest-invalid")
    entries = start_control._verify_sha256_manifest(root, manifest)
    if (
        entries != len(REQUIRED_ENTRY_NAMES)
        or runtime_bindings._manifest_names(manifest) != REQUIRED_ENTRY_NAMES
    ):
        raise ValueError("prior-recovery-result-resolution-entry-set-invalid")
    _require_private_evidence_tree(root, manifest)

    prior_repository_head = _validate_request_and_binding(request, upstream, root)
    _validate_source_target(request, root)
    handle_signature = _validate_host_identity(request, upstream, root)
    guest_terminal = _validate_result_and_phase(request, upstream, root)
    _validate_terminal(request, upstream, root, guest_terminal)
    _validate_no_forbidden_commands(root)
    runtime_control._require_no_raw_operation_id(root)

    return FreshBootRecoveryResultResolutionResultBinding(
        evidence={
            "current_boot_id_sha256": upstream.current_boot_id_sha256,
            "format": EVIDENCE_FORMAT,
            "fresh_boot_recovery_result_resolution_bindings_sha256": (
                network_ready._sha256_file(binding_path)
            ),
            "guest_recovery_outcome": "recovery-qualified",
            "maintenance_resume_invocations": 0,
            "prior_boot_id_sha256": upstream.prior_boot_id_sha256,
            "prior_recovery_result_resolution_attempt_id": (
                request.prior_fresh_boot_recovery_result_resolution_attempt_id
            ),
            "prior_recovery_result_resolution_backend_pid": (
                upstream.prior_backend_pid
            ),
            "prior_recovery_result_resolution_entries_verified": entries,
            "prior_recovery_result_resolution_handle_sha256": handle_signature[0],
            "prior_recovery_result_resolution_handle_size": handle_signature[1],
            "prior_recovery_result_resolution_manifest_sha256": (
                request.prior_fresh_boot_recovery_result_resolution_manifest_sha256
            ),
            "prior_recovery_result_resolution_repository_head": (
                prior_repository_head
            ),
            "repository_clean": True,
            "repository_head": request.expected_repository_head,
            "result_authority": "double-readback-plus-phase-complete",
            "target_uuid": request.target_uuid,
            "transaction": "artifacts-staged-preserved-no-resume",
        },
        upstream=upstream,
        prior_repository_head=prior_repository_head,
        current_boot_id_sha256=upstream.current_boot_id_sha256,
        prior_boot_id_sha256=upstream.prior_boot_id_sha256,
        prior_backend_pid=upstream.prior_backend_pid,
    )


def _require_private_evidence_tree(root: Path, manifest: Path) -> None:
    root_info = root.lstat()
    if (
        not stat.S_ISDIR(root_info.st_mode)
        or stat.S_ISLNK(root_info.st_mode)
        or stat.S_IMODE(root_info.st_mode) != 0o700
    ):
        raise ValueError("prior-recovery-result-resolution-root-invalid")
    for name in (*runtime_bindings._manifest_names(manifest), "files.sha256"):
        info = (root / name).lstat()
        if (
            not stat.S_ISREG(info.st_mode)
            or stat.S_ISLNK(info.st_mode)
            or stat.S_IMODE(info.st_mode) != 0o600
            or info.st_nlink != 1
        ):
            raise ValueError(
                "prior-recovery-result-resolution-file-identity-invalid"
            )


def _validate_request_and_binding(
    request: FreshBootRecoveryResultResolutionBindingRequest,
    upstream: resolution_control.FreshBootRecoveryResultResolutionBinding,
    root: Path,
) -> str:
    recorded_request = network_ready._read_json(root / "request.json")
    prior_repository_head = recorded_request.get("expected_repository_head")
    if (
        not isinstance(prior_repository_head, str)
        or not HEX_40.fullmatch(prior_repository_head)
    ):
        raise ValueError("prior-recovery-result-resolution-head-invalid")
    expected_request = resolution_control.FreshBootRecoveryResultResolutionRequest.as_json(
        request
    )
    expected_request["expected_repository_head"] = prior_repository_head
    if recorded_request != expected_request:
        raise ValueError("prior-recovery-result-resolution-request-invalid")

    expected_binding = dict(upstream.evidence)
    expected_binding["repository_head"] = prior_repository_head
    if network_ready._read_json(root / "binding-preflight.json") != expected_binding:
        raise ValueError("prior-recovery-result-resolution-binding-invalid")
    return prior_repository_head


def _validate_source_target(
    request: FreshBootRecoveryResultResolutionBindingRequest, root: Path
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
        raise ValueError("prior-recovery-result-resolution-source-invalid")
    before = network_ready._read_json(root / "target-files-preflight.json")
    after = network_ready._read_json(root / "target-files-postflight.json")
    if (
        before != after
        or before.get("format") != network_ready.EVIDENCE_FORMAT
        or before.get("target_package_name") != request.target_package_path.name
        or before.get("target_package_path_sha256")
        != network_ready._sha256_text(str(request.target_package_path))
    ):
        raise ValueError("prior-recovery-result-resolution-target-invalid")


def _validate_host_identity(
    request: FreshBootRecoveryResultResolutionBindingRequest,
    upstream: resolution_control.FreshBootRecoveryResultResolutionBinding,
    root: Path,
) -> tuple[str, int]:
    for name in (
        "host-process-preflight.json",
        "host-process-confirmation-001.json",
        "host-process-confirmation-002.json",
        "host-process-confirmation-003.json",
        "host-process-terminal.json",
    ):
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
            raise ValueError("prior-recovery-result-resolution-process-invalid")

    signature: tuple[str, int] | None = None
    names = (
        "target-handle-pid-discovery.json",
        "target-handle-pid-confirmation-001.json",
        "target-handle-pid-confirmation-002.json",
        "target-handle-pid-confirmation-003.json",
        "target-handle-pid-terminal.json",
    )
    for index, name in enumerate(names):
        value = network_ready._read_json(root / name)
        observation = value.get("observation")
        stdout = observation.get("stdout") if isinstance(observation, dict) else None
        expected_argv = (
            network_ready._lsof_argv(request)
            if index == 0
            else runtime_control.targeted_lsof_argv(
                request, upstream.prior_backend_pid
            )
        )
        if (
            value.get("format") != runtime_bindings.EVIDENCE_FORMAT
            or value.get("state") != "present"
            or value.get("backend_pid") != upstream.prior_backend_pid
            or value.get("backend_command") != "QEMULauncher"
            or value.get("process_record_count") != 1
            or value.get("efi_handle_count") != 1
            or value.get("qcow2_handle_count") != 1
            or not isinstance(observation, dict)
            or observation.get("argv") != list(expected_argv)
            or not _successful_observation(observation)
            or not runtime_bindings._complete_stream(stdout)
        ):
            raise ValueError("prior-recovery-result-resolution-handle-invalid")
        assert isinstance(stdout, dict)
        current = (str(stdout["sha256"]), int(stdout["total_bytes"]))
        if signature is None:
            signature = current
        elif current != signature:
            raise ValueError("prior-recovery-result-resolution-handle-drift")
    if signature is None:
        raise ValueError("prior-recovery-result-resolution-handle-missing")
    return signature


def _validate_result_and_phase(
    request: FreshBootRecoveryResultResolutionBindingRequest,
    upstream: resolution_control.FreshBootRecoveryResultResolutionBinding,
    root: Path,
) -> dict[str, object]:
    signature: tuple[str, int] | None = None
    for index in (1, 2):
        value = network_ready._read_json(
            root / f"existing-recovery-result-readback-{index}.json"
        )
        stdout = value.get("stdout")
        if (
            value.get("argv")
            != [
                "utmctl",
                "file",
                "pull",
                request.target_uuid,
                request.guest_terminal_path,
            ]
            or not _successful_observation(value)
            or not isinstance(stdout, dict)
            or stdout.get("truncated") is not False
        ):
            raise ValueError("prior-recovery-result-readback-invalid")
        current = (str(stdout.get("sha256")), int(stdout.get("total_bytes", -1)))
        if signature is None:
            signature = current
        elif current != signature:
            raise ValueError("prior-recovery-result-readback-drift")
    recorded = network_ready._read_json(root / "existing-recovery-result.json")
    expected_payload = guest_probe.canonical_json(recorded)
    expected_signature = (hashlib.sha256(expected_payload).hexdigest(), len(expected_payload))
    if signature != expected_signature:
        raise ValueError("prior-recovery-result-payload-invalid")
    parsed = resolution_control.recovery_control.parse_guest_terminal(
        expected_payload, request, upstream
    )
    if parsed != recorded or recorded.get("outcome") != "recovery-qualified":
        raise ValueError("prior-recovery-result-not-qualified")

    phase_payload = guest_probe.canonical_json(
        {"format": guest_probe.PHASE_FORMAT, "phase": "complete"}
    )
    phase = network_ready._read_json(root / "existing-recovery-phase-readback.json")
    stdout = phase.get("stdout")
    if (
        phase.get("argv")
        != [
            "utmctl",
            "file",
            "pull",
            request.target_uuid,
            request.guest_phase_path,
        ]
        or not _successful_observation(phase)
        or not isinstance(stdout, dict)
        or stdout.get("sha256") != hashlib.sha256(phase_payload).hexdigest()
        or stdout.get("total_bytes") != len(phase_payload)
        or stdout.get("truncated") is not False
    ):
        raise ValueError("prior-recovery-result-phase-invalid")
    return recorded


def _validate_terminal(
    request: FreshBootRecoveryResultResolutionBindingRequest,
    upstream: resolution_control.FreshBootRecoveryResultResolutionBinding,
    root: Path,
    guest_terminal: dict[str, object],
) -> None:
    expected = {
        "automatic_cleanup": "not-performed",
        "automatic_quit": "not-performed",
        "automatic_retry": "not-performed",
        "automatic_stop": "not-performed",
        "backend_pid": upstream.prior_backend_pid,
        "business_guest_action": "not-performed",
        "file_pull_invocations": 3,
        "file_push_invocations": 0,
        "format": resolution_control.EVIDENCE_FORMAT,
        "guest_exec_invocations": 0,
        "guest_probe_invocations": 0,
        "guest_recovery_outcome": guest_terminal["outcome"],
        "identity_observation_count": 3,
        "inventory_probe_invocations": 0,
        "maintenance_resume_invocations": 0,
        "operation_id": "existing-receipt-hash-only",
        "outcome": "recovery-qualified",
        "phase_readback_invocations": 1,
        "plain_utmctl_list": "not-performed",
        "plain_utmctl_start": "not-performed",
        "plain_utmctl_status": "not-performed",
        "prior_fresh_boot_recovery_preflight_attempt_id": (
            resolution_control.REQUIRED_PRIOR_ATTEMPT_ID
        ),
        "reason": "deferred-readback-recovery-qualified",
        "result_readback_invocations": 2,
        "result_resolution_attempt_id": REQUIRED_PRIOR_ATTEMPT_ID,
        "target_name": request.target_name,
        "target_uuid": request.target_uuid,
        "terminal_relevant_host_process_count": 0,
        "transaction": "artifacts-staged-preserved-no-resume",
    }
    if network_ready._read_json(root / "terminal.json") != expected:
        raise ValueError("prior-recovery-result-resolution-terminal-invalid")


def _validate_no_forbidden_commands(root: Path) -> None:
    forbidden = {"dpkg", "dpkg-query", "exec", "probe", "push", "quit", "resume", "retry", "start", "status", "stop"}
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
            if argv[:2] in (["utmctl", "list"], ["utmctl", "start"], ["utmctl", "status"]) or any(
                token in forbidden for token in argv
            ):
                raise ValueError("prior-recovery-result-resolution-forbidden-command")


def _successful_observation(value: object) -> bool:
    return (
        isinstance(value, dict)
        and value.get("exit_code") == 0
        and value.get("timed_out") is False
        and runtime_bindings._empty_stream(value.get("stderr"))
    )
