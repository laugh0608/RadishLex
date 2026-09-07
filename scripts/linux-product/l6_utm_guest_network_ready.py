#!/usr/bin/env python3
from __future__ import annotations

import argparse
import collections
import hashlib
import json
import os
import re
import stat
import subprocess
import sys
import uuid
from dataclasses import dataclass
from pathlib import Path
from typing import Protocol

import l6_utm_launch_transport_bindings as transport_bindings
import l6_utm_launch_transport_v2 as launch_transport
import l6_utm_start_once as start_control


EVIDENCE_FORMAT = "radishlex-linux-l6-utm-guest-network-ready-v1"
GUEST_EVIDENCE_FORMAT = "radishlex-linux-l6-v4-network-ready-v1"
CONTROL_RELATIVE_PATH = Path(
    "scripts/linux-product/l6_utm_guest_network_ready.py"
)
GUEST_SCRIPT_RELATIVE_PATH = Path(
    "scripts/linux-product/l6_v4_guest_network_ready.sh"
)
REQUIRED_PRIOR_LAUNCH_MANIFEST_SHA256 = (
    "6dbbdf404c30cfc4718dc99161c1d60fd8a063bd453fd0c5fb69ebb083b5fc3c"
)
REQUIRED_PRIOR_NETWORK_FAILURE_MANIFEST_SHA256 = (
    "d5d332120cadea7fe795f15ec31ca2d9eebb3a2565b109d42c133fb6fd127348"
)
REQUIRED_TARGET_UUID = transport_bindings.REQUIRED_TARGET_UUID
REQUIRED_TARGET_NAME = transport_bindings.REQUIRED_TARGET_NAME
REQUIRED_TARGET_CONFIG_SHA256 = (
    "2de7280bbf906a8e899662c8686cb108755c83263150f5bd455490b050536195"
)
REQUIRED_QCOW2_NAME = "FFF05A20-E829-493C-8F40-B40884425A3F.qcow2"
PROCESS_COMMAND = launch_transport.PROCESS_COMMAND
EXIT_NETWORK_READY = 0
EXIT_NETWORK_NOT_READY = 10
EXIT_PRECONDITION_REJECTED = 11
EXIT_STATE_INDETERMINATE = 12
HEX_40 = re.compile(r"[0-9a-f]{40}")
HEX_64 = re.compile(r"[0-9a-f]{64}")
SAFE_ATTEMPT_ID = re.compile(r"[a-z0-9][a-z0-9-]{0,63}")


class NetworkReadyError(ValueError):
    pass


