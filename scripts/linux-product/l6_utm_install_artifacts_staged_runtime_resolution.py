#!/usr/bin/env python3
from __future__ import annotations

import argparse
import re
import sys
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Callable, Protocol

import l6_utm_canonical_input_transfer as input_transfer
import l6_utm_guest_network_ready as network_ready
import l6_utm_install_artifacts_staged_reactivation as reactivation_control
import l6_utm_install_artifacts_staged_resume_evidence as resume_evidence
import l6_utm_install_artifacts_staged_runtime_resolution_bindings as bindings
import l6_utm_launch_transport_v2 as launch_transport
import l6_utm_start_once as start_control
import l6_v4_canonical_input_install as guest_installer


EVIDENCE_FORMAT = bindings.EVIDENCE_FORMAT
CONTROL_RELATIVE_PATH = bindings.CONTROL_RELATIVE_PATH
BINDINGS_RELATIVE_PATH = bindings.BINDINGS_RELATIVE_PATH
REQUIRED_PRIOR_REACTIVATION_MANIFEST_SHA256 = (
    bindings.REQUIRED_PRIOR_REACTIVATION_MANIFEST_SHA256
)
REQUIRED_RUNTIME_RESOLUTION_ATTEMPT_ID = (
    bindings.REQUIRED_RUNTIME_RESOLUTION_ATTEMPT_ID
)
EXIT_ORIGINAL_BOOT_RESTORED = 0
EXIT_NEW_BOOT_STARTED = 10
EXIT_PRECONDITION_REJECTED = 11
EXIT_STATE_INDETERMINATE = 12
SAFE_ATTEMPT_ID = re.compile(r"[a-z0-9][a-z0-9-]{0,95}")


class RuntimeResolutionError(ValueError):
    pass


