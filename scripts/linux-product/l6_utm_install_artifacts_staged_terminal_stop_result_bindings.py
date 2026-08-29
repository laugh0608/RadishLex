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
import l6_utm_install_artifacts_staged_reactivation as reactivation_control
import l6_utm_install_artifacts_staged_runtime_resolution as runtime_control
import l6_utm_install_artifacts_staged_runtime_resolution_bindings as runtime_bindings
import l6_utm_install_artifacts_staged_terminal_stop as control
import l6_utm_install_artifacts_staged_transaction_state_resolution_result_bindings as prior_bindings
import l6_utm_launch_transport_v2 as launch_transport
import l6_utm_start_once as start_control


EVIDENCE_FORMAT = (
    "radishlex-linux-l6-utm-install-artifacts-staged-"
    "terminal-stop-result-binding-v1"
)
BINDINGS_RELATIVE_PATH = Path(
    "scripts/linux-product/"
    "l6_utm_install_artifacts_staged_terminal_stop_result_bindings.py"
)
REQUIRED_PRIOR_MANIFEST_SHA256 = (
    "096fe01f5f082a4f494ddf6219b14add91f178c37a6a12b8fb1df635a6d5e133"
)
REQUIRED_PRIOR_ATTEMPT_ID = control.REQUIRED_ATTEMPT_ID
REQUIRED_PRIOR_REPOSITORY_HEAD = "948577822d2ec3b7bbf692727a701edef9f0a2ad"
REQUIRED_BACKEND_PID = 36343
REQUIRED_HANDLE_SHA256 = (
    "6ad7484b0761082779937429826beb8a0620fddce0f97c1b2753807dc4814286"
)
REQUIRED_HANDLE_SIZE = 378
REQUIRED_EXCLUDED_GENERIC_QEMU = (
    {
        "accounting_name": "qemu-system-aarc",
        "parent_pid": 74571,
        "pid": 81382,
        "role": "qemu",
        "uid": 501,
    },
    {
        "accounting_name": "qemu-system-aarc",
        "parent_pid": 74571,
        "pid": 97570,
        "role": "qemu",
        "uid": 501,
    },
)
HEX_40 = re.compile(r"[0-9a-f]{40}")
EMPTY_SHA256 = hashlib.sha256(b"").hexdigest()
REQUIRED_ENTRY_NAMES = (
    "request.json",
    "binding-preflight.json",
    "source-bundle-preflight.json",
    "target-files-preflight.json",
    "host-process-preflight.json",
    "target-handles-preflight.json",
    "utmctl-list-preflight.json",
    "inventory-preflight-classification.json",
    "target-handle-pid-discovery.json",
    "target-handle-pid-confirmation-001.json",
    "host-process-confirmation-001.json",
    "target-handle-pid-confirmation-002.json",
    "host-process-confirmation-002.json",
    "target-handle-pid-confirmation-003.json",
    "host-process-confirmation-003.json",
    "target-files-ready.json",
    "utmctl-stop-request-once.json",
    "host-process-quiescence-001.json",
    "target-handles-quiescence-001.json",
    "host-process-quiescence-002.json",
    "target-handles-quiescence-002.json",
    "utmctl-list-terminal.json",
    "inventory-terminal-classification.json",
    "host-process-terminal.json",
    "target-handles-terminal.json",
    "target-files-postflight.json",
    "source-bundle-postflight.json",
    "terminal.json",
)


class TerminalStopResultBindingRequest(control.TerminalStopRequest, Protocol):
    prior_terminal_stop_root: Path
    prior_terminal_stop_manifest_sha256: str
    prior_terminal_stop_attempt_id: str


@dataclass(frozen=True)
class TerminalStopResultBinding:
    evidence: dict[str, object]
    upstream: control.TerminalStopBinding
    prior_repository_head: str
    backend_pid: int
    prior_handle_sha256: str
    prior_handle_size: int
    preflight_inventory: tuple[start_control.RegisteredVm, ...]
    terminal_inventory: tuple[start_control.RegisteredVm, ...]


