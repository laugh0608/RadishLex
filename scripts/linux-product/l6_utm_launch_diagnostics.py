#!/usr/bin/env python3
from __future__ import annotations

import argparse
import hashlib
import json
import re
import stat
import sys
import uuid
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Callable

import l6_utm_start_once as start_control


EVIDENCE_FORMAT = "radishlex-linux-l6-utm-launch-diagnostics-v1"
CONTROL_RELATIVE_PATH = Path(
    "scripts/linux-product/l6_utm_launch_diagnostics.py"
)
PROCESS_COMMAND = ("/bin/ps", "-axo", "pid=,ppid=,uid=,comm=")
LOG_PREDICATE = (
    '(process == "UTM") OR (process == "utmctl") OR '
    '(process BEGINSWITH "qemu")'
)
EXIT_COLLECTED = 0
EXIT_DIAGNOSTICS_INCOMPLETE = 10
EXIT_PRECONDITION_REJECTED = 11
HEX_40 = re.compile(r"[0-9a-f]{40}")
HEX_64 = re.compile(r"[0-9a-f]{64}")
SAFE_ATTEMPT_ID = re.compile(r"[a-z0-9][a-z0-9-]{0,63}")
UTC_TIMESTAMP = re.compile(
    r"\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}\+0000"
)
HOME_PATH = re.compile(r"/(?:Users|home)/[^/\s\"']+")
MAX_LOG_WINDOW_SECONDS = 15 * 60
MAX_LOG_EVENTS = 512
MAX_LOG_LINE_BYTES = 16 * 1024
MAX_LOG_MESSAGE_BYTES = 32 * 1024
MAX_REDACTED_MESSAGE_CHARS = 1024

CommandObservation = start_control.CommandObservation
CommandRunner = start_control.CommandRunner
SubprocessCommandRunner = start_control.SubprocessCommandRunner


class LaunchDiagnosticError(ValueError):
    pass