@dataclass(frozen=True)
class RuntimeResolutionRequest:
    repository_root: Path
    expected_repository_head: str
    prior_v7_root: Path
    prior_v7_manifest_sha256: str
    prior_prepared_root: Path
    prior_prepared_manifest_sha256: str
    prior_network_root: Path
    prior_network_manifest_sha256: str
    prior_transfer_root: Path
    prior_transfer_manifest_sha256: str
    prior_resolution_root: Path
    prior_resolution_manifest_sha256: str
    prior_preflight_root: Path
    prior_preflight_manifest_sha256: str
    prior_checkpoint_root: Path
    prior_checkpoint_manifest_sha256: str
    prior_resume_failure_root: Path
    prior_resume_failure_manifest_sha256: str
    prior_backend_resolution_root: Path
    prior_backend_resolution_manifest_sha256: str
    prior_reactivation_root: Path
    prior_reactivation_manifest_sha256: str
    source_bundle_path: Path
    source_bundle_size: int
    source_bundle_sha256: str
    output_root: Path
    transfer_attempt_id: str
    resolution_attempt_id: str
    preflight_attempt_id: str
    checkpoint_attempt_id: str
    resume_attempt_id: str
    backend_resolution_attempt_id: str
    reactivation_attempt_id: str
    runtime_resolution_attempt_id: str
    target_uuid: str
    target_name: str
    target_package_path: Path
    expected_vm_count: int
    identity_observations: int
    poll_interval_seconds: int
    command_timeout_seconds: int
    authorized_install_artifacts_staged_runtime_resolution: bool
    authorized_one_potential_backend_reactivation_list: bool
    authorized_stable_target_handle_pid_observations: bool
    authorized_one_read_only_boot_id_hash: bool
    authorized_no_start_status_resume_business_guest_retry_stop_or_quit: bool

    def validate(self) -> None:
        _prior_reactivation_request_view(self).validate()
        for path, label in (
            (self.prior_reactivation_root, "prior-reactivation-root"),
            (self.output_root, "output-root"),
        ):
            if not path.is_absolute() or ".." in path.parts:
                raise RuntimeResolutionError(
                    f"{label}-must-be-absolute-normalized"
                )
        protected = (
            (self.repository_root, "repository-root"),
            (self.prior_v7_root, "prior-v7-root"),
            (self.prior_prepared_root, "prior-prepared-root"),
            (self.prior_network_root, "prior-network-root"),
            (self.prior_transfer_root, "prior-transfer-root"),
            (self.prior_resolution_root, "prior-resolution-root"),
            (self.prior_preflight_root, "prior-preflight-root"),
            (self.prior_checkpoint_root, "prior-checkpoint-root"),
            (self.prior_resume_failure_root, "prior-resume-failure-root"),
            (
                self.prior_backend_resolution_root,
                "prior-backend-resolution-root",
            ),
            (self.prior_reactivation_root, "prior-reactivation-root"),
            (self.source_bundle_path, "source-bundle-path"),
            (self.target_package_path, "target-package-path"),
        )
        for path, label in protected:
            if _paths_overlap(self.output_root, path):
                raise RuntimeResolutionError(
                    f"output-root-must-not-overlap-{label}"
                )
        if (
            self.prior_reactivation_manifest_sha256
            != REQUIRED_PRIOR_REACTIVATION_MANIFEST_SHA256
        ):
            raise RuntimeResolutionError(
                "required-prior-reactivation-manifest-mismatch"
            )
        if (
            self.runtime_resolution_attempt_id
            != REQUIRED_RUNTIME_RESOLUTION_ATTEMPT_ID
            or not SAFE_ATTEMPT_ID.fullmatch(
                self.runtime_resolution_attempt_id
            )
        ):
            raise RuntimeResolutionError(
                "required-runtime-resolution-attempt-id-mismatch"
            )
        upstream_attempts = (
            self.transfer_attempt_id,
            self.resolution_attempt_id,
            self.preflight_attempt_id,
            self.checkpoint_attempt_id,
            self.resume_attempt_id,
            self.backend_resolution_attempt_id,
            self.reactivation_attempt_id,
        )
        if self.runtime_resolution_attempt_id in upstream_attempts:
            raise RuntimeResolutionError("attempt-id-overlap")
        if self.identity_observations != 3:
            raise RuntimeResolutionError(
                "identity-observations-must-be-exactly-3"
            )
        if not 1 <= self.poll_interval_seconds <= 10:
            raise RuntimeResolutionError("poll-interval-out-of-range")
        if not 1 <= self.command_timeout_seconds <= 60:
            raise RuntimeResolutionError("command-timeout-out-of-range")
        no_mutation_authorized = (
            self.authorized_no_start_status_resume_business_guest_retry_stop_or_quit
        )
        authorizations = (
            (
                self.authorized_install_artifacts_staged_runtime_resolution,
                "authorized-install-artifacts-staged-runtime-resolution-required",
            ),
            (
                self.authorized_one_potential_backend_reactivation_list,
                "authorized-one-potential-backend-reactivation-list-required",
            ),
            (
                self.authorized_stable_target_handle_pid_observations,
                "authorized-stable-target-handle-pid-observations-required",
            ),
            (
                self.authorized_one_read_only_boot_id_hash,
                "authorized-one-read-only-boot-id-hash-required",
            ),
            (
                no_mutation_authorized,
                "authorized-no-start-status-resume-business-guest-retry-"
                "stop-or-quit-required",
            ),
        )
        for authorized, reason in authorizations:
            if not authorized:
                raise RuntimeResolutionError(reason)

    def as_json(self) -> dict[str, object]:
        return {
            "authorization": {
                "install_artifacts_staged_runtime_resolution": True,
                "no_start_status_resume_business_guest_retry_stop_or_quit": True,
                "one_potential_backend_reactivation_list": True,
                "one_read_only_boot_id_hash": True,
                "stable_target_handle_pid_observations": True,
            },
            "backend_resolution_attempt_id": (
                self.backend_resolution_attempt_id
            ),
            "checkpoint_attempt_id": self.checkpoint_attempt_id,
            "command_timeout_seconds": self.command_timeout_seconds,
            "expected_repository_head": self.expected_repository_head,
            "expected_vm_count": self.expected_vm_count,
            "format": EVIDENCE_FORMAT,
            "identity_observations": self.identity_observations,
            "poll_interval_seconds": self.poll_interval_seconds,
            "prior_backend_resolution_manifest_sha256": (
                self.prior_backend_resolution_manifest_sha256
            ),
            "prior_checkpoint_manifest_sha256": (
                self.prior_checkpoint_manifest_sha256
            ),
            "prior_network_manifest_sha256": (
                self.prior_network_manifest_sha256
            ),
            "prior_preflight_manifest_sha256": (
                self.prior_preflight_manifest_sha256
            ),
            "prior_prepared_manifest_sha256": (
                self.prior_prepared_manifest_sha256
            ),
            "prior_reactivation_manifest_sha256": (
                self.prior_reactivation_manifest_sha256
            ),
            "prior_resolution_manifest_sha256": (
                self.prior_resolution_manifest_sha256
            ),
            "prior_resume_failure_manifest_sha256": (
                self.prior_resume_failure_manifest_sha256
            ),
            "prior_transfer_manifest_sha256": (
                self.prior_transfer_manifest_sha256
            ),
            "prior_v7_manifest_sha256": self.prior_v7_manifest_sha256,
            "reactivation_attempt_id": self.reactivation_attempt_id,
            "resolution_attempt_id": self.resolution_attempt_id,
            "resume_attempt_id": self.resume_attempt_id,
            "runtime_resolution_attempt_id": (
                self.runtime_resolution_attempt_id
            ),
            "source_bundle_path_sha256": network_ready._sha256_text(
                str(self.source_bundle_path)
            ),
            "source_bundle_sha256": self.source_bundle_sha256,
            "source_bundle_size": self.source_bundle_size,
            "target_name": self.target_name,
            "target_package_path_sha256": network_ready._sha256_text(
                str(self.target_package_path)
            ),
            "target_uuid": self.target_uuid,
            "transfer_attempt_id": self.transfer_attempt_id,
        }