def validate_terminal_stop_result_bindings(
    request: TerminalStopResultBindingRequest,
) -> TerminalStopResultBinding:
    if (
        request.prior_terminal_stop_attempt_id != REQUIRED_PRIOR_ATTEMPT_ID
        or request.prior_terminal_stop_manifest_sha256
        != REQUIRED_PRIOR_MANIFEST_SHA256
    ):
        raise ValueError("required-prior-terminal-stop-mismatch")

    upstream = control.validate_terminal_stop_bindings(request)
    binding_path = request.repository_root / BINDINGS_RELATIVE_PATH
    network_ready._require_committed_regular(
        binding_path, "terminal_stop_result_bindings"
    )

    root = request.prior_terminal_stop_root
    manifest = root / "files.sha256"
    if network_ready._sha256_file(manifest) != REQUIRED_PRIOR_MANIFEST_SHA256:
        raise ValueError("prior-terminal-stop-manifest-invalid")
    entries = start_control._verify_sha256_manifest(root, manifest)
    if (
        entries != len(REQUIRED_ENTRY_NAMES)
        or runtime_bindings._manifest_names(manifest) != REQUIRED_ENTRY_NAMES
    ):
        raise ValueError("prior-terminal-stop-entry-set-invalid")
    _require_private_evidence_tree(root, manifest)

    prior_repository_head = _validate_request_and_binding(
        request, upstream, root
    )
    _validate_source_target(request, root)
    excluded_processes = _validate_process_evidence(root)
    _validate_handle_evidence(request, root)
    preflight_inventory, terminal_inventory = _validate_inventory_transition(
        request, upstream, root
    )
    _validate_stop_observation(request, root)
    _validate_terminal(request, root)
    _validate_command_inventory(request, root)
    runtime_control._require_no_raw_operation_id(root)

    return TerminalStopResultBinding(
        evidence={
            "backend_pid": REQUIRED_BACKEND_PID,
            "excluded_generic_qemu_processes": list(excluded_processes),
            "format": EVIDENCE_FORMAT,
            "graceful_stop_invocations": 1,
            "outcome": "stopped-verified",
            "prior_handle_sha256": REQUIRED_HANDLE_SHA256,
            "prior_handle_size": REQUIRED_HANDLE_SIZE,
            "prior_terminal_stop_attempt_id": (
                request.prior_terminal_stop_attempt_id
            ),
            "prior_terminal_stop_entries_verified": entries,
            "prior_terminal_stop_manifest_sha256": (
                request.prior_terminal_stop_manifest_sha256
            ),
            "prior_terminal_stop_repository_head": prior_repository_head,
            "quiescence_poll_count": 2,
            "repository_clean": True,
            "repository_head": request.expected_repository_head,
            "target_registered_status_preflight": "started",
            "target_registered_status_terminal": "stopped",
            "target_uuid": request.target_uuid,
            "terminal_stop_result_bindings_sha256": (
                network_ready._sha256_file(binding_path)
            ),
            "terminal_stop_result_bindings_size": binding_path.stat().st_size,
            "transaction": "completed-frozen-before-stop",
        },
        upstream=upstream,
        prior_repository_head=prior_repository_head,
        backend_pid=REQUIRED_BACKEND_PID,
        prior_handle_sha256=REQUIRED_HANDLE_SHA256,
        prior_handle_size=REQUIRED_HANDLE_SIZE,
        preflight_inventory=preflight_inventory,
        terminal_inventory=terminal_inventory,
    )


def _require_private_evidence_tree(root: Path, manifest: Path) -> None:
    root_info = root.lstat()
    if (
        not stat.S_ISDIR(root_info.st_mode)
        or stat.S_ISLNK(root_info.st_mode)
        or stat.S_IMODE(root_info.st_mode) != 0o700
    ):
        raise ValueError("prior-terminal-stop-root-invalid")
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
            raise ValueError("prior-terminal-stop-file-identity-invalid")


