#!/usr/bin/env python3
from __future__ import annotations

import hashlib
import json
import re
import stat
from pathlib import Path
from typing import Protocol

import l6_utm_start_once as start_control


INITIAL_EVIDENCE_FORMAT = "radishlex-linux-l6-utm-launch-diagnostics-v1"
PRIOR_EVIDENCE_FORMAT = "radishlex-linux-l6-utm-launch-diagnostics-v2"
LATEST_EVIDENCE_FORMAT = "radishlex-linux-l6-utm-launch-diagnostics-v3"
PRIOR_LOG_EVIDENCE_FORMAT = "radishlex-linux-l6-utm-launch-diagnostics-v4"
PRIOR_SCHEMA_EVIDENCE_FORMAT = "radishlex-linux-l6-utm-launch-diagnostics-v5"
EVIDENCE_FORMAT = "radishlex-linux-l6-utm-launch-diagnostics-v6"
CONTROL_RELATIVE_PATH = Path(
    "scripts/linux-product/l6_utm_launch_diagnostics.py"
)
BINDING_CONTROL_RELATIVE_PATH = Path(
    "scripts/linux-product/l6_utm_launch_diagnostic_bindings.py"
)
INITIAL_PROCESS_COMMAND = ("/bin/ps", "-axo", "pid=,ppid=,uid=,comm=")
PRIOR_PROCESS_COMMAND = ("/bin/ps", "-axo", "pid=,ppid=,uid=,ucomm=")
LATEST_PROCESS_COMMAND = PRIOR_PROCESS_COMMAND
LOG_PREDICATE = (
    '(process == "UTM") OR (process == "utmctl") OR '
    '(process BEGINSWITH "qemu")'
)
LOG_CAPTURE_BYTES = 8 * 1024 * 1024
HEX_40 = re.compile(r"[0-9a-f]{40}")
HEX_64 = re.compile(r"[0-9a-f]{64}")


class BindingError(ValueError):
    pass


class LaunchDiagnosticBindingRequest(Protocol):
    repository_root: Path
    expected_repository_head: str
    prior_start_root: Path
    prior_start_manifest_sha256: str
    prior_failure_root: Path
    prior_failure_manifest_sha256: str
    prior_postverify_root: Path
    prior_postverify_manifest_sha256: str
    initial_diagnostic_root: Path
    initial_diagnostic_manifest_sha256: str
    prior_diagnostic_root: Path
    prior_diagnostic_manifest_sha256: str
    latest_diagnostic_root: Path
    latest_diagnostic_manifest_sha256: str
    prior_log_diagnostic_root: Path
    prior_log_diagnostic_manifest_sha256: str
    prior_schema_diagnostic_root: Path
    prior_schema_diagnostic_manifest_sha256: str
    target_uuid: str
    target_name: str
    expected_vm_count: int
    log_start: str
    log_end: str


def validate_diagnostic_bindings(
    request: LaunchDiagnosticBindingRequest,
) -> dict[str, object]:
    repository_head = _run_git(
        request.repository_root, ("rev-parse", "HEAD")
    ).decode("ascii").strip()
    if repository_head != request.expected_repository_head:
        raise BindingError("repository-head-drift")
    if _run_git(request.repository_root, ("status", "--porcelain")):
        raise BindingError("repository-not-clean")
    control_sha256 = validate_control_identity(request.repository_root)
    binding_control_sha256 = validate_binding_control_identity(
        request.repository_root
    )
    prior = validate_prior_start_evidence(request)
    related = validate_related_evidence(request)
    initial_diagnostic = validate_initial_diagnostic_evidence(request)
    prior_diagnostic = validate_prior_diagnostic_evidence(request)
    latest_diagnostic = validate_latest_diagnostic_evidence(request)
    prior_log_diagnostic = validate_prior_log_diagnostic_evidence(request)
    prior_schema_diagnostic = validate_prior_schema_diagnostic_evidence(request)
    return {
        "binding_control_sha256": binding_control_sha256,
        "control_sha256": control_sha256,
        "format": EVIDENCE_FORMAT,
        **prior,
        **related,
        **initial_diagnostic,
        **prior_diagnostic,
        **latest_diagnostic,
        **prior_log_diagnostic,
        **prior_schema_diagnostic,
        "repository_clean": True,
        "repository_head": repository_head,
    }