@dataclass(frozen=True)
class RuntimeResolutionResult:
    outcome: str
    exit_code: int
    evidence_root: Path
    manifest_sha256: str
    inventory_probe_invocations: int
    guest_boot_hash_invocations: int
    identity_observation_count: int


class CommandRunner(Protocol):
    def run(
        self, argv: tuple[str, ...], timeout_seconds: int
    ) -> start_control.CommandObservation: ...


Sleeper = Callable[[float], None]


def run_runtime_resolution(
    request: RuntimeResolutionRequest,
    *,
    runner: CommandRunner | None = None,
    binding_validator=None,
    source_opener=None,
    source_revalidator=None,
    target_validator=None,
    sleeper: Sleeper = time.sleep,
) -> RuntimeResolutionResult:
    request.validate()
    writer = start_control.EvidenceWriter.create(request.output_root)
    writer.write_json("request.json", request.as_json())
    command_runner = runner or start_control.SubprocessCommandRunner()
    validate_bindings = binding_validator or validate_runtime_resolution_bindings
    open_source = source_opener or input_transfer.open_source_bundle
    revalidate_source = (
        source_revalidator or input_transfer.revalidate_open_source_bundle
    )
    validate_target = target_validator or network_ready.validate_target_files

    stage = "binding-preflight"
    reason = "not-run"
    outcome = "precondition-rejected"
    exit_code = EXIT_PRECONDITION_REJECTED
    inventory_probe_invocations = 0
    guest_boot_hash_invocations = 0
    identity_observation_count = 0
    source_bundle: input_transfer.SourceBundle | None = None
    target_preflight: dict[str, object] | None = None
    backend_pid: int | None = None
    terminal_handles = "not-observed"
    terminal_processes: tuple[dict[str, object], ...] = ()
    observed_boot_hash: str | None = None

    try:
        binding = validate_bindings(request)
        writer.write_json("binding-preflight.json", binding.evidence)

        stage = "source-bundle-preflight"
        source_bundle = open_source(request)
        writer.write_json("source-bundle-preflight.json", source_bundle.as_json())

        stage = "target-files-preflight"
        target_preflight = validate_target(request)
        writer.write_json("target-files-preflight.json", target_preflight)

        stage = "host-process-preflight"
        process_observation, terminal_processes = _observe_processes(
            command_runner, request
        )
        writer.write_json(
            "host-process-preflight.json",
            _process_evidence(process_observation, terminal_processes),
        )
        _require_no_utmctl(terminal_processes, "preflight")

        stage = "utmctl-list-once"
        inventory_probe_invocations = 1
        list_observation = command_runner.run(
            ("utmctl", "list"), request.command_timeout_seconds
        )
        writer.write_json("utmctl-list-once.json", list_observation.as_json())
        inventory = start_control.parse_utmctl_list(list_observation)
        if (
            reactivation_control._require_inventory(
                inventory, binding.upstream.baseline_inventory, request
            )
            != "started"
        ):
            raise RuntimeResolutionError(
                "target-not-registered-started-for-runtime-resolution"
            )
        writer.write_json(
            "inventory-classification.json",
            {
                "format": EVIDENCE_FORMAT,
                "other_registered_vm_count": 20,
                "other_registered_vms": "all-stopped",
                "registered_vm_count": 21,
                "target_registered_status": "started",
                "target_uuid": request.target_uuid,
            },
        )

        stage = "target-handle-pid-discovery"
        discovery = command_runner.run(
            network_ready._lsof_argv(request), request.command_timeout_seconds
        )
        discovery_identity = parse_target_handle_process(
            discovery,
            request,
            expected_argv=network_ready._lsof_argv(request),
        )
        backend_pid = int(discovery_identity["backend_pid"])
        terminal_handles = "present"
        writer.write_json(
            "target-handle-pid-discovery.json",
            _handle_identity_evidence(discovery, discovery_identity),
        )

        stage = "target-handle-pid-confirmation"
        for index in range(1, request.identity_observations + 1):
            observation = command_runner.run(
                targeted_lsof_argv(request, backend_pid),
                request.command_timeout_seconds,
            )
            identity = parse_target_handle_process(
                observation,
                request,
                expected_argv=targeted_lsof_argv(request, backend_pid),
            )
            identity_observation_count = index
            writer.write_json(
                f"target-handle-pid-confirmation-{index:03d}.json",
                _handle_identity_evidence(observation, identity),
            )
            if int(identity["backend_pid"]) != backend_pid:
                raise RuntimeResolutionError("target-backend-pid-drift")

            process_observation, terminal_processes = _observe_processes(
                command_runner, request
            )
            writer.write_json(
                f"host-process-confirmation-{index:03d}.json",
                _process_evidence(process_observation, terminal_processes),
            )
            _require_no_utmctl(terminal_processes, f"confirmation-{index:03d}")
            if index < request.identity_observations:
                sleeper(float(request.poll_interval_seconds))

        stage = "target-files-ready"
        target_ready = validate_target(request)
        writer.write_json("target-files-ready.json", target_ready)
        _require_live_target_identity_stable(target_preflight, target_ready)

        stage = "guest-boot-id-hash-once"
        guest_boot_hash_invocations = 1
        boot_observation = command_runner.run(
            reactivation_control.boot_id_hash_argv(request),
            request.command_timeout_seconds,
        )
        observed_boot_hash = reactivation_control.parse_boot_id_hash(
            boot_observation
        )
        classification = (
            "original-boot-restored"
            if observed_boot_hash == binding.expected_boot_id_sha256
            else "new-boot-started"
        )
        writer.write_json(
            "guest-boot-id-hash-once.json",
            {
                "classification": classification,
                "expected_boot_id_sha256": binding.expected_boot_id_sha256,
                "format": EVIDENCE_FORMAT,
                "observation": network_ready._observation_metadata(
                    boot_observation
                ),
                "observed_boot_id_sha256": observed_boot_hash,
            },
        )

        stage = "target-handle-pid-terminal"
        terminal_handle_observation = command_runner.run(
            targeted_lsof_argv(request, backend_pid),
            request.command_timeout_seconds,
        )
        terminal_identity = parse_target_handle_process(
            terminal_handle_observation,
            request,
            expected_argv=targeted_lsof_argv(request, backend_pid),
        )
        writer.write_json(
            "target-handle-pid-terminal.json",
            _handle_identity_evidence(
                terminal_handle_observation, terminal_identity
            ),
        )
        if int(terminal_identity["backend_pid"]) != backend_pid:
            raise RuntimeResolutionError("target-backend-pid-terminal-drift")

        stage = "host-process-terminal"
        process_observation, terminal_processes = _observe_processes(
            command_runner, request
        )
        writer.write_json(
            "host-process-terminal.json",
            _process_evidence(process_observation, terminal_processes),
        )
        _require_no_utmctl(terminal_processes, "terminal")

        stage = "target-files-postflight"
        target_postflight = validate_target(request)
        writer.write_json("target-files-postflight.json", target_postflight)
        _require_live_target_identity_stable(target_ready, target_postflight)

        if classification == "original-boot-restored":
            outcome = classification
            exit_code = EXIT_ORIGINAL_BOOT_RESTORED
            reason = "stable-target-pid-and-original-boot-hash-observed"
        else:
            outcome = classification
            exit_code = EXIT_NEW_BOOT_STARTED
            reason = "stable-target-pid-and-new-boot-hash-observed"
    except (
        RuntimeResolutionError,
        input_transfer.CanonicalInputTransferError,
        network_ready.NetworkReadyError,
        reactivation_control.ReactivationError,
        start_control.StartControlError,
        OSError,
        ValueError,
    ) as exc:
        reason = f"{stage}:{exc}"
        if inventory_probe_invocations == 1:
            outcome = "state-indeterminate"
            exit_code = EXIT_STATE_INDETERMINATE
    finally:
        if source_bundle is not None:
            try:
                writer.write_json(
                    "source-bundle-postflight.json",
                    revalidate_source(request, source_bundle),
                )
            except (
                input_transfer.CanonicalInputTransferError,
                guest_installer.GuestInputInstallError,
                OSError,
                ValueError,
            ) as exc:
                reason = f"source-bundle-postflight:{exc}"
                outcome = (
                    "state-indeterminate"
                    if inventory_probe_invocations == 1
                    else "precondition-rejected"
                )
                exit_code = (
                    EXIT_STATE_INDETERMINATE
                    if inventory_probe_invocations == 1
                    else EXIT_PRECONDITION_REJECTED
                )
            source_bundle.file_object.close()

    terminal = {
        "automatic_cleanup": "not-performed",
        "automatic_quit": "not-performed",
        "automatic_retry": "not-performed",
        "automatic_stop": "not-performed",
        "backend_pid": backend_pid,
        "business_guest_action": "not-performed",
        "file_pull_invocations": 0,
        "file_push_invocations": 0,
        "foreground_start_invocations": 0,
        "format": EVIDENCE_FORMAT,
        "guest_boot_hash_invocations": guest_boot_hash_invocations,
        "identity_observation_count": identity_observation_count,
        "inventory_probe_invocations": inventory_probe_invocations,
        "maintenance_resume_invocations": 0,
        "observed_boot_id_sha256": observed_boot_hash,
        "operation_id": "not-read-or-generated",
        "outcome": outcome,
        "plain_utmctl_list": (
            "attempted-once-as-potential-backend-reactivation"
            if inventory_probe_invocations == 1
            else "not-performed"
        ),
        "plain_utmctl_start": "not-performed",
        "plain_utmctl_status": "not-performed",
        "reason": reason,
        "runtime_resolution_attempt_id": (
            request.runtime_resolution_attempt_id
        ),
        "target_handles_terminal": terminal_handles,
        "target_name": request.target_name,
        "target_uuid": request.target_uuid,
        "terminal_relevant_host_process_count": len(terminal_processes),
        "transaction": "artifacts-staged-preserved-no-resume",
    }
    writer.write_json("terminal.json", terminal)
    _require_no_raw_operation_id(request.output_root)
    manifest_sha256 = writer.write_manifest()
    return RuntimeResolutionResult(
        outcome=outcome,
        exit_code=exit_code,
        evidence_root=request.output_root,
        manifest_sha256=manifest_sha256,
        inventory_probe_invocations=inventory_probe_invocations,
        guest_boot_hash_invocations=guest_boot_hash_invocations,
        identity_observation_count=identity_observation_count,
    )


