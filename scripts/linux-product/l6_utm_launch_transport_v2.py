#!/usr/bin/env python3
from __future__ import annotations

import argparse
import hashlib
import re
import sys
import time
import uuid
from dataclasses import dataclass
from pathlib import Path
from typing import Callable, Protocol

import l6_utm_launch_transport_bindings as transport_bindings
import l6_utm_start_once as start_control


EVIDENCE_FORMAT = transport_bindings.EVIDENCE_FORMAT
TRANSPORT_ID = transport_bindings.TRANSPORT_ID
TRANSPORT_RELATIVE_PATH = transport_bindings.TRANSPORT_RELATIVE_PATH
PROCESS_COMMAND = ("/bin/ps", "-axo", "pid=,ppid=,uid=,ucomm=")
OSASCRIPT_PATH = "/usr/bin/osascript"
UTM_BUNDLE_ROOT = transport_bindings.UTM_BUNDLE_ROOT
EXPECTED_UTM_BUNDLE_ID = transport_bindings.EXPECTED_UTM_BUNDLE_ID
EXPECTED_UTM_VERSION = transport_bindings.EXPECTED_UTM_VERSION
EXPECTED_UTM_BUILD = transport_bindings.EXPECTED_UTM_BUILD
V7_EVIDENCE_FORMAT = transport_bindings.V7_EVIDENCE_FORMAT
REQUIRED_V7_MANIFEST_SHA256 = transport_bindings.REQUIRED_V7_MANIFEST_SHA256
REQUIRED_PREPARED_MANIFEST_SHA256 = (
    transport_bindings.REQUIRED_PREPARED_MANIFEST_SHA256
)
REQUIRED_TARGET_UUID = transport_bindings.REQUIRED_TARGET_UUID
REQUIRED_TARGET_NAME = transport_bindings.REQUIRED_TARGET_NAME
FROZEN_V7_TARGET_UUID = transport_bindings.FROZEN_V7_TARGET_UUID
FROZEN_V7_TARGET_NAME = transport_bindings.FROZEN_V7_TARGET_NAME
V7_LOG_START = transport_bindings.V7_LOG_START
V7_LOG_END = transport_bindings.V7_LOG_END
EXIT_STARTED = 0
EXIT_FAILED_CLOSED_STOPPED = 10
EXIT_PRECONDITION_REJECTED = 11
EXIT_STATE_INDETERMINATE = 12
HEX_40 = re.compile(r"[0-9a-f]{40}")
HEX_64 = re.compile(r"[0-9a-f]{64}")
SAFE_ATTEMPT_ID = re.compile(r"[a-z0-9][a-z0-9-]{0,63}")


class LaunchTransportError(ValueError):
    pass