@dataclass(frozen=True)
class NetworkReadyRequest:
    repository_root: Path
    expected_repository_head: str
    prior_launch_root: Path
    prior_launch_manifest_sha256: str
    prior_network_failure_root: Path
    prior_network_failure_manifest_sha256: str
    output_root: Path
    attempt_id: str
    target_uuid: str
    target_name: str
    target_package_path: Path
    command_timeout_seconds: int
    authorized_guest_network_shutdown_and_verify: bool
    authorized_control_transfer: bool
    authorized_no_business_input_transaction_or_stop: bool

    @property
    def guest_root(self) -> str:
        return f"/var/tmp/radishlex-l6-v4-network-ready-{self.attempt_id}"

    def validate(self) -> None:
        for path, label in (
            (self.repository_root, "repository-root"),
            (self.prior_launch_root, "prior-launch-root"),
            (self.prior_network_failure_root, "prior-network-failure-root"),
            (self.output_root, "output-root"),
            (self.target_package_path, "target-package-path"),
        ):
            if not path.is_absolute() or ".." in path.parts:
                raise NetworkReadyError(f"{label}-must-be-absolute-normalized")
        if _path_is_within(self.output_root, self.repository_root):
            raise NetworkReadyError("output-root-must-be-outside-repository")
        if _path_is_within(self.output_root, self.prior_launch_root):
            raise NetworkReadyError("output-root-must-not-modify-prior-launch")
        if _path_is_within(self.output_root, self.prior_network_failure_root):
            raise NetworkReadyError(
                "output-root-must-not-modify-prior-network-failure"
            )
        if _paths_overlap(self.output_root, self.target_package_path):
            raise NetworkReadyError("output-root-must-not-overlap-target")
        if not HEX_40.fullmatch(self.expected_repository_head):
            raise NetworkReadyError("expected-repository-head-invalid")
        if (
            self.prior_launch_manifest_sha256
            != REQUIRED_PRIOR_LAUNCH_MANIFEST_SHA256
        ):
            raise NetworkReadyError("required-prior-launch-manifest-mismatch")
        if (
            self.prior_network_failure_manifest_sha256
            != REQUIRED_PRIOR_NETWORK_FAILURE_MANIFEST_SHA256
        ):
            raise NetworkReadyError(
                "required-prior-network-failure-manifest-mismatch"
            )
        if not SAFE_ATTEMPT_ID.fullmatch(self.attempt_id):
            raise NetworkReadyError("attempt-id-invalid")
        try:
            canonical_uuid = str(uuid.UUID(self.target_uuid)).upper()
        except ValueError as exc:
            raise NetworkReadyError("target-uuid-invalid") from exc
        if canonical_uuid != self.target_uuid:
            raise NetworkReadyError("target-uuid-not-canonical")
        if self.target_uuid != REQUIRED_TARGET_UUID:
            raise NetworkReadyError("required-target-uuid-mismatch")
        if self.target_name != REQUIRED_TARGET_NAME:
            raise NetworkReadyError("required-target-name-mismatch")
        expected_package = transport_bindings.expected_target_package_path(
            self.target_name
        )
        if self.target_package_path != expected_package:
            raise NetworkReadyError("target-package-path-mismatch")
        if not 1 <= self.command_timeout_seconds <= 60:
            raise NetworkReadyError("command-timeout-seconds-out-of-range")
        if not self.authorized_guest_network_shutdown_and_verify:
            raise NetworkReadyError(
                "authorized-guest-network-shutdown-and-verify-required"
            )
        if not self.authorized_control_transfer:
            raise NetworkReadyError("authorized-control-transfer-required")
        if not self.authorized_no_business_input_transaction_or_stop:
            raise NetworkReadyError(
                "authorized-no-business-input-transaction-or-stop-required"
            )

    def as_json(self) -> dict[str, object]:
        return {
            "attempt_id": self.attempt_id,
            "authorization": {
                "control_transfer": True,
                "guest_network_shutdown_and_verify": True,
                "no_business_input_transaction_or_stop": True,
            },
            "command_timeout_seconds": self.command_timeout_seconds,
            "expected_repository_head": self.expected_repository_head,
            "format": EVIDENCE_FORMAT,
            "guest_root": self.guest_root,
            "prior_launch_manifest_sha256": (
                self.prior_launch_manifest_sha256
            ),
            "prior_network_failure_manifest_sha256": (
                self.prior_network_failure_manifest_sha256
            ),
            "target_name": self.target_name,
            "target_package_path_sha256": _sha256_text(
                str(self.target_package_path)
            ),
            "target_uuid": self.target_uuid,
        }


@dataclass(frozen=True)
class NetworkReadyResult:
    outcome: str
    exit_code: int
    evidence_root: Path
    manifest_sha256: str
    network_script_invocations: int
    file_pull_invocations: int


class CommandRunner(Protocol):
    def run(
        self,
        argv: tuple[str, ...],
        timeout_seconds: int,
        *,
        stdin_bytes: bytes | None = None,
    ) -> start_control.CommandObservation: ...


class SubprocessCommandRunner:
    def run(
        self,
        argv: tuple[str, ...],
        timeout_seconds: int,
        *,
        stdin_bytes: bytes | None = None,
    ) -> start_control.CommandObservation:
        environment = os.environ.copy()
        environment["LC_ALL"] = "C"
        environment["LANG"] = "C"
        kwargs: dict[str, object] = {
            "capture_output": True,
            "check": False,
            "env": environment,
            "shell": False,
            "timeout": timeout_seconds,
        }
        if stdin_bytes is None:
            kwargs["stdin"] = subprocess.DEVNULL
        else:
            kwargs["input"] = stdin_bytes
        try:
            completed = subprocess.run(argv, **kwargs)
        except subprocess.TimeoutExpired as exc:
            return start_control.CommandObservation.from_bytes(
                argv,
                exit_code=None,
                timed_out=True,
                stdout=_timeout_bytes(exc.stdout),
                stderr=_timeout_bytes(exc.stderr),
            )
        except OSError as exc:
            raise NetworkReadyError(
                f"command-unavailable:{argv[0]}:{exc.__class__.__name__}"
            ) from exc
        return start_control.CommandObservation.from_bytes(
            argv,
            exit_code=completed.returncode,
            stdout=completed.stdout,
            stderr=completed.stderr,
        )