def validate_runtime_resolution_bindings(
    request: RuntimeResolutionRequest,
) -> bindings.RuntimeResolutionBinding:
    if Path(__file__).absolute() != request.repository_root / CONTROL_RELATIVE_PATH:
        raise RuntimeResolutionError("executed-control-path-mismatch")
    if Path(bindings.__file__).absolute() != (
        request.repository_root / BINDINGS_RELATIVE_PATH
    ):
        raise RuntimeResolutionError("executed-bindings-path-mismatch")
    try:
        return bindings.validate_runtime_resolution_bindings(request)
    except ValueError as exc:
        raise RuntimeResolutionError(str(exc)) from exc


def parse_target_handle_process(
    observation: start_control.CommandObservation,
    request: RuntimeResolutionRequest,
    *,
    expected_argv: tuple[str, ...],
) -> dict[str, object]:
    if observation.argv != expected_argv:
        raise RuntimeResolutionError("target-handle-command-argv-mismatch")
    try:
        start_control._require_successful_observation(
            observation, "target-handle-process"
        )
    except start_control.StartControlError as exc:
        raise RuntimeResolutionError(str(exc)) from exc
    try:
        text = observation.stdout.prefix.decode("utf-8")
    except UnicodeDecodeError as exc:
        raise RuntimeResolutionError("target-handle-output-not-utf8") from exc
    records: list[tuple[int, str, tuple[str, ...]]] = []
    current_pid: int | None = None
    current_command: str | None = None
    current_paths: list[str] = []

    def finish_record() -> None:
        nonlocal current_pid, current_command, current_paths
        if current_pid is not None or current_command is not None or current_paths:
            if current_pid is None or current_command is None:
                raise RuntimeResolutionError(
                    "target-handle-process-record-incomplete"
                )
            records.append((current_pid, current_command, tuple(current_paths)))
        current_pid = None
        current_command = None
        current_paths = []

    for line in text.splitlines():
        if not line:
            raise RuntimeResolutionError("target-handle-row-empty")
        field, value = line[0], line[1:]
        if field == "p":
            finish_record()
            if not value.isascii() or not value.isdigit() or int(value) <= 1:
                raise RuntimeResolutionError("target-handle-pid-invalid")
            current_pid = int(value)
        elif field == "c":
            if current_command is not None or not value:
                raise RuntimeResolutionError("target-handle-command-invalid")
            current_command = value
        elif field == "n":
            current_paths.append(value)
        elif field not in ("f", "t"):
            raise RuntimeResolutionError("target-handle-field-invalid")
    finish_record()

    expected_efi = str(request.target_package_path / "Data/efi_vars.fd")
    expected_qcow2 = str(
        request.target_package_path
        / "Data"
        / network_ready.REQUIRED_QCOW2_NAME
    )
    if len(records) != 1:
        raise RuntimeResolutionError("target-handle-process-count-invalid")
    pid, command, paths = records[0]
    if command != "QEMULauncher":
        raise RuntimeResolutionError("target-handle-command-unexpected")
    if (
        paths.count(expected_efi) != 1
        or paths.count(expected_qcow2) != 1
        or set(paths) != {expected_efi, expected_qcow2}
    ):
        raise RuntimeResolutionError("target-handle-paths-invalid")
    return {
        "backend_command": command,
        "backend_pid": pid,
        "efi_handle_count": 1,
        "process_record_count": 1,
        "qcow2_handle_count": 1,
        "state": "present",
    }