def validate_prior_start_evidence(
    request: LaunchDiagnosticBindingRequest,
) -> dict[str, object]:
    entry_names = _verify_bound_manifest(
        request.prior_start_root,
        request.prior_start_manifest_sha256,
        "prior-start",
    )
    _require_manifest_entries(
        entry_names,
        frozenset(("request.json", "terminal.json")),
        "prior-start",
    )

    request_value = _read_json_object(
        request.prior_start_root / "request.json", "prior-start-request"
    )
    terminal_value = _read_json_object(
        request.prior_start_root / "terminal.json", "prior-start-terminal"
    )
    for value, label in (
        (request_value, "prior-start-request"),
        (terminal_value, "prior-start-terminal"),
    ):
        if value.get("format") != start_control.EVIDENCE_FORMAT:
            raise BindingError(f"{label}-format")
        if value.get("clone_uuid") != request.target_uuid:
            raise BindingError(f"{label}-target-uuid")
        if value.get("clone_name") != request.target_name:
            raise BindingError(f"{label}-target-name")

    expected_terminal = {
        "automatic_retry": "not-performed",
        "automatic_stop": "not-performed",
        "guest_exec": "not-performed",
        "input_transfer": "not-performed",
        "operation_id": "not-generated",
        "outcome": "failed-closed-stopped",
        "reason": "started-not-observed-and-all-vms-stopped",
        "start_invocations": 1,
        "transaction": "not-performed",
    }
    for field, expected in expected_terminal.items():
        if terminal_value.get(field) != expected:
            raise BindingError(f"prior-start-terminal-field:{field}")
    status_poll_count = terminal_value.get("status_poll_count")
    if (
        not isinstance(status_poll_count, int)
        or isinstance(status_poll_count, bool)
        or not 1 <= status_poll_count <= 300
    ):
        raise BindingError("prior-start-terminal-status-poll-count")
    if not isinstance(terminal_value.get("start_command_timed_out"), bool):
        raise BindingError("prior-start-terminal-timeout")
    start_exit = terminal_value.get("start_command_exit_code")
    if start_exit is not None and (
        not isinstance(start_exit, int) or isinstance(start_exit, bool)
    ):
        raise BindingError("prior-start-terminal-exit-code")
    return {
        "prior_start_entries_verified": len(entry_names),
        "prior_start_manifest_sha256": request.prior_start_manifest_sha256,
        "prior_start_outcome": terminal_value["outcome"],
        "prior_start_status_poll_count": status_poll_count,
    }


def validate_related_evidence(
    request: LaunchDiagnosticBindingRequest,
) -> dict[str, object]:
    failure_entry_names = _verify_bound_manifest(
        request.prior_failure_root,
        request.prior_failure_manifest_sha256,
        "prior-failure",
    )
    _require_manifest_entries(
        failure_entry_names,
        frozenset(("failure.evidence.txt",)),
        "prior-failure",
    )
    postverify_entry_names = _verify_bound_manifest(
        request.prior_postverify_root,
        request.prior_postverify_manifest_sha256,
        "prior-postverify",
    )
    _require_manifest_entries(
        postverify_entry_names,
        frozenset(("postverify.evidence.txt",)),
        "prior-postverify",
    )
    failure_lines = _read_key_value_evidence(
        request.prior_failure_root / "failure.evidence.txt",
        "prior-failure-summary",
    )
    _require_evidence_fields(
        failure_lines,
        {
            "target_uuid": request.target_uuid,
            "failure_stage": "start-controller",
            "controller_invocations": "1",
            "guest_exec_invocations": "0",
            "file_pull_invocations": "0",
            "automatic_stop": "not-performed",
            "automatic_retry": "not-performed",
            "automatic_delete": "not-performed",
            "input_transfer": "not-performed",
            "operation_id": "not-generated",
        },
        "prior-failure-summary",
    )
    postverify_lines = _read_key_value_evidence(
        request.prior_postverify_root / "postverify.evidence.txt",
        "prior-postverify-summary",
    )
    _require_evidence_fields(
        postverify_lines,
        {
            "start_manifest_sha256": request.prior_start_manifest_sha256,
            "failure_manifest_sha256": request.prior_failure_manifest_sha256,
            "target_uuid": request.target_uuid,
            "registered_vms": "all-stopped",
            "target_vm": "stopped",
            "terminal": "failed-closed-stopped",
            "guest_exec": "not-performed",
            "file_pull_invocations": "0",
            "input_transfer": "not-performed",
            "operation_id": "not-generated",
            "transaction": "not-performed",
            "postverify": "failed-closed-preserved",
        },
        "prior-postverify-summary",
    )
    return {
        "prior_failure_entries_verified": len(failure_entry_names),
        "prior_failure_manifest_sha256": request.prior_failure_manifest_sha256,
        "prior_postverify_entries_verified": len(postverify_entry_names),
        "prior_postverify_manifest_sha256": (
            request.prior_postverify_manifest_sha256
        ),
    }