def run_guest_network_ready(
    request: NetworkReadyRequest,
    *,
    runner: CommandRunner | None = None,
    binding_validator=None,
) -> NetworkReadyResult:
    request.validate()
    writer = start_control.EvidenceWriter.create(request.output_root)
    writer.write_json("request.json", request.as_json())
    command_runner = runner or SubprocessCommandRunner()
    validate_bindings = binding_validator or validate_network_bindings

    stage = "binding-preflight"
    reason = "not-run"
    outcome = "precondition-rejected"
    exit_code = EXIT_PRECONDITION_REJECTED
    guest_exec_invocations = 0
    file_push_invocations = 0
    file_pull_invocations = 0
    network_script_invocations = 0
    network_observation: start_control.CommandObservation | None = None
    parsed_evidence: dict[str, object] | None = None

    try:
        binding = validate_bindings(request)
        writer.write_json("binding-preflight.json", binding)

        stage = "target-files-preflight"
        writer.write_json(
            "target-files-preflight.json", validate_target_files(request)
        )

        stage = "host-process-preflight"
        process_observation = command_runner.run(
            PROCESS_COMMAND, request.command_timeout_seconds
        )
        start_control._require_successful_observation(
            process_observation, "host-process-preflight"
        )
        processes = launch_transport.parse_relevant_processes(
            process_observation
        )
        role_counts = collections.Counter(
            str(item["role"]) for item in processes
        )
        if role_counts.get("utmctl", 0) != 0:
            raise NetworkReadyError("utmctl-process-active-before-guest-control")
        writer.write_json(
            "host-process-preflight.json",
            {
                "format": EVIDENCE_FORMAT,
                "observation": _observation_metadata(process_observation),
                "role_counts": dict(sorted(role_counts.items())),
            },
        )

        stage = "target-handles-preflight"
        lsof_argv = _lsof_argv(request)
        lsof_observation = command_runner.run(
            lsof_argv, request.command_timeout_seconds
        )
        handle_evidence = parse_target_handles(lsof_observation, request)
        writer.write_json(
            "target-handles-preflight.json",
            {
                "format": EVIDENCE_FORMAT,
                "observation": _observation_metadata(lsof_observation),
                **handle_evidence,
            },
        )

        guest_script = _read_guest_script(request.repository_root)
        guest_root = request.guest_root
        guest_incoming = f"{guest_root}/network-ready.incoming.sh"
        guest_script_path = f"{guest_root}/network-ready.sh"
        guest_evidence_path = f"{guest_root}/network.evidence"

        stage = "guest-root-create"
        guest_exec_invocations += 1
        setup_observation = command_runner.run(
            (
                "utmctl",
                "exec",
                request.target_uuid,
                "--cmd",
                "/bin/mkdir",
                "-m",
                "0700",
                guest_root,
            ),
            request.command_timeout_seconds,
        )
        writer.write_json("guest-root-create.json", setup_observation.as_json())
        start_control._require_successful_observation(
            setup_observation, "guest-root-create"
        )

        stage = "guest-script-push"
        file_push_invocations += 1
        push_observation = command_runner.run(
            (
                "utmctl",
                "file",
                "push",
                request.target_uuid,
                guest_incoming,
            ),
            request.command_timeout_seconds,
            stdin_bytes=guest_script,
        )
        writer.write_json("guest-script-push.json", push_observation.as_json())
        start_control._require_successful_observation(
            push_observation, "guest-script-push"
        )

        stage = "guest-script-normalize"
        guest_exec_invocations += 1
        normalize_observation = command_runner.run(
            (
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
                guest_incoming,
                guest_script_path,
            ),
            request.command_timeout_seconds,
        )
        writer.write_json(
            "guest-script-normalize.json", normalize_observation.as_json()
        )
        start_control._require_successful_observation(
            normalize_observation, "guest-script-normalize"
        )

        stage = "guest-script-readback"
        file_pull_invocations += 1
        script_readback = command_runner.run(
            (
                "utmctl",
                "file",
                "pull",
                request.target_uuid,
                guest_script_path,
            ),
            request.command_timeout_seconds,
        )
        writer.write_json(
            "guest-script-readback.json", script_readback.as_json()
        )
        _require_exact_readback(
            script_readback, guest_script, "guest-script-readback"
        )

        stage = "guest-network-script"
        guest_exec_invocations += 1
        network_script_invocations = 1
        network_observation = command_runner.run(
            (
                "utmctl",
                "exec",
                request.target_uuid,
                "--cmd",
                "/bin/sh",
                guest_script_path,
                guest_root,
                request.attempt_id,
            ),
            request.command_timeout_seconds,
        )
        writer.write_json(
            "guest-network-script.json", network_observation.as_json()
        )
        if network_observation.timed_out:
            raise NetworkReadyError("guest-network-script-timed-out")
        if (
            network_observation.stdout.truncated
            or network_observation.stderr.truncated
        ):
            raise NetworkReadyError("guest-network-script-output-truncated")

        readbacks: list[start_control.CommandObservation] = []
        for index in (1, 2):
            stage = f"guest-network-evidence-readback-{index}"
            file_pull_invocations += 1
            observation = command_runner.run(
                (
                    "utmctl",
                    "file",
                    "pull",
                    request.target_uuid,
                    guest_evidence_path,
                ),
                request.command_timeout_seconds,
            )
            writer.write_json(
                f"guest-network-evidence-readback-{index}.json",
                observation.as_json(),
            )
            start_control._require_successful_observation(
                observation, stage
            )
            if observation.stdout.total_bytes == 0:
                raise NetworkReadyError(f"{stage}-empty")
            readbacks.append(observation)

        if readbacks[0].stdout.prefix != readbacks[1].stdout.prefix:
            raise NetworkReadyError("guest-network-evidence-readback-drift")
        parsed_evidence = parse_guest_network_evidence(
            readbacks[0].stdout.prefix, request
        )
        writer.write_json("guest-network-evidence.json", parsed_evidence)
        if parsed_evidence["outcome"] == "passed":
            outcome = "network-ready"
            exit_code = EXIT_NETWORK_READY
            reason = "double-readback-loopback-only-main-routes-empty"
        else:
            outcome = "network-not-ready"
            exit_code = EXIT_NETWORK_NOT_READY
            reason = f"guest-network-predicate:{parsed_evidence['reason']}"
    except (
        NetworkReadyError,
        start_control.StartControlError,
        transport_bindings.BindingError,
    ) as exc:
        reason = f"{stage}:{exc}"
        if network_script_invocations == 1:
            outcome = "state-indeterminate"
            exit_code = EXIT_STATE_INDETERMINATE

    terminal = {
        "automatic_quit": "not-performed",
        "automatic_retry": "not-performed",
        "automatic_stop": "not-performed",
        "business_input": "not-performed",
        "file_pull_invocations": file_pull_invocations,
        "file_push_invocations": file_push_invocations,
        "format": EVIDENCE_FORMAT,
        "guest_exec_invocations": guest_exec_invocations,
        "network_evidence_outcome": (
            parsed_evidence.get("outcome") if parsed_evidence else None
        ),
        "network_script_command_exit_code": (
            network_observation.exit_code if network_observation else None
        ),
        "network_script_command_timed_out": (
            network_observation.timed_out if network_observation else False
        ),
        "network_script_invocations": network_script_invocations,
        "operation_id": "not-generated",
        "outcome": outcome,
        "plain_utmctl_list": "not-performed",
        "plain_utmctl_start": "not-performed",
        "plain_utmctl_status": "not-performed",
        "reason": reason,
        "target_name": request.target_name,
        "target_uuid": request.target_uuid,
        "transaction": "not-performed",
    }
    writer.write_json("terminal.json", terminal)
    manifest_sha256 = writer.write_manifest()
    return NetworkReadyResult(
        outcome=outcome,
        exit_code=exit_code,
        evidence_root=request.output_root,
        manifest_sha256=manifest_sha256,
        network_script_invocations=network_script_invocations,
        file_pull_invocations=file_pull_invocations,
    )