@dataclass(frozen=True)
class LaunchTransportRequest:
    repository_root: Path
    expected_repository_head: str
    prior_v7_root: Path
    prior_v7_manifest_sha256: str
    prior_prepared_root: Path
    prior_prepared_manifest_sha256: str
    output_root: Path
    attempt_id: str
    target_uuid: str
    target_name: str
    target_package_path: Path
    expected_vm_count: int
    poll_attempts: int
    poll_interval_seconds: int
    transport_timeout_seconds: int
    command_timeout_seconds: int
    authorized_l6_vm_start: bool
    authorized_foreground_applescript_transport: bool
    authorized_no_automatic_stop_retry_delete: bool

    def validate(self) -> None:
        for path, label in (
            (self.repository_root, "repository-root"),
            (self.prior_v7_root, "prior-v7-root"),
            (self.prior_prepared_root, "prior-prepared-root"),
            (self.output_root, "output-root"),
            (self.target_package_path, "target-package-path"),
        ):
            if not path.is_absolute():
                raise LaunchTransportError(f"{label}-must-be-absolute")
            if ".." in path.parts:
                raise LaunchTransportError(f"{label}-must-be-normalized")
        if _path_is_within(self.output_root, self.repository_root):
            raise LaunchTransportError("output-root-must-be-outside-repository")
        if _path_is_within(self.output_root, self.prior_v7_root):
            raise LaunchTransportError("output-root-must-not-modify-prior-v7")
        if _path_is_within(self.output_root, self.prior_prepared_root):
            raise LaunchTransportError(
                "output-root-must-not-modify-prior-prepared"
            )
        if _path_is_within(
            self.output_root, self.target_package_path
        ) or _path_is_within(self.target_package_path, self.output_root):
            raise LaunchTransportError(
                "output-root-and-target-package-must-not-overlap"
            )
        if not HEX_40.fullmatch(self.expected_repository_head):
            raise LaunchTransportError("expected-repository-head-invalid")
        if self.prior_v7_manifest_sha256 != REQUIRED_V7_MANIFEST_SHA256:
            raise LaunchTransportError("required-v7-manifest-sha256-mismatch")
        if (
            self.prior_prepared_manifest_sha256
            != REQUIRED_PREPARED_MANIFEST_SHA256
        ):
            raise LaunchTransportError(
                "required-prepared-manifest-sha256-mismatch"
            )
        if not SAFE_ATTEMPT_ID.fullmatch(self.attempt_id):
            raise LaunchTransportError("attempt-id-invalid")
        _validate_uuid(self.target_uuid, "target-uuid")
        if self.target_uuid == FROZEN_V7_TARGET_UUID:
            raise LaunchTransportError("frozen-v7-target-reuse-forbidden")
        if self.target_uuid != REQUIRED_TARGET_UUID:
            raise LaunchTransportError("required-target-uuid-mismatch")
        _validate_vm_name(self.target_name, "target-name")
        if self.target_name == FROZEN_V7_TARGET_NAME:
            raise LaunchTransportError("frozen-v7-target-reuse-forbidden")
        if self.target_name != REQUIRED_TARGET_NAME:
            raise LaunchTransportError("required-target-name-mismatch")
        try:
            expected_target_package = (
                transport_bindings.expected_target_package_path(
                    self.target_name
                )
            )
        except transport_bindings.BindingError as exc:
            raise LaunchTransportError(str(exc)) from exc
        if self.target_package_path != expected_target_package:
            raise LaunchTransportError("target-package-path-mismatch")
        if self.target_package_path.name != f"{self.target_name}.utm":
            raise LaunchTransportError("target-package-name-mismatch")
        if self.expected_vm_count != 21:
            raise LaunchTransportError("expected-vm-count-must-be-21")
        if not 2 <= self.expected_vm_count <= 128:
            raise LaunchTransportError("expected-vm-count-out-of-range")
        if not 1 <= self.poll_attempts <= 300:
            raise LaunchTransportError("poll-attempts-out-of-range")
        if not 1 <= self.poll_interval_seconds <= 10:
            raise LaunchTransportError("poll-interval-seconds-out-of-range")
        if not 1 <= self.transport_timeout_seconds <= 300:
            raise LaunchTransportError("transport-timeout-seconds-out-of-range")
        if not 1 <= self.command_timeout_seconds <= 60:
            raise LaunchTransportError("command-timeout-seconds-out-of-range")
        if not self.authorized_l6_vm_start:
            raise LaunchTransportError("authorized-l6-vm-start-required")
        if not self.authorized_foreground_applescript_transport:
            raise LaunchTransportError(
                "authorized-foreground-applescript-transport-required"
            )
        if not self.authorized_no_automatic_stop_retry_delete:
            raise LaunchTransportError(
                "authorized-no-automatic-stop-retry-delete-required"
            )

    def as_json(self) -> dict[str, object]:
        return {
            "attempt_id": self.attempt_id,
            "authorization": {
                "foreground_applescript_transport": True,
                "l6_vm_start": True,
                "no_automatic_stop_retry_delete": True,
            },
            "command_timeout_seconds": self.command_timeout_seconds,
            "expected_repository_head": self.expected_repository_head,
            "expected_utm_build": EXPECTED_UTM_BUILD,
            "expected_utm_bundle_id": EXPECTED_UTM_BUNDLE_ID,
            "expected_utm_version": EXPECTED_UTM_VERSION,
            "expected_vm_count": self.expected_vm_count,
            "format": EVIDENCE_FORMAT,
            "poll_attempts": self.poll_attempts,
            "poll_interval_seconds": self.poll_interval_seconds,
            "prior_v7_manifest_sha256": self.prior_v7_manifest_sha256,
            "prior_prepared_manifest_sha256": (
                self.prior_prepared_manifest_sha256
            ),
            "target_name": self.target_name,
            "target_package_name": self.target_package_path.name,
            "target_package_path_sha256": _sha256_text(
                str(self.target_package_path)
            ),
            "target_uuid": self.target_uuid,
            "transport_id": TRANSPORT_ID,
            "transport_timeout_seconds": self.transport_timeout_seconds,
        }