def validate_initial_diagnostic_evidence(
    request: LaunchDiagnosticBindingRequest,
) -> dict[str, object]:
    return _validate_incomplete_diagnostic_evidence(
        request,
        root=request.initial_diagnostic_root,
        manifest_sha256=request.initial_diagnostic_manifest_sha256,
        label="initial-diagnostic",
        evidence_format=INITIAL_EVIDENCE_FORMAT,
        process_command=INITIAL_PROCESS_COMMAND,
        expected_reason=(
            "host-process-observation:"
            "host-process-observation-output-truncated"
        ),
        result_prefix="initial_diagnostic",
        stdout_must_be_truncated=True,
        request_diagnostic_fields={},
        binding_diagnostic_fields={},
    )


def validate_prior_diagnostic_evidence(
    request: LaunchDiagnosticBindingRequest,
) -> dict[str, object]:
    return _validate_incomplete_diagnostic_evidence(
        request,
        root=request.prior_diagnostic_root,
        manifest_sha256=request.prior_diagnostic_manifest_sha256,
        label="prior-diagnostic",
        evidence_format=PRIOR_EVIDENCE_FORMAT,
        process_command=PRIOR_PROCESS_COMMAND,
        expected_reason=(
            "host-process-observation:host-process-identifier-invalid"
        ),
        result_prefix="prior_diagnostic",
        stdout_must_be_truncated=False,
        request_diagnostic_fields={
            "prior_diagnostic_manifest_sha256": (
                request.initial_diagnostic_manifest_sha256
            ),
        },
        binding_diagnostic_fields={
            "prior_diagnostic_entries_verified": 6,
            "prior_diagnostic_manifest_sha256": (
                request.initial_diagnostic_manifest_sha256
            ),
            "prior_diagnostic_outcome": "diagnostics-incomplete",
            "prior_diagnostic_reason": (
                "host-process-observation:"
                "host-process-observation-output-truncated"
            ),
        },
    )


def validate_latest_diagnostic_evidence(
    request: LaunchDiagnosticBindingRequest,
) -> dict[str, object]:
    return _validate_incomplete_diagnostic_evidence(
        request,
        root=request.latest_diagnostic_root,
        manifest_sha256=request.latest_diagnostic_manifest_sha256,
        label="latest-diagnostic",
        evidence_format=LATEST_EVIDENCE_FORMAT,
        process_command=LATEST_PROCESS_COMMAND,
        expected_reason=(
            "host-process-observation:host-process-identifier-invalid"
        ),
        result_prefix="latest_diagnostic",
        stdout_must_be_truncated=False,
        request_diagnostic_fields={
            "initial_diagnostic_manifest_sha256": (
                request.initial_diagnostic_manifest_sha256
            ),
            "prior_diagnostic_manifest_sha256": (
                request.prior_diagnostic_manifest_sha256
            ),
        },
        binding_diagnostic_fields={
            "initial_diagnostic_entries_verified": 6,
            "initial_diagnostic_manifest_sha256": (
                request.initial_diagnostic_manifest_sha256
            ),
            "initial_diagnostic_outcome": "diagnostics-incomplete",
            "initial_diagnostic_reason": (
                "host-process-observation:"
                "host-process-observation-output-truncated"
            ),
            "prior_diagnostic_entries_verified": 6,
            "prior_diagnostic_manifest_sha256": (
                request.prior_diagnostic_manifest_sha256
            ),
            "prior_diagnostic_outcome": "diagnostics-incomplete",
            "prior_diagnostic_reason": (
                "host-process-observation:host-process-identifier-invalid"
            ),
        },
    )


