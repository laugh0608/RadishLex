#!/usr/bin/env python3
from __future__ import annotations

import base64
import hashlib
import re
import stat
from dataclasses import dataclass
from pathlib import Path
from typing import Protocol

import l6_utm_guest_network_ready as network_ready
import l6_utm_install_artifacts_staged_boot_start_resolution as boot_control
import l6_utm_install_artifacts_staged_guest_agent_resolution as guest_agent_control
import l6_utm_install_artifacts_staged_reactivation as reactivation_control
import l6_utm_install_artifacts_staged_runtime_resolution as runtime_control
import l6_utm_install_artifacts_staged_runtime_resolution_bindings as runtime_bindings
import l6_utm_install_artifacts_staged_transaction_state_resolution as control
import l6_utm_install_artifacts_staged_transaction_state_resolution_bindings as bindings
import l6_utm_launch_transport_v2 as launch_transport
import l6_utm_start_once as start_control


EVIDENCE_FORMAT = (
    "radishlex-linux-l6-utm-install-artifacts-staged-"
    "transaction-state-resolution-result-binding-v1"
)
BINDINGS_RELATIVE_PATH = Path(
    "scripts/linux-product/"
    "l6_utm_install_artifacts_staged_transaction_state_resolution_"
    "result_bindings.py"
)
REQUIRED_PRIOR_MANIFEST_SHA256 = (
    "37058a86c830e8aed040d3ef24893c92fbd7b9ebf08820aa513546be7fe308e4"
)
REQUIRED_PRIOR_ATTEMPT_ID = control.REQUIRED_ATTEMPT_ID
REQUIRED_PRIOR_REPOSITORY_HEAD = "a03edae4cef5c1e0805f31652ffd25d2eb0e6919"
RESULT_MISSING_STDERR_SHA256 = (
    "f47dc5006790abe3e1e082c67da06834afb0c9c107151ebc4b1cf9b4ac5b25b3"
)
RESULT_MISSING_STDERR_SIZE = 291
HEX_40 = re.compile(r"[0-9a-f]{40}")
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
    "guest-transaction-state-probe-once.json",
    "guest-marker-readback.json",
    "guest-result-readback-1.json",
    "source-bundle-postflight.json",
    "terminal.json",
)


class TransactionStateResolutionResultBindingRequest(
    bindings.TransactionStateResolutionBindingRequest, Protocol
):
    prior_transaction_state_resolution_root: Path
    prior_transaction_state_resolution_manifest_sha256: str
    prior_transaction_state_resolution_attempt_id: str


@dataclass(frozen=True)
class TransactionStateResolutionResultBinding:
    evidence: dict[str, object]
    upstream: bindings.TransactionStateResolutionBinding
    prior_repository_head: str
    prior_backend_pid: int
    prior_handle_sha256: str
    prior_handle_size: int
    probe_sha256: str