@dataclass(frozen=True)
class LaunchTransportResult:
    outcome: str
    exit_code: int
    evidence_root: Path
    manifest_sha256: str
    launch_invocations: int
    status_poll_count: int


class CommandRunner(Protocol):
    def run(
        self, argv: tuple[str, ...], timeout_seconds: int
    ) -> start_control.CommandObservation: ...


BindingValidator = Callable[[LaunchTransportRequest], dict[str, object]]
TargetIdentityValidator = Callable[
    [LaunchTransportRequest, dict[str, object]], dict[str, object]
]
Sleeper = Callable[[float], None]


def run_launch_transport_once(
    request: LaunchTransportRequest,
    *,
    runner: CommandRunner | None = None,
    sleeper: Sleeper | None = None,
    binding_validator: BindingValidator | None = None,
    target_identity_validator: TargetIdentityValidator | None = None,
) -> LaunchTransportResult:
    request.validate()
    writer = start_control.EvidenceWriter.create(request.output_root)
    writer.write_json("request.json", request.as_json())
    command_runner = runner or start_control.SubprocessCommandRunner()
    sleep = sleeper or time.sleep
    validate_bindings = binding_validator or validate_launch_bindings
    validate_target = target_identity_validator or validate_target_identity

    launch_invocations = 0
    status_poll_count = 0
    target_identity_checks = 0
    stage = "binding-preflight"
    reason = "not-run"
    outcome = "precondition-rejected"
    exit_code = EXIT_PRECONDITION_REJECTED
    transport_observation: start_control.CommandObservation | None = None
    terminal_processes: tuple[dict[str, object], ...] = ()

    try:
        binding = validate_bindings(request)
        writer.write_json("binding-preflight.json", binding)
        baseline_inventory = _inventory_from_binding(binding)

        stage = "host-process-preflight"
        preflight_process_observation = command_runner.run(
            PROCESS_COMMAND, request.command_timeout_seconds
        )
        writer.write_json(
            "host-process-command-preflight.json",
            preflight_process_observation.as_json(),
        )
        preflight_processes = parse_relevant_processes(
            preflight_process_observation
        )
        writer.write_json(
            "host-processes-preflight.json",
            _process_evidence(preflight_processes),
        )
        if preflight_processes:
            raise LaunchTransportError("relevant-host-process-before-preflight")

        stage = "target-identity-preflight"
        target_identity_preflight = validate_target(request, binding)
        target_identity_checks = 1
        writer.write_json(
            "target-identity-preflight.json", target_identity_preflight
        )

        stage = "utmctl-list-prestart"
        prestart_list = command_runner.run(
            ("utmctl", "list"), request.command_timeout_seconds
        )
        writer.write_json("utmctl-list-prestart.json", prestart_list.as_json())
        prestart_vms = start_control.parse_utmctl_list(prestart_list)
        require_inventory_relative_to_v7(
            prestart_vms,
            baseline_inventory,
            request,
            target_status="stopped",
        )

        stage = "utmctl-status-prestart"
        prestart_status = command_runner.run(
            ("utmctl", "status", request.target_uuid),
            request.command_timeout_seconds,
        )
        writer.write_json(
            "utmctl-status-prestart.json", prestart_status.as_json()
        )
        if start_control.parse_utmctl_status(prestart_status) != "stopped":
            raise LaunchTransportError("target-not-stopped-before-transport")

        stage = "target-identity-ready"
        target_identity_ready = validate_target(request, binding)
        target_identity_checks = 2
        writer.write_json("target-identity-ready.json", target_identity_ready)
        if target_identity_ready != target_identity_preflight:
            raise LaunchTransportError("target-identity-changed-before-transport")

        stage = "host-process-ready"
        ready_process_observation = command_runner.run(
            PROCESS_COMMAND, request.command_timeout_seconds
        )
        writer.write_json(
            "host-process-command-ready.json", ready_process_observation.as_json()
        )
        ready_processes = parse_relevant_processes(ready_process_observation)
        writer.write_json(
            "host-processes-ready.json", _process_evidence(ready_processes)
        )
        if ready_processes:
            raise LaunchTransportError("relevant-host-process-before-transport")

        stage = "foreground-applescript-transport"
        launch_invocations = 1
        transport_observation = command_runner.run(
            transport_argv(request), request.transport_timeout_seconds
        )
        writer.write_json(
            "launch-transport.json", transport_observation.as_json()
        )

        observed_started = False
        stage = "utmctl-status-poll"
        for attempt in range(1, request.poll_attempts + 1):
            status_observation = command_runner.run(
                ("utmctl", "status", request.target_uuid),
                request.command_timeout_seconds,
            )
            status_poll_count = attempt
            writer.write_json(
                f"utmctl-status-poll-{attempt:03d}.json",
                status_observation.as_json(),
            )
            try:
                observed_status = start_control.parse_utmctl_status(
                    status_observation
                )
            except start_control.StartControlError:
                observed_status = None
            if observed_status == "started":
                observed_started = True
                stage = "host-process-poll"
                poll_process_observation = command_runner.run(
                    PROCESS_COMMAND, request.command_timeout_seconds
                )
                writer.write_json(
                    f"host-process-command-poll-{attempt:03d}.json",
                    poll_process_observation.as_json(),
                )
                poll_processes = parse_relevant_processes(
                    poll_process_observation
                )
                writer.write_json(
                    f"host-processes-poll-{attempt:03d}.json",
                    _process_evidence(poll_processes),
                )
                if (
                    _backend_process_count(poll_processes) >= 1
                    and _role_count(poll_processes, "utmctl") == 0
                ):
                    break
                stage = "utmctl-status-poll"
            if attempt < request.poll_attempts:
                sleep(request.poll_interval_seconds)

        stage = "utmctl-list-terminal"
        terminal_list = command_runner.run(
            ("utmctl", "list"), request.command_timeout_seconds
        )
        writer.write_json("utmctl-list-terminal.json", terminal_list.as_json())
        terminal_vms = start_control.parse_utmctl_list(terminal_list)

        stage = "host-process-terminal"
        terminal_process_observation = command_runner.run(
            PROCESS_COMMAND, request.command_timeout_seconds
        )
        writer.write_json(
            "host-process-command-terminal.json",
            terminal_process_observation.as_json(),
        )
        terminal_processes = parse_relevant_processes(
            terminal_process_observation
        )
        writer.write_json(
            "host-processes-terminal.json",
            _process_evidence(terminal_processes),
        )

        backend_count = _backend_process_count(terminal_processes)
        utmctl_count = _role_count(terminal_processes, "utmctl")
        if (
            observed_started
            and inventory_matches_relative_to_v7(
                terminal_vms,
                baseline_inventory,
                request,
                target_status="started",
            )
            and backend_count >= 1
            and utmctl_count == 0
        ):
            outcome = "started-observed"
            exit_code = EXIT_STARTED
            reason = "status-inventory-and-backend-started"
        elif (
            inventory_matches_relative_to_v7(
                terminal_vms,
                baseline_inventory,
                request,
                target_status="stopped",
            )
            and backend_count == 0
            and utmctl_count == 0
        ):
            outcome = "failed-closed-stopped"
            exit_code = EXIT_FAILED_CLOSED_STOPPED
            reason = "started-not-observed-all-vms-stopped-no-backend"
        else:
            outcome = "state-indeterminate"
            exit_code = EXIT_STATE_INDETERMINATE
            reason = "terminal-inventory-or-process-state-indeterminate"
    except (LaunchTransportError, start_control.StartControlError) as exc:
        reason = f"{stage}:{exc}"
        if launch_invocations == 1:
            outcome = "state-indeterminate"
            exit_code = EXIT_STATE_INDETERMINATE

    terminal = {
        "automatic_delete": "not-performed",
        "automatic_quit": "not-performed",
        "automatic_retry": "not-performed",
        "automatic_stop": "not-performed",
        "backend_process_count": _backend_process_count(terminal_processes),
        "format": EVIDENCE_FORMAT,
        "guest_exec": "not-performed",
        "input_transfer": "not-performed",
        "launch_invocations": launch_invocations,
        "operation_id": "not-generated",
        "outcome": outcome,
        "plain_utmctl_start": "not-performed",
        "reason": reason,
        "status_poll_count": status_poll_count,
        "target_name": request.target_name,
        "target_identity_checks": target_identity_checks,
        "target_uuid": request.target_uuid,
        "terminal_relevant_process_count": len(terminal_processes),
        "transaction": "not-performed",
        "transport_command_exit_code": (
            transport_observation.exit_code if transport_observation else None
        ),
        "transport_command_timed_out": (
            transport_observation.timed_out
            if transport_observation
            else False
        ),
        "transport_id": TRANSPORT_ID,
        "utm_clone": "not-performed",
        "utm_hide": "not-performed",
        "utm_stop": "not-performed",
    }
    writer.write_json("terminal.json", terminal)
    manifest_sha256 = writer.write_manifest()
    return LaunchTransportResult(
        outcome=outcome,
        exit_code=exit_code,
        evidence_root=request.output_root,
        manifest_sha256=manifest_sha256,
        launch_invocations=launch_invocations,
        status_poll_count=status_poll_count,
    )