def validate_prior_log_diagnostic_evidence(
    request: LaunchDiagnosticBindingRequest,
) -> dict[str, object]:
    return _validate_post_process_diagnostic_evidence(
        request,
        root=request.prior_log_diagnostic_root,
        manifest_sha256=request.prior_log_diagnostic_manifest_sha256,
        label="prior-log-diagnostic",
        evidence_format=PRIOR_LOG_EVIDENCE_FORMAT,
        result_prefix="prior_log_diagnostic",
        expected_reason=(
            "unified-log-observation:"
            "unified-log-observation-output-truncated"
        ),
        log_stdout_must_be_truncated=True,
        log_capture_bytes=start_control.MAX_CAPTURE_BYTES,
        request_diagnostic_fields={},
        binding_diagnostic_fields={},
    )


def validate_prior_schema_diagnostic_evidence(
    request: LaunchDiagnosticBindingRequest,
) -> dict[str, object]:
    return _validate_post_process_diagnostic_evidence(
        request,
        root=request.prior_schema_diagnostic_root,
        manifest_sha256=request.prior_schema_diagnostic_manifest_sha256,
        label="prior-schema-diagnostic",
        evidence_format=PRIOR_SCHEMA_EVIDENCE_FORMAT,
        result_prefix="prior_schema_diagnostic",
        expected_reason=(
            "unified-log-observation:unified-log-category-invalid"
        ),
        log_stdout_must_be_truncated=False,
        log_capture_bytes=LOG_CAPTURE_BYTES,
        request_diagnostic_fields={
            "prior_log_diagnostic_manifest_sha256": (
                request.prior_log_diagnostic_manifest_sha256
            ),
        },
        binding_diagnostic_fields={
            "prior_log_diagnostic_entries_verified": 8,
            "prior_log_diagnostic_manifest_sha256": (
                request.prior_log_diagnostic_manifest_sha256
            ),
            "prior_log_diagnostic_outcome": "diagnostics-incomplete",
            "prior_log_diagnostic_reason": (
                "unified-log-observation:"
                "unified-log-observation-output-truncated"
            ),
        },
    )