def targeted_lsof_argv(
    request: RuntimeResolutionRequest, backend_pid: int
) -> tuple[str, ...]:
    if backend_pid <= 1:
        raise RuntimeResolutionError("target-backend-pid-invalid")
    data = request.target_package_path / "Data"
    return (
        "/usr/sbin/lsof",
        "-n",
        "-P",
        "-a",
        "-p",
        str(backend_pid),
        "-F",
        "pctfn",
        str(data / "efi_vars.fd"),
        str(data / network_ready.REQUIRED_QCOW2_NAME),
    )


def _prior_reactivation_request_view(
    request: RuntimeResolutionRequest,
) -> reactivation_control.ReactivationRequest:
    return reactivation_control.ReactivationRequest(
        repository_root=request.repository_root,
        expected_repository_head=request.expected_repository_head,
        prior_v7_root=request.prior_v7_root,
        prior_v7_manifest_sha256=request.prior_v7_manifest_sha256,
        prior_prepared_root=request.prior_prepared_root,
        prior_prepared_manifest_sha256=request.prior_prepared_manifest_sha256,
        prior_network_root=request.prior_network_root,
        prior_network_manifest_sha256=request.prior_network_manifest_sha256,
        prior_transfer_root=request.prior_transfer_root,
        prior_transfer_manifest_sha256=request.prior_transfer_manifest_sha256,
        prior_resolution_root=request.prior_resolution_root,
        prior_resolution_manifest_sha256=(
            request.prior_resolution_manifest_sha256
        ),
        prior_preflight_root=request.prior_preflight_root,
        prior_preflight_manifest_sha256=request.prior_preflight_manifest_sha256,
        prior_checkpoint_root=request.prior_checkpoint_root,
        prior_checkpoint_manifest_sha256=(
            request.prior_checkpoint_manifest_sha256
        ),
        prior_resume_failure_root=request.prior_resume_failure_root,
        prior_resume_failure_manifest_sha256=(
            request.prior_resume_failure_manifest_sha256
        ),
        prior_backend_resolution_root=request.prior_backend_resolution_root,
        prior_backend_resolution_manifest_sha256=(
            request.prior_backend_resolution_manifest_sha256
        ),
        source_bundle_path=request.source_bundle_path,
        source_bundle_size=request.source_bundle_size,
        source_bundle_sha256=request.source_bundle_sha256,
        output_root=request.prior_reactivation_root,
        transfer_attempt_id=request.transfer_attempt_id,
        resolution_attempt_id=request.resolution_attempt_id,
        preflight_attempt_id=request.preflight_attempt_id,
        checkpoint_attempt_id=request.checkpoint_attempt_id,
        resume_attempt_id=request.resume_attempt_id,
        backend_resolution_attempt_id=request.backend_resolution_attempt_id,
        reactivation_attempt_id=request.reactivation_attempt_id,
        target_uuid=request.target_uuid,
        target_name=request.target_name,
        target_package_path=request.target_package_path,
        expected_vm_count=request.expected_vm_count,
        settle_attempts=10,
        runtime_poll_attempts=30,
        poll_interval_seconds=1,
        transport_timeout_seconds=60,
        command_timeout_seconds=60,
        authorized_install_artifacts_staged_reactivation=True,
        authorized_one_potential_backend_reactivation_list=True,
        authorized_one_foreground_start_if_stopped_quiescent=True,
        authorized_one_read_only_boot_id_hash=True,
        authorized_no_business_guest_resume_retry_stop_or_quit=True,
    )