def validate_launch_bindings(
    request: LaunchTransportRequest,
) -> dict[str, object]:
    if Path(__file__).absolute() != (
        request.repository_root / transport_bindings.CONTROL_RELATIVE_PATH
    ):
        raise LaunchTransportError("executed-control-path-mismatch")
    try:
        return transport_bindings.validate_launch_bindings(request)
    except transport_bindings.BindingError as exc:
        raise LaunchTransportError(str(exc)) from exc


def validate_target_identity(
    request: LaunchTransportRequest,
    prepared_binding: dict[str, object],
) -> dict[str, object]:
    try:
        return transport_bindings.validate_live_target_identity(
            request, prepared_binding
        )
    except transport_bindings.BindingError as exc:
        raise LaunchTransportError(str(exc)) from exc


validate_control_identity = transport_bindings.validate_control_identity
validate_v7_evidence = transport_bindings.validate_v7_evidence
validate_prepared_evidence = transport_bindings.validate_prepared_evidence
validate_utm_bundle = transport_bindings.validate_utm_bundle
expected_target_package_path = transport_bindings.expected_target_package_path


def transport_argv(request: LaunchTransportRequest) -> tuple[str, ...]:
    return (
        OSASCRIPT_PATH,
        str(request.repository_root / TRANSPORT_RELATIVE_PATH),
        request.target_uuid,
    )