def _validate_post_process_diagnostic_evidence(
    request: LaunchDiagnosticBindingRequest,
    *,
    root: Path,
    manifest_sha256: str,
    label: str,
    evidence_format: str,
    result_prefix: str,
    expected_reason: str,
    log_stdout_must_be_truncated: bool,
    log_capture_bytes: int,
    request_diagnostic_fields: dict[str, object],
    binding_diagnostic_fields: dict[str, object],
) -> dict[str, object]:
    entry_names = _verify_bound_manifest(
        root,
        manifest_sha256,
        label,
    )
    _require_manifest_entries(
        entry_names,
        frozenset(
            (
                "request.json",
                "binding-preflight.json",
                "utmctl-list-live.json",
                "utmctl-status-live.json",
                "host-process-command.json",
                "host-processes.json",
                "unified-log-command.json",
                "terminal.json",
            )
        ),
        label,
    )
    prior_request = _read_json_object(
        root / "request.json",
        f"{label}-request",
    )
    request_fields: dict[str, object] = {
        "authorization": {
            "host_launch_diagnostics": True,
            "read_system_log": True,
        },
        "expected_vm_count": request.expected_vm_count,
        "format": evidence_format,
        "initial_diagnostic_manifest_sha256": (
            request.initial_diagnostic_manifest_sha256
        ),
        "latest_diagnostic_manifest_sha256": (
            request.latest_diagnostic_manifest_sha256
        ),
        "log_end": request.log_end,
        "log_start": request.log_start,
        "prior_diagnostic_manifest_sha256": (
            request.prior_diagnostic_manifest_sha256
        ),
        "prior_failure_manifest_sha256": (
            request.prior_failure_manifest_sha256
        ),
        "prior_postverify_manifest_sha256": (
            request.prior_postverify_manifest_sha256
        ),
        "prior_start_manifest_sha256": request.prior_start_manifest_sha256,
        "target_name": request.target_name,
        "target_uuid": request.target_uuid,
    }
    request_fields.update(request_diagnostic_fields)
    _require_json_fields(
        prior_request, request_fields, f"{label}-request"
    )
    prior_repository_head = prior_request.get("expected_repository_head")
    if (
        not isinstance(prior_repository_head, str)
        or not HEX_40.fullmatch(prior_repository_head)
    ):
        raise BindingError(f"{label}-request-repository-head")

    prior_binding = _read_json_object(
        root / "binding-preflight.json",
        f"{label}-binding",
    )
    binding_fields: dict[str, object] = {
        "format": evidence_format,
        "initial_diagnostic_entries_verified": 6,
        "initial_diagnostic_manifest_sha256": (
            request.initial_diagnostic_manifest_sha256
        ),
        "initial_diagnostic_outcome": "diagnostics-incomplete",
        "latest_diagnostic_entries_verified": 6,
        "latest_diagnostic_manifest_sha256": (
            request.latest_diagnostic_manifest_sha256
        ),
        "latest_diagnostic_outcome": "diagnostics-incomplete",
        "prior_diagnostic_entries_verified": 6,
        "prior_diagnostic_manifest_sha256": (
            request.prior_diagnostic_manifest_sha256
        ),
        "prior_diagnostic_outcome": "diagnostics-incomplete",
        "prior_failure_manifest_sha256": (
            request.prior_failure_manifest_sha256
        ),
        "prior_postverify_manifest_sha256": (
            request.prior_postverify_manifest_sha256
        ),
        "prior_start_manifest_sha256": request.prior_start_manifest_sha256,
        "repository_clean": True,
        "repository_head": prior_repository_head,
    }
    binding_fields.update(binding_diagnostic_fields)
    _require_json_fields(
        prior_binding, binding_fields, f"{label}-binding"
    )
    for field in ("binding_control_sha256", "control_sha256"):
        digest = prior_binding.get(field)
        if not isinstance(digest, str) or not HEX_64.fullmatch(digest):
            raise BindingError(f"{label}-binding-{field.replace('_', '-')}")

    host_command = _read_json_object(
        root / "host-process-command.json",
        f"{label}-host-process",
    )
    _require_json_fields(
        host_command,
        {
            "argv": list(LATEST_PROCESS_COMMAND),
            "exit_code": 0,
            "timed_out": False,
        },
        f"{label}-host-process",
    )
    _validate_successful_command_streams(
        host_command,
        label=f"{label}-host-process",
        stdout_must_be_truncated=False,
        max_capture_bytes=start_control.MAX_CAPTURE_BYTES,
    )
    host_processes = _read_json_object(
        root / "host-processes.json",
        f"{label}-host-processes",
    )
    _require_json_fields(
        host_processes,
        {
            "format": evidence_format,
            "relevant_process_count": 0,
            "relevant_processes": [],
        },
        f"{label}-host-processes",
    )

    log_command = _read_json_object(
        root / "unified-log-command.json",
        f"{label}-unified-log",
    )
    _require_json_fields(
        log_command,
        {
            "argv": list(_log_command(request)),
            "exit_code": 0,
            "timed_out": False,
        },
        f"{label}-unified-log",
    )
    _validate_successful_command_streams(
        log_command,
        label=f"{label}-unified-log",
        stdout_must_be_truncated=log_stdout_must_be_truncated,
        max_capture_bytes=log_capture_bytes,
    )

    terminal = _read_json_object(
        root / "terminal.json",
        f"{label}-terminal",
    )
    _require_json_fields(
        terminal,
        {
            "automatic_delete": "not-performed",
            "automatic_retry": "not-performed",
            "format": evidence_format,
            "guest_exec": "not-performed",
            "host_process_observation": "performed",
            "input_transfer": "not-performed",
            "operation_id": "not-generated",
            "outcome": "diagnostics-incomplete",
            "reason": expected_reason,
            "root_cause": "unattributed",
            "target_name": request.target_name,
            "target_uuid": request.target_uuid,
            "transaction": "not-performed",
            "unified_log_observation": "attempted",
            "utm_clone": "not-performed",
            "utm_start": "not-performed",
            "utm_stop": "not-performed",
        },
        f"{label}-terminal",
    )
    return {
        f"{result_prefix}_entries_verified": len(entry_names),
        f"{result_prefix}_manifest_sha256": manifest_sha256,
        f"{result_prefix}_outcome": terminal["outcome"],
        f"{result_prefix}_reason": terminal["reason"],
    }


