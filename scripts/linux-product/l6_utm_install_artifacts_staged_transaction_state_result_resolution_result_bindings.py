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
import l6_utm_install_artifacts_staged_runtime_resolution as runtime_control
import l6_utm_install_artifacts_staged_runtime_resolution_bindings as runtime_bindings
import l6_utm_install_artifacts_staged_transaction_state_resolution_result_bindings as prior_bindings
import l6_utm_install_artifacts_staged_transaction_state_result_resolution as control
import l6_utm_launch_transport_v2 as launch_transport
import l6_utm_start_once as start_control


EVIDENCE_FORMAT = (
    "radishlex-linux-l6-utm-install-artifacts-staged-"
    "transaction-state-result-resolution-result-binding-v1"
)
BINDINGS_RELATIVE_PATH = Path(
    "scripts/linux-product/"
    "l6_utm_install_artifacts_staged_transaction_state_result_resolution_"
    "result_bindings.py"
)
REQUIRED_PRIOR_MANIFEST_SHA256 = (
    "7fcef38e05b7fff0d2c0fcb661bd4957343848cd06b8f5222f9dec9ef4540e3d"
)
REQUIRED_PRIOR_ATTEMPT_ID = control.REQUIRED_ATTEMPT_ID
REQUIRED_PRIOR_REPOSITORY_HEAD = "f66e32ee26819a21facb1b12506bd5b83e17098e"
REQUIRED_BACKEND_PID = 36343
REQUIRED_HANDLE_SHA256 = (
    "6ad7484b0761082779937429826beb8a0620fddce0f97c1b2753807dc4814286"
)
REQUIRED_HANDLE_SIZE = 378
REQUIRED_GUEST_RESULT_SHA256 = (
    "a40e107942f7a1af2cba616c4264eadf7060b60b9eeac30b4bec3272a3530eec"
)
REQUIRED_GUEST_RESULT_SIZE = 1_259
REQUIRED_PHASE_SHA256 = (
    "996209798e71819f6bf8a51f7164b96a64e1cb77e6112d4a05704988b83af3a4"
)
REQUIRED_PHASE_SIZE = 113
REQUIRED_BOOT_ID_SHA256 = (
    "26beda7387f2b66b4db08331c738aab9a5bb499094c65b339a8ea2033eb270e6"
)
REQUIRED_RECEIPT_SHA256 = (
    "fec8b23ad4b12e24a822d3cfc2936cdd7462aea50fc2fba680b38140bc42f73d"
)
REQUIRED_RECEIPT_SIZE = 3_786
REQUIRED_DPKG_LOG_SHA256 = (
    "c08bd0586b161efa5654b7204db988c8d1134746c8872936df596df91e9cb056"
)
REQUIRED_DPKG_LOG_SIZE = 881_152
REQUIRED_DPKG_STATUS_SHA256 = (
    "33c4973d4bcccc1932de35b2b326c61140037ee46613c7a925f2cbdbd3d5cff1"
)
REQUIRED_EXCLUDED_GENERIC_QEMU = (
    {
        "accounting_name": "qemu-system-aarc",
        "parent_pid": 74571,
        "pid": 97570,
        "role": "qemu",
        "uid": 501,
    },
)
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
    "existing-transaction-state-result-readback-1.json",
    "existing-transaction-state-result-readback-2.json",
    "existing-transaction-state-result.json",
    "existing-transaction-state-phase-readback.json",
    "target-handle-pid-terminal.json",
    "host-process-terminal.json",
    "target-files-postflight.json",
    "source-bundle-postflight.json",
    "terminal.json",
)


class TransactionStateResultResolutionResultBindingRequest(
    prior_bindings.TransactionStateResolutionResultBindingRequest,
    Protocol,
):
    prior_transaction_state_result_resolution_root: Path
    prior_transaction_state_result_resolution_manifest_sha256: str
    prior_transaction_state_result_resolution_attempt_id: str