def validate_network_bindings(
    request: NetworkReadyRequest,
) -> dict[str, object]:
    expected_control_path = request.repository_root / CONTROL_RELATIVE_PATH
    if Path(__file__).absolute() != expected_control_path:
        raise NetworkReadyError("executed-control-path-mismatch")
    repository_head = start_control._run_git(
        request.repository_root, ("rev-parse", "HEAD")
    ).decode("ascii").strip()
    if repository_head != request.expected_repository_head:
        raise NetworkReadyError("repository-head-drift")
    if start_control._run_git(
        request.repository_root, ("status", "--porcelain")
    ):
        raise NetworkReadyError("repository-not-clean")

    identities = {}
    for relative_path, label in (
        (CONTROL_RELATIVE_PATH, "control"),
        (GUEST_SCRIPT_RELATIVE_PATH, "guest_script"),
    ):
        path = request.repository_root / relative_path
        _require_committed_regular(path, label)
        identities[f"{label}_sha256"] = _sha256_file(path)

    manifest = request.prior_launch_root / "files.sha256"
    if _sha256_file(manifest) != request.prior_launch_manifest_sha256:
        raise NetworkReadyError("prior-launch-manifest-drift")
    entries = start_control._verify_sha256_manifest(
        request.prior_launch_root, manifest
    )
    if entries != 16:
        raise NetworkReadyError("prior-launch-entry-count-invalid")
    terminal = _read_json(request.prior_launch_root / "terminal.json")
    required_terminal = {
        "automatic_delete": "not-performed",
        "automatic_quit": "not-performed",
        "automatic_retry": "not-performed",
        "automatic_stop": "not-performed",
        "guest_exec": "not-performed",
        "input_transfer": "not-performed",
        "launch_invocations": 1,
        "operation_id": "not-generated",
        "outcome": "state-indeterminate",
        "plain_utmctl_start": "not-performed",
        "target_name": request.target_name,
        "target_uuid": request.target_uuid,
        "transaction": "not-performed",
        "transport_command_exit_code": 0,
        "transport_command_timed_out": False,
        "transport_id": "foreground-applescript-v1",
    }
    if any(terminal.get(key) != value for key, value in required_terminal.items()):
        raise NetworkReadyError("prior-launch-terminal-semantics-invalid")
    prior_request = _read_json(request.prior_launch_root / "request.json")
    if (
        prior_request.get("target_uuid") != request.target_uuid
        or prior_request.get("target_name") != request.target_name
        or prior_request.get("expected_repository_head")
        != "296a1c5f8b705d3e42debf123e3d048e87f36024"
    ):
        raise NetworkReadyError("prior-launch-request-semantics-invalid")
    failure_manifest = request.prior_network_failure_root / "files.sha256"
    if (
        _sha256_file(failure_manifest)
        != request.prior_network_failure_manifest_sha256
    ):
        raise NetworkReadyError("prior-network-failure-manifest-drift")
    failure_entries = start_control._verify_sha256_manifest(
        request.prior_network_failure_root, failure_manifest
    )
    if failure_entries != 7:
        raise NetworkReadyError("prior-network-failure-entry-count-invalid")
    failure_terminal = _read_json(
        request.prior_network_failure_root / "terminal.json"
    )
    required_failure_terminal = {
        "automatic_quit": "not-performed",
        "automatic_retry": "not-performed",
        "automatic_stop": "not-performed",
        "business_input": "not-performed",
        "file_pull_invocations": 0,
        "file_push_invocations": 0,
        "guest_exec_invocations": 1,
        "network_evidence_outcome": None,
        "network_script_invocations": 0,
        "operation_id": "not-generated",
        "outcome": "precondition-rejected",
        "plain_utmctl_list": "not-performed",
        "plain_utmctl_start": "not-performed",
        "plain_utmctl_status": "not-performed",
        "reason": "guest-root-create:guest-root-create-exit-64",
        "target_name": request.target_name,
        "target_uuid": request.target_uuid,
        "transaction": "not-performed",
    }
    if any(
        failure_terminal.get(key) != value
        for key, value in required_failure_terminal.items()
    ):
        raise NetworkReadyError(
            "prior-network-failure-terminal-semantics-invalid"
        )
    failure_request = _read_json(
        request.prior_network_failure_root / "request.json"
    )
    if (
        failure_request.get("attempt_id") != "d75818f-v4-20260823"
        or failure_request.get("expected_repository_head")
        != "e75509aba0160bddfbdeb72ed9543f08b9e97ae6"
        or failure_request.get("prior_launch_manifest_sha256")
        != request.prior_launch_manifest_sha256
        or failure_request.get("target_uuid") != request.target_uuid
        or failure_request.get("target_name") != request.target_name
    ):
        raise NetworkReadyError(
            "prior-network-failure-request-semantics-invalid"
        )
    return {
        "format": EVIDENCE_FORMAT,
        **identities,
        "prior_launch_entries_verified": entries,
        "prior_launch_manifest_sha256": request.prior_launch_manifest_sha256,
        "prior_network_failure_entries_verified": failure_entries,
        "prior_network_failure_manifest_sha256": (
            request.prior_network_failure_manifest_sha256
        ),
        "repository_clean": True,
        "repository_head": repository_head,
    }