def _validate_incomplete_diagnostic_evidence(
    request: LaunchDiagnosticBindingRequest,
    *,
    root: Path,
    manifest_sha256: str,
    label: str,
    evidence_format: str,
    process_command: tuple[str, ...],
    expected_reason: str,
    result_prefix: str,
    stdout_must_be_truncated: bool,
    request_diagnostic_fields: dict[str, object],
    binding_diagnostic_fields: dict[str, object],
) -> dict[str, object]:
    entry_names = _verify_bound_manifest(root, manifest_sha256, label)
    _require_manifest_entries(
        entry_names,
        frozenset(
            (
                "request.json",
                "binding-preflight.json",
                "utmctl-list-live.json",
                "utmctl-status-live.json",
                "host-process-command.json",
                "terminal.json",
            )
        ),
        label,
    )
    prior_request = _read_json_object(
        root / "request.json", f"{label}-request"
    )
    request_fields: dict[str, object] = {
        "authorization": {
            "host_launch_diagnostics": True,
            "read_system_log": True,
        },
        "expected_vm_count": request.expected_vm_count,
        "format": evidence_format,
        "log_end": request.log_end,
        "log_start": request.log_start,
        "prior_failure_manifest_sha256": (
            request.prior_failure_manifest_sha256
        ),
        "prior_postverify_manifest_sha256": (
            request.prior_postverify_manifest_sha256
        ),
        "prior_start_manifest_sha256": request.prior_start_manifest_sha256,
        "target_name": request.target_name,
        "target_uuid": request.target_uuid,
    }
    request_fields.update(request_diagnostic_fields)
    _require_json_fields(prior_request, request_fields, f"{label}-request")
    prior_repository_head = prior_request.get("expected_repository_head")
    if (
        not isinstance(prior_repository_head, str)
        or not HEX_40.fullmatch(prior_repository_head)
    ):
        raise BindingError(f"{label}-request-repository-head")

    prior_binding = _read_json_object(
        root / "binding-preflight.json", f"{label}-binding"
    )
    binding_fields: dict[str, object] = {
        "format": evidence_format,
        "prior_failure_manifest_sha256": (
            request.prior_failure_manifest_sha256
        ),
        "prior_postverify_manifest_sha256": (
            request.prior_postverify_manifest_sha256
        ),
        "prior_start_manifest_sha256": request.prior_start_manifest_sha256,
        "repository_clean": True,
        "repository_head": prior_repository_head,
    }
    binding_fields.update(binding_diagnostic_fields)
    _require_json_fields(prior_binding, binding_fields, f"{label}-binding")
    control_fields = ["control_sha256"]
    if evidence_format in (PRIOR_EVIDENCE_FORMAT, LATEST_EVIDENCE_FORMAT):
        control_fields.append("binding_control_sha256")
    for field in control_fields:
        digest = prior_binding.get(field)
        if not isinstance(digest, str) or not HEX_64.fullmatch(digest):
            raise BindingError(f"{label}-binding-{field.replace('_', '-')}")

    host_command = _read_json_object(
        root / "host-process-command.json", f"{label}-host-process"
    )
    _require_json_fields(
        host_command,
        {
            "argv": list(process_command),
            "exit_code": 0,
            "timed_out": False,
        },
        f"{label}-host-process",
    )
    stdout = host_command.get("stdout")
    stderr = host_command.get("stderr")
    if not isinstance(stdout, dict) or not isinstance(stderr, dict):
        raise BindingError(f"{label}-host-process-streams")
    _validate_process_stdout(
        stdout,
        label,
        must_be_truncated=stdout_must_be_truncated,
    )
    _require_json_fields(
        stderr,
        {
            "prefix_utf8": "",
            "sha256": hashlib.sha256(b"").hexdigest(),
            "total_bytes": 0,
            "truncated": False,
        },
        f"{label}-host-process-stderr",
    )

    terminal = _read_json_object(root / "terminal.json", f"{label}-terminal")
    _require_json_fields(
        terminal,
        {
            "automatic_delete": "not-performed",
            "automatic_retry": "not-performed",
            "format": evidence_format,
            "guest_exec": "not-performed",
            "host_process_observation": "attempted",
            "input_transfer": "not-performed",
            "operation_id": "not-generated",
            "outcome": "diagnostics-incomplete",
            "reason": expected_reason,
            "root_cause": "unattributed",
            "target_name": request.target_name,
            "target_uuid": request.target_uuid,
            "transaction": "not-performed",
            "unified_log_observation": "not-performed",
            "utm_clone": "not-performed",
            "utm_start": "not-performed",
            "utm_stop": "not-performed",
        },
        f"{label}-terminal",
    )
    return {
        f"{result_prefix}_entries_verified": len(entry_names),
        f"{result_prefix}_manifest_sha256": manifest_sha256,
        f"{result_prefix}_outcome": terminal["outcome"],
        f"{result_prefix}_reason": terminal["reason"],
    }