def validate_transaction_state_resolution_result_bindings(
    request: TransactionStateResolutionResultBindingRequest,
) -> TransactionStateResolutionResultBinding:
    if (
        request.prior_transaction_state_resolution_attempt_id
        != REQUIRED_PRIOR_ATTEMPT_ID
        or request.prior_transaction_state_resolution_manifest_sha256
        != REQUIRED_PRIOR_MANIFEST_SHA256
    ):
        raise ValueError("required-prior-transaction-state-resolution-mismatch")

    upstream = bindings.validate_transaction_state_resolution_bindings(request)
    binding_path = request.repository_root / BINDINGS_RELATIVE_PATH
    network_ready._require_committed_regular(
        binding_path, "transaction_state_resolution_result_bindings"
    )

    root = request.prior_transaction_state_resolution_root
    manifest = root / "files.sha256"
    if network_ready._sha256_file(manifest) != REQUIRED_PRIOR_MANIFEST_SHA256:
        raise ValueError("prior-transaction-state-resolution-manifest-invalid")
    entries = start_control._verify_sha256_manifest(root, manifest)
    if (
        entries != len(REQUIRED_ENTRY_NAMES)
        or runtime_bindings._manifest_names(manifest) != REQUIRED_ENTRY_NAMES
    ):
        raise ValueError("prior-transaction-state-resolution-entry-set-invalid")
    _require_private_evidence_tree(root, manifest)

    prior_repository_head = _validate_request_and_binding(
        request, upstream, root
    )
    _validate_source_target(request, root)
    _validate_inventory(request, upstream, root)
    excluded_processes = _validate_process_evidence(request, root)
    handle_signature = _validate_handle_evidence(request, root)
    _validate_start_and_readiness(request, root)
    probe_sha256 = _validate_probe_and_missing_result(request, upstream, root)
    _validate_terminal(request, upstream, root)
    _validate_no_forbidden_commands(root)
    runtime_control._require_no_raw_operation_id(root)

    return TransactionStateResolutionResultBinding(
        evidence={
            "excluded_generic_qemu_processes": list(excluded_processes),
            "format": EVIDENCE_FORMAT,
            "guest_probe_transport_exit_code": 0,
            "host_recorded_outcome": "state-indeterminate",
            "prior_transaction_state_resolution_attempt_id": (
                request.prior_transaction_state_resolution_attempt_id
            ),
            "prior_transaction_state_resolution_backend_pid": 36343,
            "prior_transaction_state_resolution_entries_verified": entries,
            "prior_transaction_state_resolution_handle_sha256": (
                handle_signature[0]
            ),
            "prior_transaction_state_resolution_handle_size": (
                handle_signature[1]
            ),
            "prior_transaction_state_resolution_manifest_sha256": (
                request.prior_transaction_state_resolution_manifest_sha256
            ),
            "prior_transaction_state_resolution_repository_head": (
                prior_repository_head
            ),
            "repository_clean": True,
            "repository_head": request.expected_repository_head,
            "result_authority": "marker-present-result-not-observed",
            "target_uuid": request.target_uuid,
            "transaction": "state-indeterminate",
            "transaction_state_probe_sha256": probe_sha256,
            "transaction_state_resolution_result_bindings_sha256": (
                network_ready._sha256_file(binding_path)
            ),
            "transaction_state_resolution_result_bindings_size": (
                binding_path.stat().st_size
            ),
        },
        upstream=upstream,
        prior_repository_head=prior_repository_head,
        prior_backend_pid=36343,
        prior_handle_sha256=handle_signature[0],
        prior_handle_size=handle_signature[1],
        probe_sha256=probe_sha256,
    )


def _require_private_evidence_tree(root: Path, manifest: Path) -> None:
    root_info = root.lstat()
    if (
        not stat.S_ISDIR(root_info.st_mode)
        or stat.S_ISLNK(root_info.st_mode)
        or stat.S_IMODE(root_info.st_mode) != 0o700
    ):
        raise ValueError("prior-transaction-state-resolution-root-invalid")
    for name in (*runtime_bindings._manifest_names(manifest), "files.sha256"):
        info = (root / name).lstat()
        if (
            not stat.S_ISREG(info.st_mode)
            or stat.S_ISLNK(info.st_mode)
            or stat.S_IMODE(info.st_mode) != 0o600
            or info.st_nlink != 1
            or info.st_uid != root_info.st_uid
            or info.st_gid != root_info.st_gid
        ):
            raise ValueError(
                "prior-transaction-state-resolution-file-identity-invalid"
            )


def _validate_request_and_binding(
    request: TransactionStateResolutionResultBindingRequest,
    upstream: bindings.TransactionStateResolutionBinding,
    root: Path,
) -> str:
    recorded_request = network_ready._read_json(root / "request.json")
    prior_repository_head = recorded_request.get("expected_repository_head")
    if (
        prior_repository_head != REQUIRED_PRIOR_REPOSITORY_HEAD
        or not isinstance(prior_repository_head, str)
        or not HEX_40.fullmatch(prior_repository_head)
        or prior_repository_head == request.expected_repository_head
    ):
        raise ValueError("prior-transaction-state-resolution-head-invalid")
    expected_request = control.TransactionStateResolutionRequest.as_json(request)
    expected_request["expected_repository_head"] = prior_repository_head
    if recorded_request != expected_request:
        raise ValueError("prior-transaction-state-resolution-request-invalid")

    expected_binding = dict(upstream.evidence)
    expected_binding["repository_head"] = prior_repository_head
    if network_ready._read_json(root / "binding-preflight.json") != expected_binding:
        raise ValueError("prior-transaction-state-resolution-binding-invalid")
    return prior_repository_head