@dataclass(frozen=True)
class TransactionStateResultResolutionResultBinding:
    evidence: dict[str, object]
    upstream: control.TransactionStateResultResolutionBinding
    prior_repository_head: str
    prior_backend_pid: int
    prior_handle_sha256: str
    prior_handle_size: int
    guest_result_sha256: str
    guest_result_size: int
    boot_id_sha256: str
    receipt_sha256: str
    receipt_size: int


def validate_transaction_state_result_resolution_result_bindings(
    request: TransactionStateResultResolutionResultBindingRequest,
) -> TransactionStateResultResolutionResultBinding:
    if (
        request.prior_transaction_state_result_resolution_attempt_id
        != REQUIRED_PRIOR_ATTEMPT_ID
        or request.prior_transaction_state_result_resolution_manifest_sha256
        != REQUIRED_PRIOR_MANIFEST_SHA256
    ):
        raise ValueError(
            "required-prior-transaction-state-result-resolution-mismatch"
        )

    upstream = control.validate_transaction_state_result_resolution_bindings(
        request
    )
    binding_path = request.repository_root / BINDINGS_RELATIVE_PATH
    network_ready._require_committed_regular(
        binding_path, "transaction_state_result_resolution_result_bindings"
    )

    root = request.prior_transaction_state_result_resolution_root
    manifest = root / "files.sha256"
    if network_ready._sha256_file(manifest) != REQUIRED_PRIOR_MANIFEST_SHA256:
        raise ValueError(
            "prior-transaction-state-result-resolution-manifest-invalid"
        )
    entries = start_control._verify_sha256_manifest(root, manifest)
    if (
        entries != len(REQUIRED_ENTRY_NAMES)
        or runtime_bindings._manifest_names(manifest) != REQUIRED_ENTRY_NAMES
    ):
        raise ValueError(
            "prior-transaction-state-result-resolution-entry-set-invalid"
        )
    _require_private_evidence_tree(root, manifest)

    prior_repository_head = _validate_request_and_binding(
        request, upstream, root
    )
    _validate_source_target(request, root)
    excluded_processes = _validate_process_evidence(root)
    handle_signature = _validate_handle_evidence(request, upstream, root)
    guest_result = _validate_result_and_phase(request, upstream, root)
    _validate_terminal(request, root)
    _validate_command_inventory(request, upstream, root)
    runtime_control._require_no_raw_operation_id(root)

    return TransactionStateResultResolutionResultBinding(
        evidence={
            "boot_id_sha256": REQUIRED_BOOT_ID_SHA256,
            "dpkg_mutation_executed": "not-performed",
            "excluded_generic_qemu_processes": list(excluded_processes),
            "format": EVIDENCE_FORMAT,
            "guest_result_sha256": REQUIRED_GUEST_RESULT_SHA256,
            "guest_result_size": REQUIRED_GUEST_RESULT_SIZE,
            "package_profile": "installed-verified",
            "prior_transaction_state_result_resolution_attempt_id": (
                request.prior_transaction_state_result_resolution_attempt_id
            ),
            "prior_transaction_state_result_resolution_backend_pid": (
                upstream.prior_backend_pid
            ),
            "prior_transaction_state_result_resolution_entries_verified": (
                entries
            ),
            "prior_transaction_state_result_resolution_handle_sha256": (
                handle_signature[0]
            ),
            "prior_transaction_state_result_resolution_handle_size": (
                handle_signature[1]
            ),
            "prior_transaction_state_result_resolution_manifest_sha256": (
                request.prior_transaction_state_result_resolution_manifest_sha256
            ),
            "prior_transaction_state_result_resolution_repository_head": (
                prior_repository_head
            ),
            "receipt_sha256": REQUIRED_RECEIPT_SHA256,
            "receipt_size": REQUIRED_RECEIPT_SIZE,
            "repository_clean": True,
            "repository_head": request.expected_repository_head,
            "result_authority": "stable-double-result-plus-completed-phase",
            "startup_profile": "allowed",
            "target_uuid": request.target_uuid,
            "transaction": guest_result.transaction,
            "transaction_state_result_resolution_result_bindings_sha256": (
                network_ready._sha256_file(binding_path)
            ),
            "transaction_state_result_resolution_result_bindings_size": (
                binding_path.stat().st_size
            ),
        },
        upstream=upstream,
        prior_repository_head=prior_repository_head,
        prior_backend_pid=upstream.prior_backend_pid,
        prior_handle_sha256=handle_signature[0],
        prior_handle_size=handle_signature[1],
        guest_result_sha256=REQUIRED_GUEST_RESULT_SHA256,
        guest_result_size=REQUIRED_GUEST_RESULT_SIZE,
        boot_id_sha256=REQUIRED_BOOT_ID_SHA256,
        receipt_sha256=REQUIRED_RECEIPT_SHA256,
        receipt_size=REQUIRED_RECEIPT_SIZE,
    )