def _validate_process_stdout(
    stdout: dict[str, object],
    label: str,
    *,
    must_be_truncated: bool,
    max_capture_bytes: int = start_control.MAX_CAPTURE_BYTES,
) -> None:
    stdout_size = stdout.get("total_bytes")
    stdout_hash = stdout.get("sha256")
    if (
        stdout.get("truncated") is not must_be_truncated
        or not isinstance(stdout_size, int)
        or isinstance(stdout_size, bool)
        or stdout_size <= 0
        or (
            must_be_truncated
            and stdout_size <= max_capture_bytes
        )
        or (
            not must_be_truncated
            and stdout_size > max_capture_bytes
        )
        or not isinstance(stdout_hash, str)
        or not HEX_64.fullmatch(stdout_hash)
    ):
        raise BindingError(f"{label}-host-process-stdout")


def _validate_successful_command_streams(
    command: dict[str, object],
    *,
    label: str,
    stdout_must_be_truncated: bool,
    max_capture_bytes: int,
) -> None:
    stdout = command.get("stdout")
    stderr = command.get("stderr")
    if not isinstance(stdout, dict) or not isinstance(stderr, dict):
        raise BindingError(f"{label}-streams")
    _validate_process_stdout(
        stdout,
        label,
        must_be_truncated=stdout_must_be_truncated,
        max_capture_bytes=max_capture_bytes,
    )
    _require_json_fields(
        stderr,
        {
            "prefix_utf8": "",
            "sha256": hashlib.sha256(b"").hexdigest(),
            "total_bytes": 0,
            "truncated": False,
        },
        f"{label}-stderr",
    )


def _log_command(
    request: LaunchDiagnosticBindingRequest,
) -> tuple[str, ...]:
    return (
        "/usr/bin/log",
        "show",
        "--style",
        "ndjson",
        "--no-pager",
        "--timezone",
        "UTC",
        "--info",
        "--no-debug",
        "--no-signpost",
        "--no-loss",
        "--start",
        request.log_start,
        "--end",
        request.log_end,
        "--predicate",
        LOG_PREDICATE,
    )


def validate_control_identity(repository_root: Path) -> str:
    expected_path = repository_root / CONTROL_RELATIVE_PATH
    invoked_path = Path(__file__).with_name(CONTROL_RELATIVE_PATH.name).absolute()
    return _validate_control_file(expected_path, invoked_path)