def _validate_source_target(
    request: TransactionStateResolutionResultBindingRequest, root: Path
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
        raise ValueError("prior-transaction-state-resolution-source-invalid")
    identities = [
        network_ready._read_json(root / name)
        for name in (
            "target-files-preflight.json",
            "target-files-ready.json",
            "target-files-started.json",
            "target-files-agent-ready.json",
        )
    ]
    if (
        any(value != identities[0] for value in identities[1:])
        or identities[0].get("format") != network_ready.EVIDENCE_FORMAT
        or identities[0].get("target_package_name")
        != request.target_package_path.name
        or identities[0].get("target_package_path_sha256")
        != network_ready._sha256_text(str(request.target_package_path))
    ):
        raise ValueError("prior-transaction-state-resolution-target-invalid")


def _validate_inventory(
    request: TransactionStateResolutionResultBindingRequest,
    upstream: bindings.TransactionStateResolutionBinding,
    root: Path,
) -> None:
    observation = _full_observation(
        network_ready._read_json(root / "utmctl-list-once.json")
    )
    inventory = start_control.parse_utmctl_list(observation)
    if (
        reactivation_control._require_inventory(
            inventory, upstream.baseline_inventory, request
        )
        != "stopped"
        or network_ready._read_json(root / "inventory-classification.json")
        != {
            "format": control.EVIDENCE_FORMAT,
            "other_registered_vm_count": 20,
            "other_registered_vms": "all-stopped",
            "registered_vm_count": 21,
            "target_registered_status": "stopped",
            "target_uuid": request.target_uuid,
        }
    ):
        raise ValueError("prior-transaction-state-resolution-inventory-invalid")


def _validate_process_evidence(
    request: TransactionStateResolutionResultBindingRequest, root: Path
) -> tuple[dict[str, object], ...]:
    del request
    names = ["host-process-preflight.json"]
    names.extend(
        f"host-process-quiescence-{index:03d}.json" for index in range(1, 4)
    )
    names.append("host-process-runtime-001.json")
    names.extend(
        f"host-process-confirmation-{index:03d}.json" for index in range(1, 4)
    )
    names.extend(
        f"host-process-readiness-{index:03d}.json" for index in range(1, 11)
    )
    excluded: tuple[dict[str, object], ...] | None = None
    for name in names:
        value = network_ready._read_json(root / name)
        observation = value.get("observation")
        current = value.get("excluded_generic_qemu_processes")
        if (
            value.get("format") != runtime_control.EVIDENCE_FORMAT
            or value.get("process_scope")
            != control.TransactionStateProbeWorkflow.process_scope
            or value.get("relevant_process_count") != 0
            or value.get("relevant_processes") != []
            or value.get("excluded_generic_qemu_process_count") != 1
            or not isinstance(current, list)
            or len(current) != 1
            or current[0].get("role") != "qemu"
            or not isinstance(observation, dict)
            or observation.get("argv") != list(launch_transport.PROCESS_COMMAND)
            or not _successful_metadata(observation)
            or not runtime_bindings._complete_stream(observation.get("stdout"))
        ):
            raise ValueError("prior-transaction-state-resolution-process-invalid")
        current_tuple = tuple(current)
        if excluded is None:
            excluded = current_tuple
        elif current_tuple != excluded:
            raise ValueError(
                "prior-transaction-state-resolution-excluded-process-drift"
            )
    if excluded is None:
        raise ValueError("prior-transaction-state-resolution-process-missing")
    return excluded


def _validate_handle_evidence(
    request: TransactionStateResolutionResultBindingRequest, root: Path
) -> tuple[str, int]:
    for name in (
        "target-handles-preflight.json",
        "target-handles-quiescence-001.json",
        "target-handles-quiescence-002.json",
        "target-handles-quiescence-003.json",
    ):
        value = network_ready._read_json(root / name)
        observation = value.get("observation")
        if (
            value.get("format") != control.EVIDENCE_FORMAT
            or value.get("state") != "absent"
            or value.get("backend_command") is not None
            or value.get("process_record_count") != 0
            or value.get("efi_handle_count") != 0
            or value.get("qcow2_handle_count") != 0
            or not isinstance(observation, dict)
            or observation.get("argv") != list(network_ready._lsof_argv(request))
            or observation.get("exit_code") != 1
            or observation.get("timed_out") is not False
            or not runtime_bindings._empty_stream(observation.get("stdout"))
            or not runtime_bindings._empty_stream(observation.get("stderr"))
        ):
            raise ValueError(
                "prior-transaction-state-resolution-absent-handle-invalid"
            )

    names = (
        "target-handles-runtime-001.json",
        "target-handle-pid-discovery.json",
        "target-handle-pid-confirmation-001.json",
        "target-handle-pid-confirmation-002.json",
        "target-handle-pid-confirmation-003.json",
        *(f"target-handle-readiness-{index:03d}.json" for index in range(1, 11)),
    )
    signature: tuple[str, int] | None = None
    for index, name in enumerate(names):
        value = network_ready._read_json(root / name)
        observation = value.get("observation")
        stdout = observation.get("stdout") if isinstance(observation, dict) else None
        expected_backend_pid = None if index == 0 else 36343
        expected_format = (
            runtime_control.EVIDENCE_FORMAT
            if 1 <= index <= 4
            else control.EVIDENCE_FORMAT
        )
        expected_argv = (
            network_ready._lsof_argv(request)
            if index < 2
            else runtime_control.targeted_lsof_argv(request, 36343)
        )
        if (
            value.get("format") != expected_format
            or value.get("state") != "present"
            or value.get("backend_pid") != expected_backend_pid
            or value.get("backend_command") != "QEMULauncher"
            or value.get("process_record_count") != 1
            or value.get("efi_handle_count") != 1
            or value.get("qcow2_handle_count") != 1
            or not isinstance(observation, dict)
            or observation.get("argv") != list(expected_argv)
            or not _successful_metadata(observation)
            or not runtime_bindings._complete_stream(stdout)
        ):
            raise ValueError(
                "prior-transaction-state-resolution-present-handle-invalid"
            )
        assert isinstance(stdout, dict)
        current = (str(stdout["sha256"]), int(stdout["total_bytes"]))
        if signature is None:
            signature = current
        elif current != signature:
            raise ValueError("prior-transaction-state-resolution-handle-drift")
    if signature is None:
        raise ValueError("prior-transaction-state-resolution-handle-missing")
    return signature


def _validate_start_and_readiness(
    request: TransactionStateResolutionResultBindingRequest, root: Path
) -> None:
    start = _full_observation(
        network_ready._read_json(root / "foreground-start-once.json")
    )
    if (
        start.argv != launch_transport.transport_argv(request)
        or start.exit_code != 0
        or start.timed_out
        or start.stdout.total_bytes != 0
        or start.stderr.total_bytes != 0
    ):
        raise ValueError("prior-transaction-state-resolution-start-invalid")
    for index in range(1, 11):
        observation = _full_observation(
            network_ready._read_json(
                root / f"guest-agent-readiness-{index:03d}.json"
            )
        )
        if observation.argv != guest_agent_control.readiness_argv(request):
            raise ValueError(
                "prior-transaction-state-resolution-readiness-argv-invalid"
            )
        state = guest_agent_control.classify_agent_readiness(observation, request)
        expected = "ready" if index == 10 else "unavailable"
        if state != expected:
            raise ValueError(
                "prior-transaction-state-resolution-readiness-state-invalid"
            )
    if network_ready._read_json(root / "guest-agent-ready.json") != {
        "attempt": 10,
        "format": control.EVIDENCE_FORMAT,
        "predicate": "canonical-boot-id-readable",
        "state": "ready",
        "target_uuid": request.target_uuid,
    }:
        raise ValueError("prior-transaction-state-resolution-ready-invalid")


def _validate_probe_and_missing_result(
    request: TransactionStateResolutionResultBindingRequest,
    upstream: bindings.TransactionStateResolutionBinding,
    root: Path,
) -> str:
    root_create = _full_observation(
        network_ready._read_json(root / "guest-control-root-create.json")
    )
    if (
        root_create.argv
        != (
            "utmctl",
            "exec",
            request.target_uuid,
            "--cmd",
            "/bin/mkdir",
            "-m",
            "0700",
            request.guest_control_root,
        )
        or not _successful_empty_observation(root_create)
    ):
        raise ValueError("prior-transaction-state-resolution-root-create-invalid")

    push = _full_observation(network_ready._read_json(root / "guest-probe-push.json"))
    normalize = _full_observation(
        network_ready._read_json(root / "guest-probe-normalize.json")
    )
    if (
        push.argv
        != (
            "utmctl",
            "file",
            "push",
            request.target_uuid,
            request.guest_probe_incoming,
        )
        or not _successful_empty_observation(push)
        or normalize.argv
        != (
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
            request.guest_probe_incoming,
            request.guest_probe_path,
        )
        or not _successful_empty_observation(normalize)
    ):
        raise ValueError("prior-transaction-state-resolution-probe-delivery-invalid")
    _require_exact_payload(
        root / "guest-probe-readback.json",
        ("utmctl", "file", "pull", request.target_uuid, request.guest_probe_path),
        upstream.probe_bytes,
        "prior-transaction-state-resolution-probe-readback",
    )
    probe_sha256 = hashlib.sha256(upstream.probe_bytes).hexdigest()
    probe = network_ready._read_json(root / "guest-transaction-state-probe-once.json")
    observation = probe.get("observation")
    if (
        probe.get("format") != control.EVIDENCE_FORMAT
        or probe.get("raw_output_persisted") is not False
        or not isinstance(observation, dict)
        or observation.get("argv")
        != list(control.transaction_state_probe_argv(request, upstream))
        or not _successful_metadata(observation)
        or not runtime_bindings._empty_stream(observation.get("stdout"))
    ):
        raise ValueError("prior-transaction-state-resolution-probe-invalid")
    _require_exact_payload(
        root / "guest-marker-readback.json",
        (
            "utmctl",
            "file",
            "pull",
            request.target_uuid,
            request.guest_marker_path,
        ),
        control.guest_probe.marker_bytes(
            request.transaction_state_resolution_attempt_id, probe_sha256
        ),
        "prior-transaction-state-resolution-marker",
    )
    result = network_ready._read_json(root / "guest-result-readback-1.json")
    if (
        result.get("argv")
        != [
            "utmctl",
            "file",
            "pull",
            request.target_uuid,
            request.guest_result_path,
        ]
        or result.get("exit_code") != 0
        or result.get("timed_out") is not False
        or not runtime_bindings._empty_stream(result.get("stdout"))
        or not _stream_matches(
            result.get("stderr"),
            RESULT_MISSING_STDERR_SHA256,
            RESULT_MISSING_STDERR_SIZE,
        )
    ):
        raise ValueError("prior-transaction-state-resolution-missing-result-invalid")
    return probe_sha256


def _validate_terminal(
    request: TransactionStateResolutionResultBindingRequest,
    upstream: bindings.TransactionStateResolutionBinding,
    root: Path,
) -> None:
    expected = {
        "agent_readiness_invocations": 10,
        "agent_readiness_state": "ready",
        "automatic_cleanup": "not-performed",
        "automatic_quit": "not-performed",
        "automatic_retry": "not-performed",
        "automatic_stop": "not-performed",
        "backend_pid": 36343,
        "boot_classification": "not-generated",
        "boot_start_attempt_id": request.transaction_state_resolution_attempt_id,
        "business_guest_action": "not-performed",
        "file_pull_invocations": 3,
        "file_push_invocations": 1,
        "foreground_start_invocations": 1,
        "format": control.EVIDENCE_FORMAT,
        "guest_exec_invocations": 13,
        "guest_probe_invocations": 1,
        "identity_observation_count": 3,
        "inventory_probe_invocations": 1,
        "maintenance_resume_invocations": 0,
        "observed_boot_id_sha256": None,
        "operation_id": "not-read-or-generated",
        "outcome": "state-indeterminate",
        "plain_utmctl_list": "attempted-once-as-potential-backend-reactivation",
        "plain_utmctl_start": "not-performed",
        "plain_utmctl_status": "not-performed",
        "readiness_identity_observation_count": 10,
        "reason": "guest-result-readback-1:guest-result-readback-1-stderr-not-empty",
        "result_readback_invocations": 1,
        "runtime_poll_count": 1,
        "target_handles_terminal": "present",
        "target_name": request.target_name,
        "target_uuid": request.target_uuid,
        "terminal_relevant_host_process_count": 0,
        "transaction": "state-indeterminate",
    }
    if (
        network_ready._read_json(root / "terminal.json") != expected
        or upstream.evidence.get("prior_transaction") != "state-indeterminate"
    ):
        raise ValueError("prior-transaction-state-resolution-terminal-invalid")


def _validate_no_forbidden_commands(root: Path) -> None:
    forbidden = {
        "cleanup",
        "dpkg",
        "dpkg-query",
        "quit",
        "repair",
        "resume",
        "retry",
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
            if argv[:2] in (["utmctl", "status"], ["utmctl", "stop"]) or any(
                token in forbidden for token in argv
            ):
                raise ValueError(
                    "prior-transaction-state-resolution-forbidden-command"
                )


def _full_observation(value: dict[str, object]) -> start_control.CommandObservation:
    argv = value.get("argv")
    stdout = _full_stream(value.get("stdout"), "stdout")
    stderr = _full_stream(value.get("stderr"), "stderr")
    if (
        not isinstance(argv, list)
        or not all(isinstance(item, str) for item in argv)
        or value.get("exit_code") is not None
        and not isinstance(value.get("exit_code"), int)
        or not isinstance(value.get("timed_out"), bool)
    ):
        raise ValueError("prior-transaction-state-resolution-observation-invalid")
    return start_control.CommandObservation(
        argv=tuple(argv),
        exit_code=value.get("exit_code"),
        timed_out=bool(value["timed_out"]),
        stdout=stdout,
        stderr=stderr,
    )


def _full_stream(value: object, label: str) -> start_control.CapturedOutput:
    if not isinstance(value, dict) or not isinstance(
        value.get("prefix_base64"), str
    ):
        raise ValueError(f"prior-transaction-state-resolution-{label}-invalid")
    try:
        payload = base64.b64decode(value["prefix_base64"], validate=True)
    except ValueError as exc:
        raise ValueError(
            f"prior-transaction-state-resolution-{label}-base64-invalid"
        ) from exc
    if (
        value.get("truncated") is not False
        or value.get("total_bytes") != len(payload)
        or value.get("sha256") != hashlib.sha256(payload).hexdigest()
    ):
        raise ValueError(f"prior-transaction-state-resolution-{label}-drift")
    return start_control.CapturedOutput.from_bytes(payload)


def _require_exact_payload(
    path: Path, argv: tuple[str, ...], payload: bytes, label: str
) -> None:
    observation = _full_observation(network_ready._read_json(path))
    if (
        observation.argv != argv
        or observation.exit_code != 0
        or observation.timed_out
        or observation.stderr.total_bytes != 0
        or observation.stdout.prefix != payload
    ):
        raise ValueError(f"{label}-invalid")


def _successful_metadata(value: object) -> bool:
    return (
        isinstance(value, dict)
        and value.get("exit_code") == 0
        and value.get("timed_out") is False
        and runtime_bindings._empty_stream(value.get("stderr"))
    )


def _successful_empty_observation(
    value: start_control.CommandObservation,
) -> bool:
    return (
        value.exit_code == 0
        and not value.timed_out
        and value.stdout.total_bytes == 0
        and value.stderr.total_bytes == 0
    )


def _stream_matches(value: object, digest: str, size: int) -> bool:
    return (
        isinstance(value, dict)
        and value.get("sha256") == digest
        and value.get("total_bytes") == size
        and value.get("truncated") is False
    )