def validate_target_files(
    request: NetworkReadyRequest,
) -> dict[str, object]:
    package = request.target_package_path
    data = package / "Data"
    config = package / "config.plist"
    efi = data / "efi_vars.fd"
    qcow2 = data / REQUIRED_QCOW2_NAME
    for path, label, expected_mode in (
        (package, "package", 0o755),
        (data, "data", 0o755),
    ):
        try:
            info = path.lstat()
        except OSError as exc:
            raise NetworkReadyError(
                f"target-{label}-unavailable"
            ) from exc
        if (
            not stat.S_ISDIR(info.st_mode)
            or stat.S_ISLNK(info.st_mode)
            or stat.S_IMODE(info.st_mode) != expected_mode
            or info.st_uid != 501
            or info.st_gid != 20
        ):
            raise NetworkReadyError(f"target-{label}-identity-invalid")
    descriptors: dict[str, object] = {}
    for path, label in (
        (config, "config"),
        (efi, "efi"),
        (qcow2, "qcow2"),
    ):
        try:
            info = path.lstat()
        except OSError as exc:
            raise NetworkReadyError(
                f"target-{label}-unavailable"
            ) from exc
        if (
            not stat.S_ISREG(info.st_mode)
            or stat.S_ISLNK(info.st_mode)
            or stat.S_IMODE(info.st_mode) != 0o644
            or info.st_uid != 501
            or info.st_gid != 20
            or info.st_nlink != 1
            or info.st_size <= 0
        ):
            raise NetworkReadyError(f"target-{label}-identity-invalid")
        descriptors[label] = {
            "device": info.st_dev,
            "inode": info.st_ino,
            "mode": "0644",
            "size": info.st_size,
        }
    if _sha256_file(config) != REQUIRED_TARGET_CONFIG_SHA256:
        raise NetworkReadyError("target-config-hash-drift")
    return {
        "format": EVIDENCE_FORMAT,
        "target_config_sha256": REQUIRED_TARGET_CONFIG_SHA256,
        "target_descriptors": descriptors,
        "target_package_name": package.name,
        "target_package_path_sha256": _sha256_text(str(package)),
    }