def _observe_processes(
    runner: CommandRunner, request: RuntimeResolutionRequest
) -> tuple[
    start_control.CommandObservation, tuple[dict[str, object], ...]
]:
    observation = runner.run(
        launch_transport.PROCESS_COMMAND, request.command_timeout_seconds
    )
    return observation, launch_transport.parse_relevant_processes(observation)


def _require_no_utmctl(
    processes: tuple[dict[str, object], ...], label: str
) -> None:
    if launch_transport._role_count(processes, "utmctl") != 0:
        raise RuntimeResolutionError(f"utmctl-process-active:{label}")


def _process_evidence(
    observation: start_control.CommandObservation,
    processes: tuple[dict[str, object], ...],
) -> dict[str, object]:
    return {
        "format": EVIDENCE_FORMAT,
        "observation": network_ready._observation_metadata(observation),
        "relevant_process_count": len(processes),
        "relevant_processes": list(processes),
    }


def _handle_identity_evidence(
    observation: start_control.CommandObservation,
    identity: dict[str, object],
) -> dict[str, object]:
    return {
        "format": EVIDENCE_FORMAT,
        "observation": network_ready._observation_metadata(observation),
        **identity,
    }


def _require_live_target_identity_stable(
    before: dict[str, object], after: dict[str, object]
) -> None:
    fixed_keys = (
        "format",
        "target_config_sha256",
        "target_package_name",
        "target_package_path_sha256",
    )
    if any(before.get(key) != after.get(key) for key in fixed_keys):
        raise RuntimeResolutionError("target-fixed-identity-drift")
    before_descriptors = before.get("target_descriptors")
    after_descriptors = after.get("target_descriptors")
    if not isinstance(before_descriptors, dict) or not isinstance(
        after_descriptors, dict
    ):
        raise RuntimeResolutionError("target-descriptors-invalid")
    for name in ("config", "efi", "qcow2"):
        before_value = before_descriptors.get(name)
        after_value = after_descriptors.get(name)
        if not isinstance(before_value, dict) or not isinstance(after_value, dict):
            raise RuntimeResolutionError("target-descriptor-invalid")
        for key in ("device", "inode", "mode"):
            if before_value.get(key) != after_value.get(key):
                raise RuntimeResolutionError(f"target-{name}-{key}-drift")
        before_size = before_value.get("size")
        after_size = after_value.get("size")
        if not isinstance(before_size, int) or not isinstance(after_size, int):
            raise RuntimeResolutionError(f"target-{name}-size-invalid")
        if name == "qcow2":
            if after_size < before_size:
                raise RuntimeResolutionError("target-qcow2-size-regressed")
        elif after_size != before_size:
            raise RuntimeResolutionError(f"target-{name}-size-drift")