def _validate_request_and_binding(
    request: TerminalStopResultBindingRequest,
    upstream: control.TerminalStopBinding,
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
        raise ValueError("prior-terminal-stop-head-invalid")
    expected_request = control.terminal_stop_request_json(request)
    expected_request["expected_repository_head"] = prior_repository_head
    if recorded_request != expected_request:
        raise ValueError("prior-terminal-stop-request-invalid")

    expected_binding = dict(upstream.evidence)
    expected_binding["repository_head"] = prior_repository_head
    if network_ready._read_json(root / "binding-preflight.json") != expected_binding:
        raise ValueError("prior-terminal-stop-binding-invalid")
    return prior_repository_head


def _validate_source_target(
    request: TerminalStopResultBindingRequest, root: Path
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
        raise ValueError("prior-terminal-stop-source-invalid")
    target_before = network_ready._read_json(root / "target-files-preflight.json")
    target_ready = network_ready._read_json(root / "target-files-ready.json")
    target_after = network_ready._read_json(root / "target-files-postflight.json")
    if (
        target_before != target_ready
        or target_before != target_after
        or target_before.get("format") != network_ready.EVIDENCE_FORMAT
        or target_before.get("target_package_name")
        != request.target_package_path.name
        or target_before.get("target_package_path_sha256")
        != network_ready._sha256_text(str(request.target_package_path))
    ):
        raise ValueError("prior-terminal-stop-target-invalid")


def _validate_process_evidence(
    root: Path,
) -> tuple[dict[str, object], ...]:
    names = (
        "host-process-preflight.json",
        "host-process-confirmation-001.json",
        "host-process-confirmation-002.json",
        "host-process-confirmation-003.json",
        "host-process-quiescence-001.json",
        "host-process-quiescence-002.json",
        "host-process-terminal.json",
    )
    excluded: tuple[dict[str, object], ...] | None = None
    for name in names:
        value = network_ready._read_json(root / name)
        observation = value.get("observation")
        current = value.get("excluded_generic_qemu_processes")
        if (
            value.get("format") != control.EVIDENCE_FORMAT
            or value.get("process_scope") != "utm-specific-accounting-v1"
            or value.get("relevant_process_count") != 0
            or value.get("relevant_processes") != []
            or value.get("excluded_generic_qemu_process_count") != 2
            or not isinstance(current, list)
            or tuple(current) != REQUIRED_EXCLUDED_GENERIC_QEMU
            or not isinstance(observation, dict)
            or observation.get("argv") != list(launch_transport.PROCESS_COMMAND)
            or not prior_bindings._successful_metadata(observation)
            or not runtime_bindings._complete_stream(observation.get("stdout"))
        ):
            raise ValueError("prior-terminal-stop-process-invalid")
        current_tuple = tuple(current)
        if excluded is None:
            excluded = current_tuple
        elif current_tuple != excluded:
            raise ValueError("prior-terminal-stop-process-drift")
    if excluded is None:
        raise ValueError("prior-terminal-stop-process-missing")
    return excluded


def _validate_handle_evidence(
    request: TerminalStopResultBindingRequest, root: Path
) -> None:
    untargeted = network_ready._lsof_argv(request)
    targeted = runtime_control.targeted_lsof_argv(request, REQUIRED_BACKEND_PID)
    present = (
        ("target-handles-preflight.json", control.EVIDENCE_FORMAT, untargeted, False),
        (
            "target-handle-pid-discovery.json",
            runtime_control.EVIDENCE_FORMAT,
            untargeted,
            True,
        ),
        (
            "target-handle-pid-confirmation-001.json",
            runtime_control.EVIDENCE_FORMAT,
            targeted,
            True,
        ),
        (
            "target-handle-pid-confirmation-002.json",
            runtime_control.EVIDENCE_FORMAT,
            targeted,
            True,
        ),
        (
            "target-handle-pid-confirmation-003.json",
            runtime_control.EVIDENCE_FORMAT,
            targeted,
            True,
        ),
        (
            "target-handles-quiescence-001.json",
            control.EVIDENCE_FORMAT,
            untargeted,
            False,
        ),
    )
    for name, evidence_format, argv, requires_pid in present:
        value = network_ready._read_json(root / name)
        observation = value.get("observation")
        stdout = observation.get("stdout") if isinstance(observation, dict) else None
        if (
            value.get("format") != evidence_format
            or value.get("state") != "present"
            or value.get("backend_command") != "QEMULauncher"
            or value.get("process_record_count") != 1
            or value.get("efi_handle_count") != 1
            or value.get("qcow2_handle_count") != 1
            or (requires_pid and value.get("backend_pid") != REQUIRED_BACKEND_PID)
            or (not requires_pid and "backend_pid" in value)
            or not isinstance(observation, dict)
            or observation.get("argv") != list(argv)
            or not prior_bindings._successful_metadata(observation)
            or not isinstance(stdout, dict)
            or stdout.get("sha256") != REQUIRED_HANDLE_SHA256
            or stdout.get("total_bytes") != REQUIRED_HANDLE_SIZE
            or stdout.get("truncated") is not False
        ):
            raise ValueError("prior-terminal-stop-handle-present-invalid")

    for name in (
        "target-handles-quiescence-002.json",
        "target-handles-terminal.json",
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
            or observation.get("argv") != list(untargeted)
            or not _empty_exit_one_observation(observation)
        ):
            raise ValueError("prior-terminal-stop-handle-absent-invalid")


def _validate_inventory_transition(
    request: TerminalStopResultBindingRequest,
    upstream: control.TerminalStopBinding,
    root: Path,
) -> tuple[
    tuple[start_control.RegisteredVm, ...],
    tuple[start_control.RegisteredVm, ...],
]:
    preflight = _read_full_observation(
        root / "utmctl-list-preflight.json", ("utmctl", "list")
    )
    terminal = _read_full_observation(
        root / "utmctl-list-terminal.json", ("utmctl", "list")
    )
    preflight_inventory = start_control.parse_utmctl_list(preflight)
    terminal_inventory = start_control.parse_utmctl_list(terminal)
    if (
        reactivation_control._require_inventory(
            preflight_inventory, upstream.baseline_inventory, request
        )
        != "started"
        or reactivation_control._require_inventory(
            terminal_inventory, upstream.baseline_inventory, request
        )
        != "stopped"
        or network_ready._read_json(
            root / "inventory-preflight-classification.json"
        )
        != control._inventory_evidence(request, preflight_inventory, "started")
        or network_ready._read_json(
            root / "inventory-terminal-classification.json"
        )
        != control._inventory_evidence(request, terminal_inventory, "stopped")
    ):
        raise ValueError("prior-terminal-stop-inventory-transition-invalid")
    return preflight_inventory, terminal_inventory


def _validate_stop_observation(
    request: TerminalStopResultBindingRequest, root: Path
) -> None:
    observation = _read_full_observation(
        root / "utmctl-stop-request-once.json",
        ("utmctl", "stop", request.target_uuid, "--request"),
    )
    if observation.stdout.prefix or observation.stderr.prefix:
        raise ValueError("prior-terminal-stop-command-output-invalid")


def _validate_terminal(
    request: TerminalStopResultBindingRequest, root: Path
) -> None:
    expected = {
        "automatic_cleanup": "not-performed",
        "automatic_force": "not-performed",
        "automatic_kill": "not-performed",
        "automatic_quit": "not-performed",
        "automatic_repair": "not-performed",
        "automatic_retry": "not-performed",
        "backend_pid_current": REQUIRED_BACKEND_PID,
        "backend_pid_prior": REQUIRED_BACKEND_PID,
        "business_guest_action": "not-performed",
        "current_handle_sha256": REQUIRED_HANDLE_SHA256,
        "current_handle_size": REQUIRED_HANDLE_SIZE,
        "file_pull_invocations": 0,
        "file_push_invocations": 0,
        "format": control.EVIDENCE_FORMAT,
        "graceful_stop_invocations": 1,
        "guest_exec_invocations": 0,
        "identity_observation_count": 3,
        "maintenance_resume_invocations": 0,
        "operation_id": "not-read-or-generated",
        "outcome": "stopped-verified",
        "plain_utmctl_list_invocations": 2,
        "plain_utmctl_start": "not-performed",
        "plain_utmctl_status": "not-performed",
        "preflight_inventory_invocations": 1,
        "quiescence_poll_count": 2,
        "reason": "single-graceful-request-and-all-stopped-cross-check",
        "registered_status_preflight": "started",
        "stop_command_error": None,
        "target_handles_preflight": "present",
        "target_handles_terminal": "absent",
        "target_name": request.target_name,
        "target_uuid": request.target_uuid,
        "terminal_excluded_generic_qemu_process_count": 2,
        "terminal_inventory_invocations": 1,
        "terminal_relevant_host_process_count": 0,
        "terminal_stop_attempt_id": request.prior_terminal_stop_attempt_id,
        "transaction": "completed-frozen-before-stop",
    }
    if network_ready._read_json(root / "terminal.json") != expected:
        raise ValueError("prior-terminal-stop-terminal-invalid")


def _validate_command_inventory(
    request: TerminalStopResultBindingRequest, root: Path
) -> None:
    untargeted = list(network_ready._lsof_argv(request))
    targeted = list(
        runtime_control.targeted_lsof_argv(request, REQUIRED_BACKEND_PID)
    )
    expected: dict[str, list[str]] = {
        "host-process-preflight.json": list(launch_transport.PROCESS_COMMAND),
        "target-handles-preflight.json": untargeted,
        "utmctl-list-preflight.json": ["utmctl", "list"],
        "target-handle-pid-discovery.json": untargeted,
        "utmctl-stop-request-once.json": [
            "utmctl",
            "stop",
            request.target_uuid,
            "--request",
        ],
        "host-process-quiescence-001.json": list(
            launch_transport.PROCESS_COMMAND
        ),
        "target-handles-quiescence-001.json": untargeted,
        "host-process-quiescence-002.json": list(
            launch_transport.PROCESS_COMMAND
        ),
        "target-handles-quiescence-002.json": untargeted,
        "utmctl-list-terminal.json": ["utmctl", "list"],
        "host-process-terminal.json": list(launch_transport.PROCESS_COMMAND),
        "target-handles-terminal.json": untargeted,
    }
    for index in range(1, 4):
        expected[f"target-handle-pid-confirmation-{index:03d}.json"] = targeted
        expected[f"host-process-confirmation-{index:03d}.json"] = list(
            launch_transport.PROCESS_COMMAND
        )
    for name in REQUIRED_ENTRY_NAMES:
        found = _all_argv(network_ready._read_json(root / name))
        wanted = [expected[name]] if name in expected else []
        if found != wanted:
            raise ValueError("prior-terminal-stop-command-inventory-invalid")


def _read_full_observation(
    path: Path, expected_argv: tuple[str, ...]
) -> start_control.CommandObservation:
    value = network_ready._read_json(path)
    if (
        value.get("argv") != list(expected_argv)
        or value.get("exit_code") != 0
        or value.get("timed_out") is not False
    ):
        raise ValueError("prior-terminal-stop-full-observation-invalid")
    stdout = _read_complete_stream(value.get("stdout"), "stdout")
    stderr = _read_complete_stream(value.get("stderr"), "stderr")
    if stderr:
        raise ValueError("prior-terminal-stop-full-observation-stderr")
    return start_control.CommandObservation.from_bytes(
        expected_argv, stdout=stdout, stderr=stderr
    )


def _read_complete_stream(value: object, label: str) -> bytes:
    if (
        not isinstance(value, dict)
        or value.get("truncated") is not False
        or not isinstance(value.get("prefix_base64"), str)
        or not isinstance(value.get("prefix_utf8"), str)
        or not isinstance(value.get("sha256"), str)
        or not isinstance(value.get("total_bytes"), int)
    ):
        raise ValueError(f"prior-terminal-stop-{label}-stream-invalid")
    try:
        payload = base64.b64decode(value["prefix_base64"], validate=True)
    except ValueError as exc:
        raise ValueError(
            f"prior-terminal-stop-{label}-stream-base64-invalid"
        ) from exc
    if (
        value.get("prefix_utf8") != payload.decode("utf-8", errors="replace")
        or value.get("total_bytes") != len(payload)
        or value.get("sha256") != hashlib.sha256(payload).hexdigest()
    ):
        raise ValueError(f"prior-terminal-stop-{label}-stream-drift")
    return payload


def _empty_exit_one_observation(value: dict[str, object]) -> bool:
    return (
        value.get("exit_code") == 1
        and value.get("timed_out") is False
        and _empty_metadata_stream(value.get("stdout"))
        and _empty_metadata_stream(value.get("stderr"))
    )


def _empty_metadata_stream(value: object) -> bool:
    return (
        isinstance(value, dict)
        and value.get("sha256") == EMPTY_SHA256
        and value.get("total_bytes") == 0
        and value.get("truncated") is False
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