def parse_target_handles(
    observation: start_control.CommandObservation,
    request: NetworkReadyRequest,
) -> dict[str, object]:
    start_control._require_successful_observation(
        observation, "target-handles-preflight"
    )
    try:
        text = observation.stdout.prefix.decode("utf-8")
    except UnicodeDecodeError as exc:
        raise NetworkReadyError("lsof-output-not-utf8") from exc
    expected_efi = str(request.target_package_path / "Data/efi_vars.fd")
    expected_qcow2 = str(
        request.target_package_path / "Data" / REQUIRED_QCOW2_NAME
    )
    process_records: list[tuple[str, tuple[str, ...]]] = []
    current_command: str | None = None
    current_paths: list[str] = []

    def finish_record() -> None:
        nonlocal current_command, current_paths
        if current_command is not None or current_paths:
            if current_command is None:
                raise NetworkReadyError("lsof-process-command-missing")
            process_records.append((current_command, tuple(current_paths)))
        current_command = None
        current_paths = []

    for line in text.splitlines():
        if not line:
            raise NetworkReadyError("lsof-row-empty")
        field, value = line[0], line[1:]
        if field == "p":
            finish_record()
        elif field == "c":
            if current_command is not None:
                raise NetworkReadyError("lsof-command-duplicate")
            current_command = value
        elif field == "n":
            current_paths.append(value)
        elif field not in ("f", "t"):
            raise NetworkReadyError("lsof-field-invalid")
    finish_record()
    if not process_records:
        raise NetworkReadyError("target-handles-absent")
    if any(command != "QEMULauncher" for command, _ in process_records):
        raise NetworkReadyError("target-handles-unexpected-command")
    all_paths = [path for _, paths in process_records for path in paths]
    if all_paths.count(expected_efi) != 1 or all_paths.count(expected_qcow2) != 1:
        raise NetworkReadyError("target-handle-count-mismatch")
    if set(all_paths) != {expected_efi, expected_qcow2}:
        raise NetworkReadyError("target-handle-path-mismatch")
    return {
        "backend_command": "QEMULauncher",
        "efi_handle_count": 1,
        "process_record_count": len(process_records),
        "qcow2_handle_count": 1,
    }