@dataclass(frozen=True)
class LaunchDiagnosticRequest:
    repository_root: Path
    expected_repository_head: str
    prior_start_root: Path
    prior_start_manifest_sha256: str
    prior_failure_root: Path
    prior_failure_manifest_sha256: str
    prior_postverify_root: Path
    prior_postverify_manifest_sha256: str
    output_root: Path
    attempt_id: str
    target_uuid: str
    target_name: str
    expected_vm_count: int
    log_start: str
    log_end: str
    command_timeout_seconds: int
    log_timeout_seconds: int
    authorized_host_launch_diagnostics: bool
    authorized_read_system_log: bool

    def validate(self) -> None:
        for path, label in (
            (self.repository_root, "repository-root"),
            (self.prior_start_root, "prior-start-root"),
            (self.prior_failure_root, "prior-failure-root"),
            (self.prior_postverify_root, "prior-postverify-root"),
            (self.output_root, "output-root"),
        ):
            if not path.is_absolute():
                raise LaunchDiagnosticError(f"{label}-must-be-absolute")
            if ".." in path.parts:
                raise LaunchDiagnosticError(f"{label}-must-be-normalized")
        if _path_is_within(self.output_root, self.repository_root):
            raise LaunchDiagnosticError("output-root-must-be-outside-repository")
        for root, label in (
            (self.prior_start_root, "prior-start"),
            (self.prior_failure_root, "prior-failure"),
            (self.prior_postverify_root, "prior-postverify"),
        ):
            if _path_is_within(self.output_root, root):
                raise LaunchDiagnosticError(
                    f"output-root-must-not-modify-{label}"
                )
        if not HEX_40.fullmatch(self.expected_repository_head):
            raise LaunchDiagnosticError("expected-repository-head-invalid")
        if not HEX_64.fullmatch(self.prior_start_manifest_sha256):
            raise LaunchDiagnosticError("prior-start-manifest-sha256-invalid")
        if not HEX_64.fullmatch(self.prior_failure_manifest_sha256):
            raise LaunchDiagnosticError("prior-failure-manifest-sha256-invalid")
        if not HEX_64.fullmatch(self.prior_postverify_manifest_sha256):
            raise LaunchDiagnosticError(
                "prior-postverify-manifest-sha256-invalid"
            )
        if not SAFE_ATTEMPT_ID.fullmatch(self.attempt_id):
            raise LaunchDiagnosticError("attempt-id-invalid")
        try:
            parsed_uuid = uuid.UUID(self.target_uuid)
        except ValueError as exc:
            raise LaunchDiagnosticError("target-uuid-invalid") from exc
        if str(parsed_uuid).upper() != self.target_uuid:
            raise LaunchDiagnosticError("target-uuid-must-be-uppercase-canonical")
        if (
            not self.target_name
            or len(self.target_name.encode("utf-8")) > 160
            or any(character in "\x00\r\n" for character in self.target_name)
        ):
            raise LaunchDiagnosticError("target-name-invalid")
        if not 1 <= self.expected_vm_count <= 128:
            raise LaunchDiagnosticError("expected-vm-count-out-of-range")
        log_start = _parse_utc_timestamp(self.log_start, "log-start")
        log_end = _parse_utc_timestamp(self.log_end, "log-end")
        window_seconds = (log_end - log_start).total_seconds()
        if not 0 < window_seconds <= MAX_LOG_WINDOW_SECONDS:
            raise LaunchDiagnosticError("log-window-out-of-range")
        if not 1 <= self.command_timeout_seconds <= 60:
            raise LaunchDiagnosticError("command-timeout-seconds-out-of-range")
        if not 1 <= self.log_timeout_seconds <= 120:
            raise LaunchDiagnosticError("log-timeout-seconds-out-of-range")
        if not self.authorized_host_launch_diagnostics:
            raise LaunchDiagnosticError(
                "authorized-host-launch-diagnostics-required"
            )
        if not self.authorized_read_system_log:
            raise LaunchDiagnosticError("authorized-read-system-log-required")

    def as_json(self) -> dict[str, object]:
        return {
            "attempt_id": self.attempt_id,
            "authorization": {
                "host_launch_diagnostics": True,
                "read_system_log": True,
            },
            "command_timeout_seconds": self.command_timeout_seconds,
            "expected_repository_head": self.expected_repository_head,
            "expected_vm_count": self.expected_vm_count,
            "format": EVIDENCE_FORMAT,
            "log_end": self.log_end,
            "log_start": self.log_start,
            "log_timeout_seconds": self.log_timeout_seconds,
            "prior_failure_manifest_sha256": (
                self.prior_failure_manifest_sha256
            ),
            "prior_postverify_manifest_sha256": (
                self.prior_postverify_manifest_sha256
            ),
            "prior_start_manifest_sha256": self.prior_start_manifest_sha256,
            "target_name": self.target_name,
            "target_uuid": self.target_uuid,
        }


@dataclass(frozen=True)
class LaunchDiagnosticResult:
    outcome: str
    exit_code: int
    evidence_root: Path
    manifest_sha256: str


BindingValidator = Callable[[LaunchDiagnosticRequest], dict[str, object]]


