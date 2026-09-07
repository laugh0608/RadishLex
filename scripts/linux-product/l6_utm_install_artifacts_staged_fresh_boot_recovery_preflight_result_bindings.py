#!/usr/bin/env python3
from __future__ import annotations

import hashlib
import re
import stat
from dataclasses import dataclass
from pathlib import Path
from typing import Protocol

import l6_utm_guest_network_ready as network_ready
import l6_utm_install_artifacts_staged_fresh_boot_recovery_preflight as control
import l6_utm_install_artifacts_staged_fresh_boot_recovery_preflight_bindings as bindings
import l6_utm_install_artifacts_staged_new_boot_recovery_preflight as recovery_control
import l6_utm_install_artifacts_staged_new_boot_recovery_preflight_result_bindings as legacy_result
import l6_utm_install_artifacts_staged_runtime_resolution as runtime_control
import l6_utm_install_artifacts_staged_runtime_resolution_bindings as runtime_bindings
import l6_utm_launch_transport_v2 as launch_transport
import l6_utm_start_once as start_control


EVIDENCE_FORMAT = (
    "radishlex-linux-l6-utm-install-artifacts-staged-"
    "fresh-boot-recovery-preflight-result-binding-v1"
)
BINDINGS_RELATIVE_PATH = Path(
    "scripts/linux-product/"
    "l6_utm_install_artifacts_staged_fresh_boot_recovery_preflight_"
    "result_bindings.py"
)
REQUIRED_PRIOR_MANIFEST_SHA256 = (
    "39586a3bb662d9a28a7a2a6e5721b49c4b5395baf47f4ad03bef3da1a20e1bed"
)
REQUIRED_PRIOR_ATTEMPT_ID = bindings.REQUIRED_ATTEMPT_ID
RESULT_MISSING_STDERR_SHA256 = (
    "96c1c6b83cf60477f05f584380252e5700b33283887799a2cacbe35f159d5819"
)
RESULT_MISSING_STDERR_SIZE = 322
HEX_40 = re.compile(r"[0-9a-f]{40}")
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
    "source-bundle-postflight.json",
    "terminal.json",
)


class FreshBootRecoveryPreflightResultBindingRequest(
    bindings.FreshBootRecoveryPreflightBindingRequest, Protocol
):
    prior_fresh_boot_recovery_preflight_root: Path
    prior_fresh_boot_recovery_preflight_manifest_sha256: str
    prior_fresh_boot_recovery_preflight_attempt_id: str


@dataclass(frozen=True)
class FreshBootRecoveryPreflightResultBinding:
    evidence: dict[str, object]
    upstream: bindings.FreshBootRecoveryPreflightBinding
    prior_repository_head: str
    current_boot_id_sha256: str
    prior_boot_id_sha256: str
    prior_backend_pid: int