def parse_relevant_processes(
    observation: start_control.CommandObservation,
) -> tuple[dict[str, object], ...]:
    try:
        start_control._require_successful_observation(
            observation, "host-process-observation"
        )
    except start_control.StartControlError as exc:
        raise LaunchTransportError(str(exc)) from exc
    try:
        text = observation.stdout.prefix.decode("utf-8")
    except UnicodeDecodeError as exc:
        raise LaunchTransportError("host-process-output-not-utf8") from exc
    records: list[dict[str, object]] = []
    seen_pids: set[int] = set()
    for line in text.splitlines():
        fields = line.split(maxsplit=3)
        if len(fields) != 4:
            raise LaunchTransportError("host-process-row-invalid")
        pid_text, parent_text, uid_text, accounting_name = fields
        if not all(
            value.isascii() and value.isdigit()
            for value in (pid_text, parent_text)
        ) or not (
            uid_text.isascii()
            and (uid_text.isdigit() or uid_text == "-2")
        ):
            raise LaunchTransportError("host-process-identifier-invalid")
        pid = int(pid_text)
        parent_pid = int(parent_text)
        uid = int(uid_text)
        if (
            pid < 0
            or parent_pid < 0
            or (uid < 0 and uid != -2)
            or pid in seen_pids
        ):
            raise LaunchTransportError("host-process-identifier-invalid")
        seen_pids.add(pid)
        if (
            not accounting_name
            or len(accounting_name.encode("utf-8")) > 256
            or any(character in "\x00\r\n/" for character in accounting_name)
        ):
            raise LaunchTransportError(
                "host-process-accounting-name-invalid"
            )
        if pid == 0 and (
            parent_pid != 0 or uid != 0 or accounting_name != "kernel_task"
        ):
            raise LaunchTransportError("host-process-identifier-invalid")
        role = _process_role(accounting_name)
        if role is not None:
            records.append(
                {
                    "accounting_name": accounting_name,
                    "parent_pid": parent_pid,
                    "pid": pid,
                    "role": role,
                    "uid": uid,
                }
            )
    return tuple(sorted(records, key=lambda item: int(item["pid"])))