def _require_no_raw_operation_id(root: Path) -> None:
    for path in root.iterdir():
        if path.is_file() and resume_evidence.RAW_OPERATION_TOKEN.search(
            path.read_bytes()
        ):
            raise RuntimeResolutionError(
                f"host-evidence-contains-raw-operation-id:{path.name}"
            )


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
            "Resolve one frozen reactivation runtime using one inventory "
            "probe, stable target-handle PID observations, and one boot hash."
        )
    )
    parser.add_argument("command", choices=("resolve-runtime-once",))
    for name in (
        "repository-root",
        "prior-v7-root",
        "prior-prepared-root",
        "prior-network-root",
        "prior-transfer-root",
        "prior-resolution-root",
        "prior-preflight-root",
        "prior-checkpoint-root",
        "prior-resume-failure-root",
        "prior-backend-resolution-root",
        "prior-reactivation-root",
        "source-bundle-path",
        "output-root",
        "target-package-path",
    ):
        parser.add_argument(f"--{name}", type=Path, required=True)
    for name in (
        "expected-repository-head",
        "prior-v7-manifest-sha256",
        "prior-prepared-manifest-sha256",
        "prior-network-manifest-sha256",
        "prior-transfer-manifest-sha256",
        "prior-resolution-manifest-sha256",
        "prior-preflight-manifest-sha256",
        "prior-checkpoint-manifest-sha256",
        "prior-resume-failure-manifest-sha256",
        "prior-backend-resolution-manifest-sha256",
        "prior-reactivation-manifest-sha256",
        "source-bundle-sha256",
        "transfer-attempt-id",
        "resolution-attempt-id",
        "preflight-attempt-id",
        "checkpoint-attempt-id",
        "resume-attempt-id",
        "backend-resolution-attempt-id",
        "reactivation-attempt-id",
        "runtime-resolution-attempt-id",
        "target-uuid",
        "target-name",
    ):
        parser.add_argument(f"--{name}", required=True)
    parser.add_argument("--source-bundle-size", type=int, required=True)
    parser.add_argument("--expected-vm-count", type=int, default=21)
    parser.add_argument("--identity-observations", type=int, default=3)
    parser.add_argument("--poll-interval-seconds", type=int, default=1)
    parser.add_argument("--command-timeout-seconds", type=int, default=60)
    parser.add_argument(
        "--authorized-install-artifacts-staged-runtime-resolution",
        action="store_true",
    )
    parser.add_argument(
        "--authorized-one-potential-backend-reactivation-list",
        action="store_true",
    )
    parser.add_argument(
        "--authorized-stable-target-handle-pid-observations",
        action="store_true",
    )
    parser.add_argument(
        "--authorized-one-read-only-boot-id-hash", action="store_true"
    )
    parser.add_argument(
        "--authorized-no-start-status-resume-business-guest-retry-stop-or-quit",
        action="store_true",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    values = vars(args).copy()
    values.pop("command")
    request = RuntimeResolutionRequest(**values)
    try:
        result = run_runtime_resolution(request)
    except RuntimeResolutionError as exc:
        print(
            f"runtime resolution rejected before evidence creation: {exc}",
            file=sys.stderr,
        )
        return EXIT_PRECONDITION_REJECTED
    print(f"outcome={result.outcome}")
    print(f"evidence_root={result.evidence_root}")
    print(f"manifest_sha256={result.manifest_sha256}")
    print(f"inventory_probe_invocations={result.inventory_probe_invocations}")
    print(f"guest_boot_hash_invocations={result.guest_boot_hash_invocations}")
    print(f"identity_observation_count={result.identity_observation_count}")
    return result.exit_code


if __name__ == "__main__":
    raise SystemExit(main())