def run_launch_diagnostics(
    request: LaunchDiagnosticRequest,
    *,
    runner: CommandRunner | None = None,
    binding_validator: BindingValidator | None = None,
) -> LaunchDiagnosticResult:
    request.validate()
    try:
        writer = start_control.EvidenceWriter.create(request.output_root)
    except start_control.StartControlError as exc:
        raise LaunchDiagnosticError(str(exc)) from exc
    writer.write_json("request.json", request.as_json())
    command_runner = runner or SubprocessCommandRunner()
    validate_bindings = binding_validator or validate_diagnostic_bindings

    stage = "binding-preflight"
    reason = "not-run"
    outcome = "precondition-rejected"
    exit_code = EXIT_PRECONDITION_REJECTED
    host_process_observation = "not-performed"
    unified_log_observation = "not-performed"

    try:
        binding_result = validate_bindings(request)
        writer.write_json("binding-preflight.json", binding_result)

        stage = "utmctl-list-live"
        list_observation = command_runner.run(
            ("utmctl", "list"), request.command_timeout_seconds
        )
        writer.write_json("utmctl-list-live.json", list_observation.as_json())
        registered = start_control.parse_utmctl_list(list_observation)
        _require_inventory_stopped(registered, request)

        stage = "utmctl-status-live"
        status_observation = command_runner.run(
            ("utmctl", "status", request.target_uuid),
            request.command_timeout_seconds,
        )
        writer.write_json(
            "utmctl-status-live.json", status_observation.as_json()
        )
        if start_control.parse_utmctl_status(status_observation) != "stopped":
            raise LaunchDiagnosticError("target-not-stopped")

        stage = "host-process-observation"
        process_observation = command_runner.run(
            PROCESS_COMMAND, request.command_timeout_seconds
        )
        host_process_observation = "attempted"
        writer.write_json(
            "host-process-command.json",
            _redacted_command_metadata(process_observation),
        )
        _require_success(process_observation, "host-process-observation")
        relevant_processes = parse_relevant_processes(process_observation)
        writer.write_json(
            "host-processes.json",
            {
                "format": EVIDENCE_FORMAT,
                "relevant_process_count": len(relevant_processes),
                "relevant_processes": list(relevant_processes),
            },
        )
        host_process_observation = "performed"

        stage = "unified-log-observation"
        log_observation = command_runner.run(
            _log_command(request), request.log_timeout_seconds
        )
        unified_log_observation = "attempted"
        writer.write_json(
            "unified-log-command.json",
            _redacted_command_metadata(log_observation),
        )
        _require_success(log_observation, "unified-log-observation")
        events = parse_unified_log_events(log_observation)
        writer.write_json(
            "unified-log.json",
            {
                "event_count": len(events),
                "events": list(events),
                "format": EVIDENCE_FORMAT,
                "log_end": request.log_end,
                "log_start": request.log_start,
                "predicate_sha256": hashlib.sha256(
                    LOG_PREDICATE.encode("utf-8")
                ).hexdigest(),
            },
        )
        unified_log_observation = "performed"
        outcome = "diagnostics-collected"
        exit_code = EXIT_COLLECTED
        reason = "read-only-host-facts-collected"
    except (
        LaunchDiagnosticError,
        start_control.StartControlError,
    ) as exc:
        reason = f"{stage}:{exc}"
        if stage in (
            "binding-preflight",
            "utmctl-list-live",
            "utmctl-status-live",
        ):
            outcome = "precondition-rejected"
            exit_code = EXIT_PRECONDITION_REJECTED
        else:
            outcome = "diagnostics-incomplete"
            exit_code = EXIT_DIAGNOSTICS_INCOMPLETE

    writer.write_json(
        "terminal.json",
        {
            "automatic_delete": "not-performed",
            "automatic_retry": "not-performed",
            "format": EVIDENCE_FORMAT,
            "guest_exec": "not-performed",
            "host_process_observation": host_process_observation,
            "input_transfer": "not-performed",
            "operation_id": "not-generated",
            "outcome": outcome,
            "reason": reason,
            "root_cause": "unattributed",
            "target_name": request.target_name,
            "target_uuid": request.target_uuid,
            "transaction": "not-performed",
            "unified_log_observation": unified_log_observation,
            "utm_clone": "not-performed",
            "utm_start": "not-performed",
            "utm_stop": "not-performed",
        },
    )
    manifest_sha256 = writer.write_manifest()
    return LaunchDiagnosticResult(
        outcome=outcome,
        exit_code=exit_code,
        evidence_root=request.output_root,
        manifest_sha256=manifest_sha256,
    )


def validate_diagnostic_bindings(
    request: LaunchDiagnosticRequest,
) -> dict[str, object]:
    repository_head = _run_git(
        request.repository_root, ("rev-parse", "HEAD")
    ).decode("ascii").strip()
    if repository_head != request.expected_repository_head:
        raise LaunchDiagnosticError("repository-head-drift")
    if _run_git(request.repository_root, ("status", "--porcelain")):
        raise LaunchDiagnosticError("repository-not-clean")
    control_sha256 = _validate_control_identity(request.repository_root)
    prior = _validate_prior_start_evidence(request)
    related = _validate_related_evidence(request)
    return {
        "control_sha256": control_sha256,
        "format": EVIDENCE_FORMAT,
        **prior,
        **related,
        "repository_clean": True,
        "repository_head": repository_head,
    }