def validate_binding_control_identity(repository_root: Path) -> str:
    expected_path = repository_root / BINDING_CONTROL_RELATIVE_PATH
    invoked_path = Path(__file__).absolute()
    return _validate_control_file(expected_path, invoked_path)


def _validate_control_file(expected_path: Path, invoked_path: Path) -> str:
    if invoked_path != expected_path:
        raise BindingError("executed-control-path-mismatch")
    try:
        control_stat = expected_path.lstat()
    except OSError as exc:
        raise BindingError("executed-control-unavailable") from exc
    if (
        not stat.S_ISREG(control_stat.st_mode)
        or stat.S_ISLNK(control_stat.st_mode)
        or control_stat.st_nlink != 1
        or stat.S_IMODE(control_stat.st_mode) & 0o022
    ):
        raise BindingError("executed-control-identity-invalid")
    try:
        return start_control._sha256_file(expected_path)
    except start_control.StartControlError as exc:
        raise BindingError(str(exc)) from exc


def _verify_bound_manifest(
    root: Path, expected_manifest_sha256: str, label: str
) -> frozenset[str]:
    manifest_path = root / "files.sha256"
    try:
        if start_control._sha256_file(manifest_path) != expected_manifest_sha256:
            raise BindingError(f"{label}-manifest-drift")
        verified_entries = start_control._verify_sha256_manifest(
            root, manifest_path
        )
    except start_control.StartControlError as exc:
        raise BindingError(f"{label}:{exc}") from exc
    try:
        entry_names = frozenset(
            line[66:]
            for line in manifest_path.read_text(encoding="ascii").splitlines()
        )
    except (OSError, UnicodeDecodeError) as exc:
        raise BindingError(f"{label}-manifest-unreadable") from exc
    if len(entry_names) != verified_entries:
        raise BindingError(f"{label}-manifest-entry-count")
    return entry_names


def _require_manifest_entries(
    actual: frozenset[str], required: frozenset[str], label: str
) -> None:
    missing = sorted(required - actual)
    if missing:
        raise BindingError(f"{label}-manifest-required-entry:{missing[0]}")


def _read_key_value_evidence(path: Path, label: str) -> dict[str, str]:
    try:
        payload = path.read_bytes()
    except OSError as exc:
        raise BindingError(f"{label}-unavailable") from exc
    if not payload or len(payload) > 64 * 1024 or not payload.endswith(b"\n"):
        raise BindingError(f"{label}-size-or-newline")
    try:
        lines = payload.decode("utf-8").splitlines()
    except UnicodeDecodeError as exc:
        raise BindingError(f"{label}-not-utf8") from exc
    if len(lines) > 128:
        raise BindingError(f"{label}-line-budget")
    values: dict[str, str] = {}
    for line in lines:
        if line.count("=") != 1:
            raise BindingError(f"{label}-line-invalid")
        key, value = line.split("=", maxsplit=1)
        if (
            not re.fullmatch(r"[a-z][a-z0-9_]*", key)
            or not value
            or any(character in "\x00\r\n" for character in value)
            or key in values
        ):
            raise BindingError(f"{label}-field-invalid")
        values[key] = value
    return values


def _require_evidence_fields(
    actual: dict[str, str],
    expected: dict[str, str],
    label: str,
) -> None:
    for key, value in expected.items():
        if actual.get(key) != value:
            raise BindingError(f"{label}-field:{key}")


def _require_json_fields(
    actual: dict[str, object],
    expected: dict[str, object],
    label: str,
) -> None:
    for key, value in expected.items():
        if actual.get(key) != value:
            raise BindingError(f"{label}-field:{key}")


def _run_git(repository_root: Path, arguments: tuple[str, ...]) -> bytes:
    try:
        return start_control._run_git(repository_root, arguments)
    except start_control.StartControlError as exc:
        raise BindingError(str(exc)) from exc


def _read_json_object(path: Path, label: str) -> dict[str, object]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise BindingError(f"{label}-invalid") from exc
    if not isinstance(value, dict):
        raise BindingError(f"{label}-not-object")
    return value