def require_inventory_relative_to_v7(
    registered: tuple[start_control.RegisteredVm, ...],
    baseline: tuple[start_control.RegisteredVm, ...],
    request: LaunchTransportRequest,
    *,
    target_status: str,
) -> None:
    if not inventory_matches_relative_to_v7(
        registered, baseline, request, target_status=target_status
    ):
        raise LaunchTransportError(
            f"inventory-not-v7-plus-new-target:{target_status}"
        )


def inventory_matches_relative_to_v7(
    registered: tuple[start_control.RegisteredVm, ...],
    baseline: tuple[start_control.RegisteredVm, ...],
    request: LaunchTransportRequest,
    *,
    target_status: str,
) -> bool:
    if len(registered) != request.expected_vm_count:
        return False
    current = {item.uuid: item for item in registered}
    if len(current) != len(registered):
        return False
    target = current.pop(request.target_uuid, None)
    if (
        target is None
        or target.name != request.target_name
        or target.status != target_status
    ):
        return False
    baseline_by_uuid = {item.uuid: item for item in baseline}
    if set(current) != set(baseline_by_uuid):
        return False
    return all(
        current[vm_uuid].name == item.name
        and current[vm_uuid].status == "stopped"
        for vm_uuid, item in baseline_by_uuid.items()
    )


def _inventory_from_binding(
    binding: dict[str, object],
) -> tuple[start_control.RegisteredVm, ...]:
    value = binding.get("baseline_inventory")
    if not isinstance(value, list):
        raise LaunchTransportError("binding-baseline-inventory-invalid")
    inventory: list[start_control.RegisteredVm] = []
    seen: set[str] = set()
    for item in value:
        if not isinstance(item, dict):
            raise LaunchTransportError("binding-baseline-inventory-invalid")
        vm_uuid = item.get("uuid")
        name = item.get("name")
        status_value = item.get("status")
        if (
            not isinstance(vm_uuid, str)
            or not isinstance(name, str)
            or status_value != "stopped"
            or vm_uuid in seen
        ):
            raise LaunchTransportError("binding-baseline-inventory-invalid")
        _validate_uuid(vm_uuid, "binding-baseline-uuid")
        _validate_vm_name(name, "binding-baseline-name")
        seen.add(vm_uuid)
        inventory.append(start_control.RegisteredVm(vm_uuid, "stopped", name))
    if len(inventory) + 1 != binding.get("expected_current_vm_count"):
        raise LaunchTransportError("binding-baseline-inventory-invalid")
    return tuple(inventory)


def _process_evidence(
    records: tuple[dict[str, object], ...]
) -> dict[str, object]:
    return {
        "format": EVIDENCE_FORMAT,
        "relevant_process_count": len(records),
        "relevant_processes": list(records),
    }


def _process_role(accounting_name: str) -> str | None:
    if accounting_name == "UTM":
        return "utm-app"
    if accounting_name == "utmctl":
        return "utmctl"
    if accounting_name == "QEMUHelper":
        return "qemu-helper"
    if accounting_name == "QEMULauncher":
        return "qemu-launcher"
    if accounting_name.startswith("qemu"):
        return "qemu"
    return None


def _backend_process_count(records: tuple[dict[str, object], ...]) -> int:
    return sum(
        item.get("role") in ("qemu-helper", "qemu-launcher", "qemu")
        for item in records
    )