def parse_guest_network_evidence(
    payload: bytes, request: NetworkReadyRequest
) -> dict[str, object]:
    try:
        text = payload.decode("ascii")
    except UnicodeDecodeError as exc:
        raise NetworkReadyError("guest-network-evidence-not-ascii") from exc
    expected_keys = (
        "format",
        "attempt_id",
        "boot_id",
        "outcome",
        "reason",
        "active_interface_count",
        "active_interfaces",
        "non_loopback_interface_count",
        "non_loopback_up_count",
        "ipv4_main_route_count",
        "ipv6_main_route_count",
    )
    lines = text.splitlines()
    if len(lines) != len(expected_keys) or not text.endswith("\n"):
        raise NetworkReadyError("guest-network-evidence-shape-invalid")
    values: dict[str, str] = {}
    for expected_key, line in zip(expected_keys, lines, strict=True):
        key, separator, value = line.partition("=")
        if separator != "=" or key != expected_key or key in values:
            raise NetworkReadyError("guest-network-evidence-key-invalid")
        values[key] = value
    if values["format"] != GUEST_EVIDENCE_FORMAT:
        raise NetworkReadyError("guest-network-evidence-format-invalid")
    if values["attempt_id"] != request.attempt_id:
        raise NetworkReadyError("guest-network-evidence-attempt-mismatch")
    try:
        parsed_boot_id = uuid.UUID(values["boot_id"])
    except ValueError as exc:
        raise NetworkReadyError("guest-network-evidence-boot-id-invalid") from exc
    if str(parsed_boot_id) != values["boot_id"]:
        raise NetworkReadyError("guest-network-evidence-boot-id-not-canonical")
    if values["outcome"] not in ("passed", "failed"):
        raise NetworkReadyError("guest-network-evidence-outcome-invalid")
    counts = {}
    for key in (
        "active_interface_count",
        "non_loopback_interface_count",
        "non_loopback_up_count",
        "ipv4_main_route_count",
        "ipv6_main_route_count",
    ):
        value = values[key]
        if not value.isascii() or not value.isdigit() or str(int(value)) != value:
            raise NetworkReadyError("guest-network-evidence-count-invalid")
        counts[key] = int(value)
    passed_predicates = (
        values["reason"] == "none"
        and values["active_interfaces"] == "lo"
        and counts["active_interface_count"] == 1
        and counts["non_loopback_up_count"] == 0
        and counts["ipv4_main_route_count"] == 0
        and counts["ipv6_main_route_count"] == 0
    )
    if (values["outcome"] == "passed") != passed_predicates:
        raise NetworkReadyError("guest-network-evidence-outcome-inconsistent")
    if values["outcome"] == "failed" and values["reason"] == "none":
        raise NetworkReadyError("guest-network-evidence-failure-reason-missing")
    return {
        "active_interface_count": counts["active_interface_count"],
        "active_interfaces": values["active_interfaces"],
        "attempt_id": values["attempt_id"],
        "boot_id": values["boot_id"],
        "format": values["format"],
        "ipv4_main_route_count": counts["ipv4_main_route_count"],
        "ipv6_main_route_count": counts["ipv6_main_route_count"],
        "non_loopback_interface_count": counts[
            "non_loopback_interface_count"
        ],
        "non_loopback_up_count": counts["non_loopback_up_count"],
        "outcome": values["outcome"],
        "reason": values["reason"],
    }


def _lsof_argv(request: NetworkReadyRequest) -> tuple[str, ...]:
    data = request.target_package_path / "Data"
    return (
        "/usr/sbin/lsof",
        "-n",
        "-P",
        "-F",
        "ctfn",
        str(data / "efi_vars.fd"),
        str(data / REQUIRED_QCOW2_NAME),
    )


def _read_guest_script(repository_root: Path) -> bytes:
    path = repository_root / GUEST_SCRIPT_RELATIVE_PATH
    _require_committed_regular(path, "guest-script")
    try:
        payload = path.read_bytes()
    except OSError as exc:
        raise NetworkReadyError("guest-script-unreadable") from exc
    if not payload or len(payload) > 32 * 1024:
        raise NetworkReadyError("guest-script-size-invalid")
    return payload


def _require_exact_readback(
    observation: start_control.CommandObservation,
    expected: bytes,
    label: str,
) -> None:
    start_control._require_successful_observation(observation, label)
    if observation.stdout.total_bytes != len(expected):
        raise NetworkReadyError(f"{label}-size-mismatch")
    if observation.stdout.prefix != expected:
        raise NetworkReadyError(f"{label}-content-mismatch")


