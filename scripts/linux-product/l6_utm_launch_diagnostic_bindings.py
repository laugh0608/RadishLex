#!/usr/bin/env python3
from __future__ import annotations

import hashlib
import json
import re
import stat
from pathlib import Path
from typing import Protocol

import l6_utm_start_once as start_control


PRIOR_EVIDENCE_FORMAT = "radishlex-linux-l6-utm-launch-diagnostics-v1"
EVIDENCE_FORMAT = "radishlex-linux-l6-utm-launch-diagnostics-v2"
CONTROL_RELATIVE_PATH = Path(
    "scripts/linux-product/l6_utm_launch_diagnostics.py"
)
BINDING_CONTROL_RELATIVE_PATH = Path(
    "scripts/linux-product/l6_utm_launch_diagnostic_bindings.py"
)
PRIOR_PROCESS_COMMAND = ("/bin/ps", "-axo", "pid=,ppid=,uid=,comm=")
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
    prior_diagnostic_root: Path
    prior_diagnostic_manifest_sha256: str
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
    prior_diagnostic = validate_prior_diagnostic_evidence(request)
    return {
        "binding_control_sha256": binding_control_sha256,
        "control_sha256": control_sha256,
        "format": EVIDENCE_FORMAT,
        **prior,
        **related,
        **prior_diagnostic,
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


def validate_prior_diagnostic_evidence(
    request: LaunchDiagnosticBindingRequest,
) -> dict[str, object]:
    entry_names = _verify_bound_manifest(
        request.prior_diagnostic_root,
        request.prior_diagnostic_manifest_sha256,
        "prior-diagnostic",
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
                "terminal.json",
            )
        ),
        "prior-diagnostic",
    )
    prior_request = _read_json_object(
        request.prior_diagnostic_root / "request.json",
        "prior-diagnostic-request",
    )
    _require_json_fields(
        prior_request,
        {
            "authorization": {
                "host_launch_diagnostics": True,
                "read_system_log": True,
            },
            "expected_vm_count": request.expected_vm_count,
            "format": PRIOR_EVIDENCE_FORMAT,
            "log_end": request.log_end,
            "log_start": request.log_start,
            "prior_failure_manifest_sha256": (
                request.prior_failure_manifest_sha256
            ),
            "prior_postverify_manifest_sha256": (
                request.prior_postverify_manifest_sha256
            ),
            "prior_start_manifest_sha256": (
                request.prior_start_manifest_sha256
            ),
            "target_name": request.target_name,
            "target_uuid": request.target_uuid,
        },
        "prior-diagnostic-request",
    )
    prior_repository_head = prior_request.get("expected_repository_head")
    if (
        not isinstance(prior_repository_head, str)
        or not HEX_40.fullmatch(prior_repository_head)
    ):
        raise BindingError("prior-diagnostic-request-repository-head")
    prior_binding = _read_json_object(
        request.prior_diagnostic_root / "binding-preflight.json",
        "prior-diagnostic-binding",
    )
    _require_json_fields(
        prior_binding,
        {
            "format": PRIOR_EVIDENCE_FORMAT,
            "prior_failure_manifest_sha256": (
                request.prior_failure_manifest_sha256
            ),
            "prior_postverify_manifest_sha256": (
                request.prior_postverify_manifest_sha256
            ),
            "prior_start_manifest_sha256": (
                request.prior_start_manifest_sha256
            ),
            "repository_clean": True,
            "repository_head": prior_repository_head,
        },
        "prior-diagnostic-binding",
    )
    prior_control_sha256 = prior_binding.get("control_sha256")
    if (
        not isinstance(prior_control_sha256, str)
        or not HEX_64.fullmatch(prior_control_sha256)
    ):
        raise BindingError("prior-diagnostic-binding-control-sha256")
    host_command = _read_json_object(
        request.prior_diagnostic_root / "host-process-command.json",
        "prior-diagnostic-host-process",
    )
    _require_json_fields(
        host_command,
        {
            "argv": list(PRIOR_PROCESS_COMMAND),
            "exit_code": 0,
            "timed_out": False,
        },
        "prior-diagnostic-host-process",
    )
    stdout = host_command.get("stdout")
    stderr = host_command.get("stderr")
    if not isinstance(stdout, dict) or not isinstance(stderr, dict):
        raise BindingError("prior-diagnostic-host-process-streams")
    stdout_size = stdout.get("total_bytes")
    stdout_hash = stdout.get("sha256")
    if (
        stdout.get("truncated") is not True
        or not isinstance(stdout_size, int)
        or isinstance(stdout_size, bool)
        or stdout_size <= start_control.MAX_CAPTURE_BYTES
        or not isinstance(stdout_hash, str)
        or not HEX_64.fullmatch(stdout_hash)
    ):
        raise BindingError("prior-diagnostic-host-process-stdout")
    _require_json_fields(
        stderr,
        {
            "prefix_utf8": "",
            "sha256": hashlib.sha256(b"").hexdigest(),
            "total_bytes": 0,
            "truncated": False,
        },
        "prior-diagnostic-host-process-stderr",
    )
    terminal = _read_json_object(
        request.prior_diagnostic_root / "terminal.json",
        "prior-diagnostic-terminal",
    )
    expected_terminal = {
        "automatic_delete": "not-performed",
        "automatic_retry": "not-performed",
        "format": PRIOR_EVIDENCE_FORMAT,
        "guest_exec": "not-performed",
        "host_process_observation": "attempted",
        "input_transfer": "not-performed",
        "operation_id": "not-generated",
        "outcome": "diagnostics-incomplete",
        "reason": (
            "host-process-observation:"
            "host-process-observation-output-truncated"
        ),
        "root_cause": "unattributed",
        "target_name": request.target_name,
        "target_uuid": request.target_uuid,
        "transaction": "not-performed",
        "unified_log_observation": "not-performed",
        "utm_clone": "not-performed",
        "utm_start": "not-performed",
        "utm_stop": "not-performed",
    }
    _require_json_fields(
        terminal, expected_terminal, "prior-diagnostic-terminal"
    )
    return {
        "prior_diagnostic_entries_verified": len(entry_names),
        "prior_diagnostic_manifest_sha256": (
            request.prior_diagnostic_manifest_sha256
        ),
        "prior_diagnostic_outcome": terminal["outcome"],
        "prior_diagnostic_reason": terminal["reason"],
    }


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