def _role_count(
    records: tuple[dict[str, object], ...], role: str
) -> int:
    return sum(item.get("role") == role for item in records)


def _validate_uuid(value: str, label: str) -> None:
    try:
        parsed = uuid.UUID(value)
    except ValueError as exc:
        raise LaunchTransportError(f"{label}-invalid") from exc
    if str(parsed).upper() != value:
        raise LaunchTransportError(f"{label}-must-be-uppercase-canonical")


def _validate_vm_name(value: str, label: str) -> None:
    if (
        not value
        or len(value.encode("utf-8")) > 160
        or any(character in "\x00\r\n" for character in value)
    ):
        raise LaunchTransportError(f"{label}-invalid")


def _path_is_within(path: Path, root: Path) -> bool:
    try:
        path.relative_to(root)
    except ValueError:
        return False
    return True


def _sha256_text(value: str) -> str:
    return hashlib.sha256(value.encode("utf-8")).hexdigest()


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Run one authorized foreground AppleScript UTM launch transport "
            "against a new target relative to the frozen v7 inventory."
        )
    )
    parser.add_argument("command", choices=("launch-once",))
    parser.add_argument("--repository-root", type=Path, required=True)
    parser.add_argument("--expected-repository-head", required=True)
    parser.add_argument("--prior-v7-root", type=Path, required=True)
    parser.add_argument("--prior-v7-manifest-sha256", required=True)
    parser.add_argument("--prior-prepared-root", type=Path, required=True)
    parser.add_argument("--prior-prepared-manifest-sha256", required=True)
    parser.add_argument("--output-root", type=Path, required=True)
    parser.add_argument("--attempt-id", required=True)
    parser.add_argument("--target-uuid", required=True)
    parser.add_argument("--target-name", required=True)
    parser.add_argument("--target-package-path", type=Path, required=True)
    parser.add_argument("--expected-vm-count", type=int, required=True)
    parser.add_argument("--poll-attempts", type=int, default=60)
    parser.add_argument("--poll-interval-seconds", type=int, default=1)
    parser.add_argument("--transport-timeout-seconds", type=int, default=90)
    parser.add_argument("--command-timeout-seconds", type=int, default=15)
    parser.add_argument("--authorized-l6-vm-start", action="store_true")
    parser.add_argument(
        "--authorized-foreground-applescript-transport", action="store_true"
    )
    parser.add_argument(
        "--authorized-no-automatic-stop-retry-delete", action="store_true"
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    request = LaunchTransportRequest(
        repository_root=args.repository_root,
        expected_repository_head=args.expected_repository_head,
        prior_v7_root=args.prior_v7_root,
        prior_v7_manifest_sha256=args.prior_v7_manifest_sha256,
        prior_prepared_root=args.prior_prepared_root,
        prior_prepared_manifest_sha256=(
            args.prior_prepared_manifest_sha256
        ),
        output_root=args.output_root,
        attempt_id=args.attempt_id,
        target_uuid=args.target_uuid,
        target_name=args.target_name,
        target_package_path=args.target_package_path,
        expected_vm_count=args.expected_vm_count,
        poll_attempts=args.poll_attempts,
        poll_interval_seconds=args.poll_interval_seconds,
        transport_timeout_seconds=args.transport_timeout_seconds,
        command_timeout_seconds=args.command_timeout_seconds,
        authorized_l6_vm_start=args.authorized_l6_vm_start,
        authorized_foreground_applescript_transport=(
            args.authorized_foreground_applescript_transport
        ),
        authorized_no_automatic_stop_retry_delete=(
            args.authorized_no_automatic_stop_retry_delete
        ),
    )
    try:
        result = run_launch_transport_once(request)
    except LaunchTransportError as exc:
        print(f"l6_utm_launch_transport_v2_error={exc}", file=sys.stderr)
        return EXIT_PRECONDITION_REJECTED
    print(f"launch_transport_outcome={result.outcome}")
    print(f"evidence_root={result.evidence_root}")
    print(f"files_sha256={result.manifest_sha256}")
    return result.exit_code


if __name__ == "__main__":
    raise SystemExit(main())