def _observation_metadata(
    observation: start_control.CommandObservation,
) -> dict[str, object]:
    return {
        "argv": list(observation.argv),
        "exit_code": observation.exit_code,
        "stderr": {
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


def _require_committed_regular(path: Path, label: str) -> None:
    try:
        info = path.lstat()
    except OSError as exc:
        raise NetworkReadyError(f"{label}-unavailable") from exc
    if (
        not stat.S_ISREG(info.st_mode)
        or stat.S_ISLNK(info.st_mode)
        or info.st_nlink != 1
        or stat.S_IMODE(info.st_mode) & 0o022
    ):
        raise NetworkReadyError(f"{label}-identity-invalid")


def _read_json(path: Path) -> dict[str, object]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise NetworkReadyError(f"json-invalid:{path.name}") from exc
    if not isinstance(value, dict):
        raise NetworkReadyError(f"json-not-object:{path.name}")
    return value


def _sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    try:
        with path.open("rb") as source:
            while chunk := source.read(1024 * 1024):
                digest.update(chunk)
    except OSError as exc:
        raise NetworkReadyError(f"sha256-unreadable:{path.name}") from exc
    return digest.hexdigest()


def _sha256_text(value: str) -> str:
    return hashlib.sha256(value.encode("utf-8")).hexdigest()


def _timeout_bytes(value: bytes | str | None) -> bytes:
    if value is None:
        return b""
    if isinstance(value, bytes):
        return value
    return value.encode("utf-8", errors="replace")


def _path_is_within(path: Path, root: Path) -> bool:
    try:
        path.relative_to(root)
    except ValueError:
        return False
    return True


def _paths_overlap(first: Path, second: Path) -> bool:
    return _path_is_within(first, second) or _path_is_within(second, first)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Perform one authorized guest network shutdown and prove "
            "loopback-only state through two independent file readbacks."
        )
    )
    parser.add_argument("command", choices=("network-ready",))
    parser.add_argument("--repository-root", type=Path, required=True)
    parser.add_argument("--expected-repository-head", required=True)
    parser.add_argument("--prior-launch-root", type=Path, required=True)
    parser.add_argument(
        "--prior-launch-manifest-sha256", required=True
    )
    parser.add_argument("--prior-network-failure-root", type=Path, required=True)
    parser.add_argument(
        "--prior-network-failure-manifest-sha256", required=True
    )
    parser.add_argument("--output-root", type=Path, required=True)
    parser.add_argument("--attempt-id", required=True)
    parser.add_argument("--target-uuid", required=True)
    parser.add_argument("--target-name", required=True)
    parser.add_argument("--target-package-path", type=Path, required=True)
    parser.add_argument("--command-timeout-seconds", type=int, default=60)
    parser.add_argument(
        "--authorized-guest-network-shutdown-and-verify",
        action="store_true",
    )
    parser.add_argument("--authorized-control-transfer", action="store_true")
    parser.add_argument(
        "--authorized-no-business-input-transaction-or-stop",
        action="store_true",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    request = NetworkReadyRequest(
        repository_root=args.repository_root,
        expected_repository_head=args.expected_repository_head,
        prior_launch_root=args.prior_launch_root,
        prior_launch_manifest_sha256=args.prior_launch_manifest_sha256,
        prior_network_failure_root=args.prior_network_failure_root,
        prior_network_failure_manifest_sha256=(
            args.prior_network_failure_manifest_sha256
        ),
        output_root=args.output_root,
        attempt_id=args.attempt_id,
        target_uuid=args.target_uuid,
        target_name=args.target_name,
        target_package_path=args.target_package_path,
        command_timeout_seconds=args.command_timeout_seconds,
        authorized_guest_network_shutdown_and_verify=(
            args.authorized_guest_network_shutdown_and_verify
        ),
        authorized_control_transfer=args.authorized_control_transfer,
        authorized_no_business_input_transaction_or_stop=(
            args.authorized_no_business_input_transaction_or_stop
        ),
    )
    try:
        result = run_guest_network_ready(request)
    except (
        NetworkReadyError,
        start_control.StartControlError,
        transport_bindings.BindingError,
    ) as exc:
        print(f"l6_utm_guest_network_ready_error={exc}", file=sys.stderr)
        return EXIT_PRECONDITION_REJECTED
    print(f"guest_network_ready_outcome={result.outcome}")
    print(f"evidence_root={result.evidence_root}")
    print(f"files_sha256={result.manifest_sha256}")
    return result.exit_code


if __name__ == "__main__":
    raise SystemExit(main())