def validate_fresh_boot_recovery_preflight_result_bindings(
    request: FreshBootRecoveryPreflightResultBindingRequest,
) -> FreshBootRecoveryPreflightResultBinding:
    if (
        request.prior_fresh_boot_recovery_preflight_attempt_id
        != REQUIRED_PRIOR_ATTEMPT_ID
        or request.prior_fresh_boot_recovery_preflight_manifest_sha256
        != REQUIRED_PRIOR_MANIFEST_SHA256
    ):
        raise ValueError("required-prior-fresh-boot-recovery-preflight-mismatch")

    upstream = bindings.validate_fresh_boot_recovery_preflight_bindings(request)
    binding_path = request.repository_root / BINDINGS_RELATIVE_PATH
    network_ready._require_committed_regular(
        binding_path, "fresh_boot_recovery_preflight_result_bindings"
    )

    root = request.prior_fresh_boot_recovery_preflight_root
    manifest = root / "files.sha256"
    if network_ready._sha256_file(manifest) != REQUIRED_PRIOR_MANIFEST_SHA256:
        raise ValueError("prior-fresh-boot-recovery-preflight-manifest-invalid")
    entries = start_control._verify_sha256_manifest(root, manifest)
    if (
        entries != len(REQUIRED_ENTRY_NAMES)
        or runtime_bindings._manifest_names(manifest) != REQUIRED_ENTRY_NAMES
    ):
        raise ValueError("prior-fresh-boot-recovery-preflight-entry-set-invalid")
    _require_private_evidence_tree(root, manifest)

    prior_repository_head = _validate_request_and_binding(
        request, upstream, root
    )
    legacy_result._validate_source_target(request, root)
    handle_signature = _validate_host_identity(request, upstream, root)
    _validate_delivery(request, upstream, root)
    _validate_probe_and_missing_result(request, upstream, root)
    _validate_terminal(request, upstream, root)
    _validate_no_forbidden_commands(root)
    runtime_control._require_no_raw_operation_id(root)

    return FreshBootRecoveryPreflightResultBinding(
        evidence={
            "current_boot_id_sha256": upstream.current_boot_id_sha256,
            "format": EVIDENCE_FORMAT,
            "fresh_boot_recovery_preflight_result_bindings_sha256": (
                network_ready._sha256_file(binding_path)
            ),
            "guest_probe_transport_exit_code": 0,
            "guest_recovery_outcome": "not-observed",
            "host_recorded_outcome": "state-indeterminate",
            "prior_boot_id_sha256": upstream.prior_boot_id_sha256,
            "prior_fresh_boot_recovery_preflight_attempt_id": (
                request.prior_fresh_boot_recovery_preflight_attempt_id
            ),
            "prior_fresh_boot_recovery_preflight_backend_pid": (
                upstream.prior_backend_pid
            ),
            "prior_fresh_boot_recovery_preflight_entries_verified": entries,
            "prior_fresh_boot_recovery_preflight_handle_sha256": (
                handle_signature[0]
            ),
            "prior_fresh_boot_recovery_preflight_handle_size": (
                handle_signature[1]
            ),
            "prior_fresh_boot_recovery_preflight_manifest_sha256": (
                request.prior_fresh_boot_recovery_preflight_manifest_sha256
            ),
            "prior_fresh_boot_recovery_preflight_repository_head": (
                prior_repository_head
            ),
            "repository_clean": True,
            "repository_head": request.expected_repository_head,
            "result_authority": "marker-present-result-not-observed",
            "target_uuid": request.target_uuid,
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
        raise ValueError("prior-fresh-boot-recovery-preflight-root-invalid")
    for name in (*runtime_bindings._manifest_names(manifest), "files.sha256"):
        info = (root / name).lstat()
        if (
            not stat.S_ISREG(info.st_mode)
            or stat.S_ISLNK(info.st_mode)
            or stat.S_IMODE(info.st_mode) != 0o600
            or info.st_nlink != 1
        ):
            raise ValueError(
                "prior-fresh-boot-recovery-preflight-file-identity-invalid"
            )


def _validate_request_and_binding(
    request: FreshBootRecoveryPreflightResultBindingRequest,
    upstream: bindings.FreshBootRecoveryPreflightBinding,
    root: Path,
) -> str:
    recorded_request = network_ready._read_json(root / "request.json")
    prior_repository_head = recorded_request.get("expected_repository_head")
    if (
        not isinstance(prior_repository_head, str)
        or not HEX_40.fullmatch(prior_repository_head)
    ):
        raise ValueError(
            "prior-fresh-boot-recovery-preflight-repository-head-invalid"
        )
    expected_request = control.FreshBootRecoveryPreflightRequest.as_json(
        request
    )
    expected_request["expected_repository_head"] = prior_repository_head
    if recorded_request != expected_request:
        raise ValueError("prior-fresh-boot-recovery-preflight-request-invalid")

    expected_binding = dict(upstream.evidence)
    expected_binding["repository_head"] = prior_repository_head
    if network_ready._read_json(root / "binding-preflight.json") != expected_binding:
        raise ValueError("prior-fresh-boot-recovery-preflight-binding-invalid")
    return prior_repository_head


def _validate_host_identity(
    request: FreshBootRecoveryPreflightResultBindingRequest,
    upstream: bindings.FreshBootRecoveryPreflightBinding,
    root: Path,
) -> tuple[str, int]:
    for name in (
        "host-process-preflight.json",
        "host-process-confirmation-001.json",
        "host-process-confirmation-002.json",
        "host-process-confirmation-003.json",
        "host-process-agent-ready.json",
        "host-process-pre-probe.json",
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
            raise ValueError(
                "prior-fresh-boot-recovery-preflight-process-invalid"
            )

    signature: tuple[str, int] | None = None
    handle_names = (
        "target-handle-pid-discovery.json",
        "target-handle-pid-confirmation-001.json",
        "target-handle-pid-confirmation-002.json",
        "target-handle-pid-confirmation-003.json",
        "target-handle-agent-ready.json",
        "target-handle-pre-probe.json",
    )
    for index, name in enumerate(handle_names):
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
            raise ValueError(
                "prior-fresh-boot-recovery-preflight-handle-invalid"
            )
        assert isinstance(stdout, dict)
        current = (str(stdout["sha256"]), int(stdout["total_bytes"]))
        if signature is None:
            signature = current
        elif current != signature:
            raise ValueError(
                "prior-fresh-boot-recovery-preflight-handle-drift"
            )

    readiness = network_ready._read_json(root / "guest-agent-readiness-001.json")
    if (
        readiness.get("argv") != list(recovery_control.readiness_argv(request))
        or not _successful_empty_observation(readiness)
        or network_ready._read_json(root / "guest-agent-ready.json")
        != {
            "attempt": 1,
            "format": recovery_control.EVIDENCE_FORMAT,
            "predicate": "canonical-boot-id-readable",
            "state": "ready",
            "target_uuid": request.target_uuid,
        }
    ):
        raise ValueError(
            "prior-fresh-boot-recovery-preflight-readiness-invalid"
        )
    if signature is None:
        raise ValueError("prior-fresh-boot-recovery-preflight-handle-missing")
    return signature


def _validate_delivery(
    request: FreshBootRecoveryPreflightResultBindingRequest,
    upstream: bindings.FreshBootRecoveryPreflightBinding,
    root: Path,
) -> None:
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
            request.guest_recovery_root,
        ]
        or not _successful_empty_observation(root_create)
    ):
        raise ValueError(
            "prior-fresh-boot-recovery-preflight-root-create-invalid"
        )

    for label, incoming, final, payload in (
        (
            "frozen-resume-driver",
            request.guest_resume_driver_incoming,
            request.guest_resume_driver_path,
            upstream.resume_driver_bytes,
        ),
        (
            "recovery-preflight-probe",
            request.guest_probe_incoming,
            request.guest_probe_path,
            upstream.probe_bytes,
        ),
    ):
        for action, argv in (
            ("push", ["utmctl", "file", "push", request.target_uuid, incoming]),
            (
                "chown",
                [
                    "utmctl",
                    "exec",
                    request.target_uuid,
                    "--cmd",
                    "/bin/chown",
                    "root:root",
                    incoming,
                ],
            ),
            (
                "chmod",
                [
                    "utmctl",
                    "exec",
                    request.target_uuid,
                    "--cmd",
                    "/bin/chmod",
                    "0600",
                    incoming,
                ],
            ),
            (
                "publish",
                [
                    "utmctl",
                    "exec",
                    request.target_uuid,
                    "--cmd",
                    "/bin/mv",
                    "--",
                    incoming,
                    final,
                ],
            ),
        ):
            value = network_ready._read_json(root / f"guest-{label}-{action}.json")
            if value.get("argv") != argv or not _successful_empty_observation(value):
                raise ValueError(
                    f"prior-fresh-boot-recovery-preflight-{label}-{action}-invalid"
                )
        readback = network_ready._read_json(root / f"guest-{label}-readback.json")
        stdout = readback.get("stdout")
        if (
            readback.get("argv")
            != ["utmctl", "file", "pull", request.target_uuid, final]
            or not _successful_observation(readback)
            or not isinstance(stdout, dict)
            or stdout.get("sha256") != hashlib.sha256(payload).hexdigest()
            or stdout.get("total_bytes") != len(payload)
            or stdout.get("truncated") is not False
        ):
            raise ValueError(
                f"prior-fresh-boot-recovery-preflight-{label}-readback-invalid"
            )


def _validate_probe_and_missing_result(
    request: FreshBootRecoveryPreflightResultBindingRequest,
    upstream: bindings.FreshBootRecoveryPreflightBinding,
    root: Path,
) -> None:
    probe = network_ready._read_json(root / "guest-recovery-preflight-once.json")
    observation = probe.get("observation")
    if (
        probe.get("format") != recovery_control.EVIDENCE_FORMAT
        or probe.get("raw_output_persisted") is not False
        or not isinstance(observation, dict)
        or observation.get("argv")
        != list(recovery_control.probe_argv(request, upstream))
        or not _successful_empty_observation(observation)
    ):
        raise ValueError(
            "prior-fresh-boot-recovery-preflight-probe-invalid"
        )

    marker_payload = recovery_control.marker_bytes(request, upstream)
    marker = network_ready._read_json(root / "guest-recovery-marker-readback.json")
    marker_stdout = marker.get("stdout")
    if (
        marker.get("argv")
        != [
            "utmctl",
            "file",
            "pull",
            request.target_uuid,
            request.guest_marker_path,
        ]
        or not _successful_observation(marker)
        or not isinstance(marker_stdout, dict)
        or marker_stdout.get("sha256")
        != hashlib.sha256(marker_payload).hexdigest()
        or marker_stdout.get("total_bytes") != len(marker_payload)
        or marker_stdout.get("truncated") is not False
    ):
        raise ValueError(
            "prior-fresh-boot-recovery-preflight-marker-invalid"
        )

    result = network_ready._read_json(root / "guest-recovery-result-readback-1.json")
    stdout = result.get("stdout")
    stderr = result.get("stderr")
    if (
        result.get("argv")
        != [
            "utmctl",
            "file",
            "pull",
            request.target_uuid,
            request.guest_terminal_path,
        ]
        or result.get("exit_code") != 0
        or result.get("timed_out") is not False
        or not runtime_bindings._empty_stream(stdout)
        or not isinstance(stderr, dict)
        or stderr.get("sha256") != RESULT_MISSING_STDERR_SHA256
        or stderr.get("total_bytes") != RESULT_MISSING_STDERR_SIZE
        or stderr.get("truncated") is not False
    ):
        raise ValueError(
            "prior-fresh-boot-recovery-preflight-missing-result-invalid"
        )


def _validate_terminal(
    request: FreshBootRecoveryPreflightResultBindingRequest,
    upstream: bindings.FreshBootRecoveryPreflightBinding,
    root: Path,
) -> None:
    expected = {
        "agent_readiness_invocations": 1,
        "automatic_cleanup": "not-performed",
        "automatic_quit": "not-performed",
        "automatic_retry": "not-performed",
        "automatic_stop": "not-performed",
        "backend_pid": upstream.prior_backend_pid,
        "business_guest_action": "not-performed",
        "file_pull_invocations": 4,
        "file_push_invocations": 2,
        "format": recovery_control.EVIDENCE_FORMAT,
        "guest_exec_invocations": 8,
        "guest_probe_invocations": 1,
        "guest_probe_transport_exit_code": 0,
        "guest_recovery_outcome": "not-observed",
        "identity_observation_count": 3,
        "inventory_probe_invocations": 0,
        "maintenance_resume_invocations": 0,
        "operation_id": "existing-receipt-hash-only",
        "outcome": "state-indeterminate",
        "plain_utmctl_list": "not-performed",
        "plain_utmctl_start": "not-performed",
        "plain_utmctl_status": "not-performed",
        "reason": (
            "guest-recovery-result-readback-1:"
            "guest-recovery-result-readback-1-stderr-not-empty"
        ),
        "recovery_preflight_attempt_id": REQUIRED_PRIOR_ATTEMPT_ID,
        "result_readback_invocations": 1,
        "target_name": request.target_name,
        "target_uuid": request.target_uuid,
        "terminal_relevant_host_process_count": 0,
        "transaction": "artifacts-staged-preserved-no-resume",
    }
    if network_ready._read_json(root / "terminal.json") != expected:
        raise ValueError(
            "prior-fresh-boot-recovery-preflight-terminal-invalid"
        )


def _validate_no_forbidden_commands(root: Path) -> None:
    forbidden_guest_tokens = {
        "dpkg",
        "dpkg-query",
        "quit",
        "resume",
        "retry",
        "start",
        "status",
        "stop",
    }
    for name in REQUIRED_ENTRY_NAMES:
        value = network_ready._read_json(root / name)
        observations = [value]
        nested = value.get("observation")
        if isinstance(nested, dict):
            observations.append(nested)
        for observation in observations:
            argv = observation.get("argv")
            if not isinstance(argv, list) or not all(
                isinstance(item, str) for item in argv
            ):
                continue
            if argv[:2] in (
                ["utmctl", "list"],
                ["utmctl", "start"],
                ["utmctl", "status"],
            ) or any(token in forbidden_guest_tokens for token in argv):
                raise ValueError(
                    "prior-fresh-boot-recovery-preflight-forbidden-command"
                )


def _successful_observation(value: object) -> bool:
    return (
        isinstance(value, dict)
        and value.get("exit_code") == 0
        and value.get("timed_out") is False
        and runtime_bindings._empty_stream(value.get("stderr"))
    )


def _successful_empty_observation(value: object) -> bool:
    return _successful_observation(value) and runtime_bindings._empty_stream(
        value.get("stdout") if isinstance(value, dict) else None
    )