def _require_private_evidence_tree(root: Path, manifest: Path) -> None:
    root_info = root.lstat()
    if (
        not stat.S_ISDIR(root_info.st_mode)
        or stat.S_ISLNK(root_info.st_mode)
        or stat.S_IMODE(root_info.st_mode) != 0o700
    ):
        raise ValueError(
            "prior-transaction-state-result-resolution-root-invalid"
        )
    for name in (*runtime_bindings._manifest_names(manifest), "files.sha256"):
        path = root / name
        info = path.lstat()
        if (
            not stat.S_ISREG(info.st_mode)
            or stat.S_ISLNK(info.st_mode)
            or stat.S_IMODE(info.st_mode) != 0o600
            or info.st_nlink != 1
            or info.st_uid != root_info.st_uid
            or info.st_gid != root_info.st_gid
        ):
            raise ValueError(
                "prior-transaction-state-result-resolution-file-identity-invalid"
            )


def _validate_request_and_binding(
    request: TransactionStateResultResolutionResultBindingRequest,
    upstream: control.TransactionStateResultResolutionBinding,
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
        raise ValueError(
            "prior-transaction-state-result-resolution-head-invalid"
        )
    expected_request = control.TransactionStateResultResolutionRequest.as_json(
        request
    )
    expected_request["expected_repository_head"] = prior_repository_head
    if recorded_request != expected_request:
        raise ValueError(
            "prior-transaction-state-result-resolution-request-invalid"
        )

    expected_binding = dict(upstream.evidence)
    expected_binding["repository_head"] = prior_repository_head
    if network_ready._read_json(root / "binding-preflight.json") != expected_binding:
        raise ValueError(
            "prior-transaction-state-result-resolution-binding-invalid"
        )
    return prior_repository_head


def _validate_source_target(
    request: TransactionStateResultResolutionResultBindingRequest,
    root: Path,
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
        raise ValueError(
            "prior-transaction-state-result-resolution-source-invalid"
        )
    target_before = network_ready._read_json(root / "target-files-preflight.json")
    target_after = network_ready._read_json(root / "target-files-postflight.json")
    if (
        target_before != target_after
        or target_before.get("format") != network_ready.EVIDENCE_FORMAT
        or target_before.get("target_package_name")
        != request.target_package_path.name
        or target_before.get("target_package_path_sha256")
        != network_ready._sha256_text(str(request.target_package_path))
    ):
        raise ValueError(
            "prior-transaction-state-result-resolution-target-invalid"
        )


def _validate_process_evidence(root: Path) -> tuple[dict[str, object], ...]:
    excluded: tuple[dict[str, object], ...] | None = None
    for name in (
        "host-process-preflight.json",
        "host-process-confirmation-001.json",
        "host-process-confirmation-002.json",
        "host-process-confirmation-003.json",
        "host-process-terminal.json",
    ):
        value = network_ready._read_json(root / name)
        observation = value.get("observation")
        current = value.get("excluded_generic_qemu_processes")
        if (
            value.get("format") != runtime_control.EVIDENCE_FORMAT
            or value.get("process_scope") != "utm-specific-accounting-v1"
            or value.get("relevant_process_count") != 0
            or value.get("relevant_processes") != []
            or value.get("excluded_generic_qemu_process_count") != 1
            or not isinstance(current, list)
            or tuple(current) != REQUIRED_EXCLUDED_GENERIC_QEMU
            or not isinstance(observation, dict)
            or observation.get("argv") != list(launch_transport.PROCESS_COMMAND)
            or not prior_bindings._successful_metadata(observation)
            or not runtime_bindings._complete_stream(observation.get("stdout"))
        ):
            raise ValueError(
                "prior-transaction-state-result-resolution-process-invalid"
            )
        current_tuple = tuple(current)
        if excluded is None:
            excluded = current_tuple
        elif current_tuple != excluded:
            raise ValueError(
                "prior-transaction-state-result-resolution-process-drift"
            )
    if excluded is None:
        raise ValueError(
            "prior-transaction-state-result-resolution-process-missing"
        )
    return excluded


def _validate_handle_evidence(
    request: TransactionStateResultResolutionResultBindingRequest,
    upstream: control.TransactionStateResultResolutionBinding,
    root: Path,
) -> tuple[str, int]:
    names = (
        "target-handle-pid-discovery.json",
        "target-handle-pid-confirmation-001.json",
        "target-handle-pid-confirmation-002.json",
        "target-handle-pid-confirmation-003.json",
        "target-handle-pid-terminal.json",
    )
    signature: tuple[str, int] | None = None
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
            value.get("format") != runtime_control.EVIDENCE_FORMAT
            or value.get("state") != "present"
            or value.get("backend_pid") != REQUIRED_BACKEND_PID
            or value.get("backend_pid") != upstream.prior_backend_pid
            or value.get("backend_command") != "QEMULauncher"
            or value.get("process_record_count") != 1
            or value.get("efi_handle_count") != 1
            or value.get("qcow2_handle_count") != 1
            or not isinstance(observation, dict)
            or observation.get("argv") != list(expected_argv)
            or not prior_bindings._successful_metadata(observation)
            or not runtime_bindings._complete_stream(stdout)
        ):
            raise ValueError(
                "prior-transaction-state-result-resolution-handle-invalid"
            )
        assert isinstance(stdout, dict)
        current = (str(stdout["sha256"]), int(stdout["total_bytes"]))
        if signature is None:
            signature = current
        elif current != signature:
            raise ValueError(
                "prior-transaction-state-result-resolution-handle-drift"
            )
    if (
        signature != (REQUIRED_HANDLE_SHA256, REQUIRED_HANDLE_SIZE)
        or signature
        != (upstream.prior_handle_sha256, upstream.prior_handle_size)
    ):
        raise ValueError(
            "prior-transaction-state-result-resolution-handle-signature-invalid"
        )
    return signature


def _validate_result_and_phase(
    request: TransactionStateResultResolutionResultBindingRequest,
    upstream: control.TransactionStateResultResolutionBinding,
    root: Path,
) -> control.boot_control.GuestProbeResolution:
    result_argv = (
        "utmctl",
        "file",
        "pull",
        request.target_uuid,
        request.guest_result_path,
    )
    payloads = [
        _read_payload(
            root / f"existing-transaction-state-result-readback-{index}.json",
            result_argv,
            f"prior-transaction-state-result-{index}",
        )
        for index in (1, 2)
    ]
    if (
        payloads[0] != payloads[1]
        or len(payloads[0]) != REQUIRED_GUEST_RESULT_SIZE
        or hashlib.sha256(payloads[0]).hexdigest()
        != REQUIRED_GUEST_RESULT_SHA256
    ):
        raise ValueError(
            "prior-transaction-state-result-resolution-result-drift"
        )
    guest_result = control.prior_control.parse_transaction_state_result(
        payloads[0], request, upstream.probe_sha256
    )
    expected_evidence = {
        "boot_id_sha256": REQUIRED_BOOT_ID_SHA256,
        "dpkg_log_sha256": REQUIRED_DPKG_LOG_SHA256,
        "dpkg_log_size": REQUIRED_DPKG_LOG_SIZE,
        "dpkg_status_sha256": REQUIRED_DPKG_STATUS_SHA256,
        "format": control.prior_control.EVIDENCE_FORMAT,
        "guest_result_sha256": REQUIRED_GUEST_RESULT_SHA256,
        "guest_result_size": REQUIRED_GUEST_RESULT_SIZE,
        "operation_id": "hash-only",
        "operation_id_sha256": (
            control.prior_control.guest_probe.EXPECTED_OPERATION_ID_SHA256
        ),
        "package_profile": "installed-verified",
        "receipt_sha256": REQUIRED_RECEIPT_SHA256,
        "receipt_size": REQUIRED_RECEIPT_SIZE,
        "transaction": "completed",
    }
    if (
        guest_result.outcome != "transaction-completed"
        or guest_result.exit_code != control.EXIT_TRANSACTION_COMPLETED
        or guest_result.transaction != "completed"
        or guest_result.evidence != expected_evidence
    ):
        raise ValueError(
            "prior-transaction-state-result-resolution-result-semantics-invalid"
        )
    expected_recorded = {
        "evidence": expected_evidence,
        "format": control.EVIDENCE_FORMAT,
        "outcome": "transaction-completed",
        "reason": guest_result.reason,
        "transaction": "completed",
    }
    if (
        network_ready._read_json(root / "existing-transaction-state-result.json")
        != expected_recorded
    ):
        raise ValueError(
            "prior-transaction-state-result-resolution-recorded-result-invalid"
        )

    phase = control.prior_control.guest_probe.canonical_json(
        {
            "format": control.prior_control.guest_probe.PHASE_FORMAT,
            "phase": "completed",
        }
    )
    if (
        len(phase) != REQUIRED_PHASE_SIZE
        or hashlib.sha256(phase).hexdigest() != REQUIRED_PHASE_SHA256
    ):
        raise ValueError(
            "prior-transaction-state-result-resolution-phase-constant-invalid"
        )
    _require_exact_payload(
        root / "existing-transaction-state-phase-readback.json",
        (
            "utmctl",
            "file",
            "pull",
            request.target_uuid,
            request.guest_transaction_phase_path,
        ),
        phase,
        "prior-transaction-state-result-resolution-phase",
    )
    return guest_result


def _validate_terminal(
    request: TransactionStateResultResolutionResultBindingRequest,
    root: Path,
) -> None:
    expected = {
        "automatic_cleanup": "not-performed",
        "automatic_quit": "not-performed",
        "automatic_retry": "not-performed",
        "automatic_stop": "not-performed",
        "backend_pid": REQUIRED_BACKEND_PID,
        "business_guest_action": "existing-result-readback-only",
        "file_pull_invocations": 3,
        "file_push_invocations": 0,
        "format": control.EVIDENCE_FORMAT,
        "guest_exec_invocations": 0,
        "guest_probe_invocations": 0,
        "identity_observation_count": 3,
        "inventory_probe_invocations": 0,
        "maintenance_resume_invocations": 0,
        "operation_id": "hash-only",
        "outcome": "transaction-completed",
        "phase_readback_invocations": 1,
        "plain_utmctl_list": "not-performed",
        "plain_utmctl_start": "not-performed",
        "plain_utmctl_status": "not-performed",
        "prior_transaction_state_resolution_attempt_id": (
            request.prior_transaction_state_resolution_attempt_id
        ),
        "reason": "deferred-readback-transaction-completed",
        "result_readback_invocations": 2,
        "target_name": request.target_name,
        "target_uuid": request.target_uuid,
        "terminal_excluded_generic_qemu_process_count": 1,
        "terminal_relevant_host_process_count": 0,
        "transaction": "completed",
        "transaction_state_result_resolution_attempt_id": (
            request.transaction_state_result_resolution_attempt_id
        ),
    }
    if network_ready._read_json(root / "terminal.json") != expected:
        raise ValueError(
            "prior-transaction-state-result-resolution-terminal-invalid"
        )


def _validate_command_inventory(
    request: TransactionStateResultResolutionResultBindingRequest,
    upstream: control.TransactionStateResultResolutionBinding,
    root: Path,
) -> None:
    expected: dict[str, list[str]] = {
        "host-process-preflight.json": list(launch_transport.PROCESS_COMMAND),
        "target-handle-pid-discovery.json": list(
            network_ready._lsof_argv(request)
        ),
        "existing-transaction-state-phase-readback.json": [
            "utmctl",
            "file",
            "pull",
            request.target_uuid,
            request.guest_transaction_phase_path,
        ],
    }
    for index in range(1, 4):
        expected[f"target-handle-pid-confirmation-{index:03d}.json"] = list(
            runtime_control.targeted_lsof_argv(
                request, upstream.prior_backend_pid
            )
        )
        expected[f"host-process-confirmation-{index:03d}.json"] = list(
            launch_transport.PROCESS_COMMAND
        )
    expected["target-handle-pid-terminal.json"] = list(
        runtime_control.targeted_lsof_argv(request, upstream.prior_backend_pid)
    )
    expected["host-process-terminal.json"] = list(
        launch_transport.PROCESS_COMMAND
    )
    for index in (1, 2):
        expected[f"existing-transaction-state-result-readback-{index}.json"] = [
            "utmctl",
            "file",
            "pull",
            request.target_uuid,
            request.guest_result_path,
        ]

    for name in REQUIRED_ENTRY_NAMES:
        found = _all_argv(network_ready._read_json(root / name))
        wanted = [expected[name]] if name in expected else []
        if found != wanted:
            raise ValueError(
                "prior-transaction-state-result-resolution-command-inventory-invalid"
            )


def _all_argv(value: object) -> list[list[str]]:
    found: list[list[str]] = []
    if isinstance(value, dict):
        argv = value.get("argv")
        if isinstance(argv, list) and all(isinstance(item, str) for item in argv):
            found.append(argv)
        for key, child in value.items():
            if key != "argv":
                found.extend(_all_argv(child))
    elif isinstance(value, list):
        for child in value:
            found.extend(_all_argv(child))
    return found


def _read_payload(path: Path, argv: tuple[str, ...], label: str) -> bytes:
    value = network_ready._read_json(path)
    stdout = value.get("stdout")
    if (
        value.get("argv") != list(argv)
        or not prior_bindings._successful_metadata(value)
        or not isinstance(stdout, dict)
        or stdout.get("truncated") is not False
        or not isinstance(stdout.get("prefix_base64"), str)
    ):
        raise ValueError(f"{label}-observation-invalid")
    try:
        payload = base64.b64decode(stdout["prefix_base64"], validate=True)
    except ValueError as exc:
        raise ValueError(f"{label}-base64-invalid") from exc
    if (
        stdout.get("total_bytes") != len(payload)
        or stdout.get("sha256") != hashlib.sha256(payload).hexdigest()
    ):
        raise ValueError(f"{label}-stream-invalid")
    return payload


def _require_exact_payload(
    path: Path, argv: tuple[str, ...], expected: bytes, label: str
) -> None:
    if _read_payload(path, argv, label) != expected:
        raise ValueError(f"{label}-payload-invalid")