def _validate_prior_start_evidence(
    request: LaunchDiagnosticRequest,
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
            raise LaunchDiagnosticError(f"{label}-format")
        if value.get("clone_uuid") != request.target_uuid:
            raise LaunchDiagnosticError(f"{label}-target-uuid")
        if value.get("clone_name") != request.target_name:
            raise LaunchDiagnosticError(f"{label}-target-name")

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
            raise LaunchDiagnosticError(
                f"prior-start-terminal-field:{field}"
            )
    status_poll_count = terminal_value.get("status_poll_count")
    if (
        not isinstance(status_poll_count, int)
        or isinstance(status_poll_count, bool)
        or not 1 <= status_poll_count <= 300
    ):
        raise LaunchDiagnosticError("prior-start-terminal-status-poll-count")
    if not isinstance(terminal_value.get("start_command_timed_out"), bool):
        raise LaunchDiagnosticError("prior-start-terminal-timeout")
    start_exit = terminal_value.get("start_command_exit_code")
    if start_exit is not None and (
        not isinstance(start_exit, int) or isinstance(start_exit, bool)
    ):
        raise LaunchDiagnosticError("prior-start-terminal-exit-code")
    return {
        "prior_start_entries_verified": len(entry_names),
        "prior_start_manifest_sha256": request.prior_start_manifest_sha256,
        "prior_start_outcome": terminal_value["outcome"],
        "prior_start_status_poll_count": status_poll_count,
    }


def _validate_related_evidence(
    request: LaunchDiagnosticRequest,
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


def _verify_bound_manifest(
    root: Path, expected_manifest_sha256: str, label: str
) -> frozenset[str]:
    manifest_path = root / "files.sha256"
    try:
        if start_control._sha256_file(manifest_path) != expected_manifest_sha256:
            raise LaunchDiagnosticError(f"{label}-manifest-drift")
        verified_entries = start_control._verify_sha256_manifest(
            root, manifest_path
        )
    except start_control.StartControlError as exc:
        raise LaunchDiagnosticError(f"{label}:{exc}") from exc
    try:
        entry_names = frozenset(
            line[66:]
            for line in manifest_path.read_text(encoding="ascii").splitlines()
        )
    except (OSError, UnicodeDecodeError) as exc:
        raise LaunchDiagnosticError(f"{label}-manifest-unreadable") from exc
    if len(entry_names) != verified_entries:
        raise LaunchDiagnosticError(f"{label}-manifest-entry-count")
    return entry_names


def _require_manifest_entries(
    actual: frozenset[str], required: frozenset[str], label: str
) -> None:
    missing = sorted(required - actual)
    if missing:
        raise LaunchDiagnosticError(
            f"{label}-manifest-required-entry:{missing[0]}"
        )


def _read_key_value_evidence(path: Path, label: str) -> dict[str, str]:
    try:
        payload = path.read_bytes()
    except OSError as exc:
        raise LaunchDiagnosticError(f"{label}-unavailable") from exc
    if not payload or len(payload) > 64 * 1024 or not payload.endswith(b"\n"):
        raise LaunchDiagnosticError(f"{label}-size-or-newline")
    try:
        lines = payload.decode("utf-8").splitlines()
    except UnicodeDecodeError as exc:
        raise LaunchDiagnosticError(f"{label}-not-utf8") from exc
    if len(lines) > 128:
        raise LaunchDiagnosticError(f"{label}-line-budget")
    values: dict[str, str] = {}
    for line in lines:
        if line.count("=") != 1:
            raise LaunchDiagnosticError(f"{label}-line-invalid")
        key, value = line.split("=", maxsplit=1)
        if (
            not re.fullmatch(r"[a-z][a-z0-9_]*", key)
            or not value
            or any(character in "\x00\r\n" for character in value)
            or key in values
        ):
            raise LaunchDiagnosticError(f"{label}-field-invalid")
        values[key] = value
    return values


def _require_evidence_fields(
    actual: dict[str, str],
    expected: dict[str, str],
    label: str,
) -> None:
    for key, value in expected.items():
        if actual.get(key) != value:
            raise LaunchDiagnosticError(f"{label}-field:{key}")


def parse_relevant_processes(
    observation: CommandObservation,
) -> tuple[dict[str, object], ...]:
    _require_success(observation, "host-process-observation")
    try:
        text = observation.stdout.prefix.decode("utf-8")
    except UnicodeDecodeError as exc:
        raise LaunchDiagnosticError("host-process-output-not-utf8") from exc
    records: list[dict[str, object]] = []
    seen_pids: set[int] = set()
    for line in text.splitlines():
        fields = line.split(maxsplit=3)
        if len(fields) != 4:
            raise LaunchDiagnosticError("host-process-row-invalid")
        pid_text, parent_text, uid_text, executable_path = fields
        if not all(value.isascii() and value.isdigit() for value in fields[:3]):
            raise LaunchDiagnosticError("host-process-identifier-invalid")
        pid = int(pid_text)
        parent_pid = int(parent_text)
        uid = int(uid_text)
        if pid <= 0 or parent_pid < 0 or uid < 0 or pid in seen_pids:
            raise LaunchDiagnosticError("host-process-identifier-invalid")
        seen_pids.add(pid)
        executable_name = Path(executable_path).name
        role = _process_role(executable_name)
        if role is None:
            continue
        records.append(
            {
                "executable_name": executable_name,
                "executable_path_sha256": hashlib.sha256(
                    executable_path.encode("utf-8")
                ).hexdigest(),
                "parent_pid": parent_pid,
                "pid": pid,
                "role": role,
                "uid": uid,
            }
        )
    return tuple(sorted(records, key=lambda value: int(value["pid"])))


def parse_unified_log_events(
    observation: CommandObservation,
) -> tuple[dict[str, object], ...]:
    _require_success(observation, "unified-log-observation")
    events: list[dict[str, object]] = []
    raw_lines = tuple(
        raw_line
        for raw_line in observation.stdout.prefix.splitlines()
        if raw_line
    )
    finished_seen = False
    for line_index, raw_line in enumerate(raw_lines):
        if len(raw_line) > MAX_LOG_LINE_BYTES:
            raise LaunchDiagnosticError("unified-log-line-too-large")
        try:
            value = json.loads(raw_line)
        except (UnicodeDecodeError, json.JSONDecodeError) as exc:
            raise LaunchDiagnosticError("unified-log-line-invalid") from exc
        if not isinstance(value, dict):
            raise LaunchDiagnosticError("unified-log-event-not-object")
        if "finished" in value:
            if (
                finished_seen
                or line_index != len(raw_lines) - 1
                or value.get("finished") is not True
            ):
                raise LaunchDiagnosticError("unified-log-finished-invalid")
            finished_seen = True
            continue
        process_path = _bounded_string(
            value.get("processImagePath"),
            "unified-log-process-path",
            4096,
        )
        if not process_path.startswith("/"):
            raise LaunchDiagnosticError("unified-log-process-path-not-absolute")
        process_name = Path(process_path).name
        role = _process_role(process_name)
        if role is None:
            raise LaunchDiagnosticError("unified-log-process-unrelated")
        message = _bounded_string(
            value.get("eventMessage"),
            "unified-log-message",
            MAX_LOG_MESSAGE_BYTES,
        )
        timestamp = _bounded_string(
            value.get("timestamp"), "unified-log-timestamp", 128
        )
        events.append(
            {
                "category": _optional_bounded_string(
                    value.get("category"), "unified-log-category", 256
                ),
                "message_prefix": _redact_home_paths(message)[
                    :MAX_REDACTED_MESSAGE_CHARS
                ],
                "message_sha256": hashlib.sha256(
                    message.encode("utf-8")
                ).hexdigest(),
                "message_type": _optional_bounded_string(
                    value.get("messageType"), "unified-log-message-type", 64
                ),
                "process_name": process_name,
                "process_path_sha256": hashlib.sha256(
                    process_path.encode("utf-8")
                ).hexdigest(),
                "role": role,
                "subsystem": _optional_bounded_string(
                    value.get("subsystem"), "unified-log-subsystem", 256
                ),
                "timestamp": timestamp,
            }
        )
        if len(events) > MAX_LOG_EVENTS:
            raise LaunchDiagnosticError("unified-log-event-budget-exceeded")
    if not finished_seen:
        raise LaunchDiagnosticError("unified-log-finished-missing")
    return tuple(events)


def _require_inventory_stopped(
    registered: tuple[start_control.RegisteredVm, ...],
    request: LaunchDiagnosticRequest,
) -> None:
    if len(registered) != request.expected_vm_count:
        raise LaunchDiagnosticError("live-inventory-count")
    target = next(
        (item for item in registered if item.uuid == request.target_uuid), None
    )
    if (
        target is None
        or target.name != request.target_name
        or target.status != "stopped"
    ):
        raise LaunchDiagnosticError("live-target-not-stopped")
    if any(item.status != "stopped" for item in registered):
        raise LaunchDiagnosticError("live-peer-not-stopped")


def _redacted_command_metadata(
    observation: CommandObservation,
) -> dict[str, object]:
    return {
        "argv": list(observation.argv),
        "exit_code": observation.exit_code,
        "stderr": {
            "prefix_utf8": _redact_home_paths(
                observation.stderr.prefix.decode("utf-8", errors="replace")
            ),
            "sha256": observation.stderr.sha256,
            "total_bytes": observation.stderr.total_bytes,
            "truncated": observation.stderr.truncated,
        },
        "stdout": {
            "sha256": observation.stdout.sha256,
            "total_bytes": observation.stdout.total_bytes,
            "truncated": observation.stdout.truncated,
        },
        "timed_out": observation.timed_out,
    }


def _require_success(observation: CommandObservation, label: str) -> None:
    try:
        start_control._require_successful_observation(observation, label)
    except start_control.StartControlError as exc:
        raise LaunchDiagnosticError(str(exc)) from exc


def _run_git(repository_root: Path, arguments: tuple[str, ...]) -> bytes:
    try:
        return start_control._run_git(repository_root, arguments)
    except start_control.StartControlError as exc:
        raise LaunchDiagnosticError(str(exc)) from exc


def _validate_control_identity(repository_root: Path) -> str:
    expected_path = repository_root / CONTROL_RELATIVE_PATH
    invoked_path = Path(__file__).absolute()
    if invoked_path != expected_path:
        raise LaunchDiagnosticError("executed-control-path-mismatch")
    try:
        control_stat = expected_path.lstat()
    except OSError as exc:
        raise LaunchDiagnosticError("executed-control-unavailable") from exc
    if (
        not stat.S_ISREG(control_stat.st_mode)
        or stat.S_ISLNK(control_stat.st_mode)
        or control_stat.st_nlink != 1
        or stat.S_IMODE(control_stat.st_mode) & 0o022
    ):
        raise LaunchDiagnosticError("executed-control-identity-invalid")
    try:
        return start_control._sha256_file(expected_path)
    except start_control.StartControlError as exc:
        raise LaunchDiagnosticError(str(exc)) from exc


def _read_json_object(path: Path, label: str) -> dict[str, object]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise LaunchDiagnosticError(f"{label}-invalid") from exc
    if not isinstance(value, dict):
        raise LaunchDiagnosticError(f"{label}-not-object")
    return value


def _parse_utc_timestamp(value: str, label: str) -> datetime:
    if not UTC_TIMESTAMP.fullmatch(value):
        raise LaunchDiagnosticError(f"{label}-invalid")
    try:
        parsed = datetime.strptime(value, "%Y-%m-%d %H:%M:%S%z")
    except ValueError as exc:
        raise LaunchDiagnosticError(f"{label}-invalid") from exc
    return parsed.replace(tzinfo=timezone.utc)


def _bounded_string(value: object, label: str, maximum_bytes: int) -> str:
    if (
        not isinstance(value, str)
        or not value
        or len(value.encode("utf-8")) > maximum_bytes
        or "\x00" in value
    ):
        raise LaunchDiagnosticError(f"{label}-invalid")
    return value


def _optional_bounded_string(
    value: object, label: str, maximum_bytes: int
) -> str | None:
    if value is None:
        return None
    return _bounded_string(value, label, maximum_bytes)


def _process_role(executable_name: str) -> str | None:
    if executable_name == "UTM":
        return "utm-app"
    if executable_name == "utmctl":
        return "utmctl"
    if executable_name.startswith("qemu"):
        return "qemu"
    return None


def _redact_home_paths(value: str) -> str:
    return HOME_PATH.sub(
        lambda match: "/Users/<redacted>"
        if match.group(0).startswith("/Users/")
        else "/home/<redacted>",
        value,
    )


def _log_command(request: LaunchDiagnosticRequest) -> tuple[str, ...]:
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


def _path_is_within(path: Path, root: Path) -> bool:
    try:
        path.relative_to(root)
    except ValueError:
        return False
    return True


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Collect bounded read-only UTM launch diagnostics for a frozen "
            "failed-closed start without starting, stopping, or entering a VM."
        )
    )
    parser.add_argument("command", choices=("collect",))
    parser.add_argument("--repository-root", type=Path, required=True)
    parser.add_argument("--expected-repository-head", required=True)
    parser.add_argument("--prior-start-root", type=Path, required=True)
    parser.add_argument("--prior-start-manifest-sha256", required=True)
    parser.add_argument("--prior-failure-root", type=Path, required=True)
    parser.add_argument("--prior-failure-manifest-sha256", required=True)
    parser.add_argument("--prior-postverify-root", type=Path, required=True)
    parser.add_argument("--prior-postverify-manifest-sha256", required=True)
    parser.add_argument("--output-root", type=Path, required=True)
    parser.add_argument("--attempt-id", required=True)
    parser.add_argument("--target-uuid", required=True)
    parser.add_argument("--target-name", required=True)
    parser.add_argument("--expected-vm-count", type=int, required=True)
    parser.add_argument("--log-start", required=True)
    parser.add_argument("--log-end", required=True)
    parser.add_argument("--command-timeout-seconds", type=int, default=15)
    parser.add_argument("--log-timeout-seconds", type=int, default=30)
    parser.add_argument(
        "--authorized-host-launch-diagnostics", action="store_true"
    )
    parser.add_argument("--authorized-read-system-log", action="store_true")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    request = LaunchDiagnosticRequest(
        repository_root=args.repository_root,
        expected_repository_head=args.expected_repository_head,
        prior_start_root=args.prior_start_root,
        prior_start_manifest_sha256=args.prior_start_manifest_sha256,
        prior_failure_root=args.prior_failure_root,
        prior_failure_manifest_sha256=args.prior_failure_manifest_sha256,
        prior_postverify_root=args.prior_postverify_root,
        prior_postverify_manifest_sha256=(
            args.prior_postverify_manifest_sha256
        ),
        output_root=args.output_root,
        attempt_id=args.attempt_id,
        target_uuid=args.target_uuid,
        target_name=args.target_name,
        expected_vm_count=args.expected_vm_count,
        log_start=args.log_start,
        log_end=args.log_end,
        command_timeout_seconds=args.command_timeout_seconds,
        log_timeout_seconds=args.log_timeout_seconds,
        authorized_host_launch_diagnostics=(
            args.authorized_host_launch_diagnostics
        ),
        authorized_read_system_log=args.authorized_read_system_log,
    )
    try:
        result = run_launch_diagnostics(request)
    except LaunchDiagnosticError as exc:
        print(f"l6_utm_launch_diagnostics_error={exc}", file=sys.stderr)
        return EXIT_PRECONDITION_REJECTED
    print(f"launch_diagnostics_outcome={result.outcome}")
    print(f"evidence_root={result.evidence_root}")
    print(f"files_sha256={result.manifest_sha256}")
    return result.exit_code


if __name__ == "__main__":
    raise SystemExit(main())
