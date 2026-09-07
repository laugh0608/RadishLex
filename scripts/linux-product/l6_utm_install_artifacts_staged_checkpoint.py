#!/usr/bin/env python3
from __future__ import annotations

import argparse
import hashlib
import re
import sys
import tarfile
import time
import uuid
from dataclasses import dataclass
from pathlib import Path
from typing import Callable, Protocol

import l6_utm_canonical_input_preflight as prior_preflight
import l6_utm_canonical_input_resolution as input_resolution
import l6_utm_canonical_input_transfer as input_transfer
import l6_utm_guest_network_ready as network_ready
import l6_utm_install_artifacts_staged_checkpoint_bindings as checkpoint_bindings
import l6_utm_install_artifacts_staged_checkpoint_evidence as checkpoint_evidence
import l6_utm_launch_transport_bindings as transport_bindings
import l6_utm_launch_transport_v2 as launch_transport
import l6_utm_start_once as start_control
import l6_v4_canonical_input_install as guest_installer
import l6_v4_install_artifacts_staged_checkpoint_driver as checkpoint_driver


EVIDENCE_FORMAT = checkpoint_bindings.EVIDENCE_FORMAT
CONTROL_RELATIVE_PATH = checkpoint_bindings.CONTROL_RELATIVE_PATH
BINDINGS_RELATIVE_PATH = checkpoint_bindings.BINDINGS_RELATIVE_PATH
DRIVER_RELATIVE_PATH = checkpoint_bindings.DRIVER_RELATIVE_PATH
EVIDENCE_RELATIVE_PATH = checkpoint_bindings.EVIDENCE_RELATIVE_PATH
REQUIRED_PRIOR_NETWORK_MANIFEST_SHA256 = (
    input_transfer.REQUIRED_PRIOR_NETWORK_MANIFEST_SHA256
)
REQUIRED_PRIOR_TRANSFER_MANIFEST_SHA256 = (
    input_resolution.REQUIRED_PRIOR_TRANSFER_MANIFEST_SHA256
)
REQUIRED_PRIOR_RESOLUTION_MANIFEST_SHA256 = (
    prior_preflight.REQUIRED_PRIOR_RESOLUTION_MANIFEST_SHA256
)
REQUIRED_PRIOR_PREFLIGHT_MANIFEST_SHA256 = (
    checkpoint_bindings.REQUIRED_PRIOR_PREFLIGHT_MANIFEST_SHA256
)
REQUIRED_TRANSFER_ATTEMPT_ID = input_resolution.REQUIRED_TRANSFER_ATTEMPT_ID
REQUIRED_RESOLUTION_ATTEMPT_ID = prior_preflight.REQUIRED_RESOLUTION_ATTEMPT_ID
REQUIRED_PREFLIGHT_ATTEMPT_ID = checkpoint_bindings.REQUIRED_PREFLIGHT_ATTEMPT_ID
REQUIRED_CHECKPOINT_ATTEMPT_ID = (
    checkpoint_bindings.REQUIRED_CHECKPOINT_ATTEMPT_ID
)
REQUIRED_SOURCE_BUNDLE_SIZE = input_resolution.REQUIRED_SOURCE_BUNDLE_SIZE
REQUIRED_SOURCE_BUNDLE_SHA256 = input_resolution.REQUIRED_SOURCE_BUNDLE_SHA256
REQUIRED_TARGET_UUID = input_resolution.REQUIRED_TARGET_UUID
REQUIRED_TARGET_NAME = input_resolution.REQUIRED_TARGET_NAME
PROCESS_COMMAND = launch_transport.PROCESS_COMMAND
EXIT_CHECKPOINT_PREPARED = 0
EXIT_CHECKPOINT_REJECTED = 10
EXIT_PRECONDITION_REJECTED = 11
EXIT_STATE_INDETERMINATE = 12
HEX_40 = re.compile(r"[0-9a-f]{40}")
HEX_64 = re.compile(r"[0-9a-f]{64}")
SAFE_ATTEMPT_ID = re.compile(r"[a-z0-9][a-z0-9-]{0,95}")


class InstallArtifactsStagedCheckpointError(ValueError):
    pass


@dataclass(frozen=True)
class InstallArtifactsStagedCheckpointRequest:
    repository_root: Path
    expected_repository_head: str
    prior_network_root: Path
    prior_network_manifest_sha256: str
    prior_transfer_root: Path
    prior_transfer_manifest_sha256: str
    prior_resolution_root: Path
    prior_resolution_manifest_sha256: str
    prior_preflight_root: Path
    prior_preflight_manifest_sha256: str
    source_bundle_path: Path
    source_bundle_size: int
    source_bundle_sha256: str
    output_root: Path
    transfer_attempt_id: str
    resolution_attempt_id: str
    preflight_attempt_id: str
    checkpoint_attempt_id: str
    target_uuid: str
    target_name: str
    target_package_path: Path
    command_timeout_seconds: int
    checkpoint_timeout_seconds: int
    evidence_settle_seconds: int
    authorized_install_artifacts_staged_checkpoint: bool
    authorized_generate_one_operation_id: bool
    authorized_one_acceptance_checkpoint_and_process_group_termination: bool
    authorized_no_resume_retry_cleanup_stop_or_quit: bool

    @property
    def guest_checkpoint_root(self) -> str:
        return (
            "/var/tmp/radishlex-l6-v4-install-artifacts-staged-checkpoint-"
            f"{self.checkpoint_attempt_id}"
        )

    @property
    def guest_driver_incoming(self) -> str:
        return f"{self.guest_checkpoint_root}/checkpoint-driver.incoming.py"

    @property
    def guest_driver_path(self) -> str:
        return f"{self.guest_checkpoint_root}/checkpoint-driver.py"

    @property
    def guest_marker_path(self) -> str:
        return f"{self.guest_checkpoint_root}/attempt.marker.json"

    @property
    def guest_phase_path(self) -> str:
        return f"{self.guest_checkpoint_root}/phase.json"

    @property
    def guest_terminal_path(self) -> str:
        return f"{self.guest_checkpoint_root}/checkpoint.evidence.json"

    def validate(self) -> None:
        paths = (
            (self.repository_root, "repository-root"),
            (self.prior_network_root, "prior-network-root"),
            (self.prior_transfer_root, "prior-transfer-root"),
            (self.prior_resolution_root, "prior-resolution-root"),
            (self.prior_preflight_root, "prior-preflight-root"),
            (self.source_bundle_path, "source-bundle-path"),
            (self.output_root, "output-root"),
            (self.target_package_path, "target-package-path"),
        )
        for path, label in paths:
            if not path.is_absolute() or ".." in path.parts:
                raise InstallArtifactsStagedCheckpointError(
                    f"{label}-must-be-absolute-normalized"
                )
        for protected, label in paths[:-2] + ((self.target_package_path, "target"),):
            if _paths_overlap(self.output_root, protected):
                raise InstallArtifactsStagedCheckpointError(
                    f"output-root-must-not-overlap-{label}"
                )
        if not HEX_40.fullmatch(self.expected_repository_head):
            raise InstallArtifactsStagedCheckpointError(
                "expected-repository-head-invalid"
            )
        fixed = (
            (
                self.prior_network_manifest_sha256,
                REQUIRED_PRIOR_NETWORK_MANIFEST_SHA256,
                "prior-network-manifest",
            ),
            (
                self.prior_transfer_manifest_sha256,
                REQUIRED_PRIOR_TRANSFER_MANIFEST_SHA256,
                "prior-transfer-manifest",
            ),
            (
                self.prior_resolution_manifest_sha256,
                REQUIRED_PRIOR_RESOLUTION_MANIFEST_SHA256,
                "prior-resolution-manifest",
            ),
            (
                self.prior_preflight_manifest_sha256,
                REQUIRED_PRIOR_PREFLIGHT_MANIFEST_SHA256,
                "prior-preflight-manifest",
            ),
            (
                self.source_bundle_sha256,
                REQUIRED_SOURCE_BUNDLE_SHA256,
                "source-bundle-sha256",
            ),
        )
        for actual, expected, label in fixed:
            if actual != expected:
                raise InstallArtifactsStagedCheckpointError(
                    f"required-{label}-mismatch"
                )
        if self.source_bundle_size != REQUIRED_SOURCE_BUNDLE_SIZE:
            raise InstallArtifactsStagedCheckpointError(
                "required-source-bundle-size-mismatch"
            )
        attempts = (
            (self.transfer_attempt_id, REQUIRED_TRANSFER_ATTEMPT_ID, "transfer"),
            (
                self.resolution_attempt_id,
                REQUIRED_RESOLUTION_ATTEMPT_ID,
                "resolution",
            ),
            (
                self.preflight_attempt_id,
                REQUIRED_PREFLIGHT_ATTEMPT_ID,
                "preflight",
            ),
            (
                self.checkpoint_attempt_id,
                REQUIRED_CHECKPOINT_ATTEMPT_ID,
                "checkpoint",
            ),
        )
        for actual, expected, label in attempts:
            if actual != expected or not SAFE_ATTEMPT_ID.fullmatch(actual):
                raise InstallArtifactsStagedCheckpointError(
                    f"required-{label}-attempt-id-mismatch"
                )
        if len({item[0] for item in attempts}) != len(attempts):
            raise InstallArtifactsStagedCheckpointError("attempt-id-overlap")
        try:
            canonical_uuid = str(uuid.UUID(self.target_uuid)).upper()
        except ValueError as exc:
            raise InstallArtifactsStagedCheckpointError(
                "target-uuid-invalid"
            ) from exc
        if canonical_uuid != self.target_uuid or self.target_uuid != REQUIRED_TARGET_UUID:
            raise InstallArtifactsStagedCheckpointError(
                "required-target-uuid-mismatch"
            )
        if self.target_name != REQUIRED_TARGET_NAME:
            raise InstallArtifactsStagedCheckpointError(
                "required-target-name-mismatch"
            )
        if self.target_package_path != transport_bindings.expected_target_package_path(
            self.target_name
        ):
            raise InstallArtifactsStagedCheckpointError(
                "target-package-path-mismatch"
            )
        for value, low, high, label in (
            (self.command_timeout_seconds, 1, 60, "command-timeout"),
            (self.checkpoint_timeout_seconds, 1, 30 * 60, "checkpoint-timeout"),
            (self.evidence_settle_seconds, 1, 60, "evidence-settle"),
        ):
            if not low <= value <= high:
                raise InstallArtifactsStagedCheckpointError(
                    f"{label}-out-of-range"
                )
        authorizations = (
            (
                self.authorized_install_artifacts_staged_checkpoint,
                "authorized-install-artifacts-staged-checkpoint-required",
            ),
            (
                self.authorized_generate_one_operation_id,
                "authorized-generate-one-operation-id-required",
            ),
            (
                self.authorized_one_acceptance_checkpoint_and_process_group_termination,
                "authorized-one-acceptance-checkpoint-required",
            ),
            (
                self.authorized_no_resume_retry_cleanup_stop_or_quit,
                "authorized-no-resume-retry-cleanup-stop-or-quit-required",
            ),
        )
        for authorized, reason in authorizations:
            if not authorized:
                raise InstallArtifactsStagedCheckpointError(reason)

    def as_json(self) -> dict[str, object]:
        return {
            "authorization": {
                "generate_one_operation_id": True,
                "install_artifacts_staged_checkpoint": True,
                "no_resume_retry_cleanup_stop_or_quit": True,
                "one_acceptance_checkpoint_and_process_group_termination": True,
            },
            "checkpoint_attempt_id": self.checkpoint_attempt_id,
            "checkpoint_timeout_seconds": self.checkpoint_timeout_seconds,
            "command_timeout_seconds": self.command_timeout_seconds,
            "evidence_settle_seconds": self.evidence_settle_seconds,
            "expected_repository_head": self.expected_repository_head,
            "format": EVIDENCE_FORMAT,
            "guest_checkpoint_root": self.guest_checkpoint_root,
            "guest_final_input_root": str(guest_installer.FINAL_INPUT_ROOT),
            "preflight_attempt_id": self.preflight_attempt_id,
            "prior_network_manifest_sha256": self.prior_network_manifest_sha256,
            "prior_preflight_manifest_sha256": self.prior_preflight_manifest_sha256,
            "prior_resolution_manifest_sha256": self.prior_resolution_manifest_sha256,
            "prior_transfer_manifest_sha256": self.prior_transfer_manifest_sha256,
            "resolution_attempt_id": self.resolution_attempt_id,
            "source_bundle_path_sha256": _sha256_text(str(self.source_bundle_path)),
            "source_bundle_sha256": self.source_bundle_sha256,
            "source_bundle_size": self.source_bundle_size,
            "target_name": self.target_name,
            "target_package_path_sha256": _sha256_text(
                str(self.target_package_path)
            ),
            "target_uuid": self.target_uuid,
            "transfer_attempt_id": self.transfer_attempt_id,
        }


@dataclass(frozen=True)
class InstallArtifactsStagedCheckpointResult:
    outcome: str
    exit_code: int
    evidence_root: Path
    manifest_sha256: str
    checkpoint_invocations: int


class CommandRunner(Protocol):
    def run(
        self,
        argv: tuple[str, ...],
        timeout_seconds: int,
        *,
        stdin_file=None,
    ) -> start_control.CommandObservation: ...


Sleeper = Callable[[float], None]


def run_install_artifacts_staged_checkpoint(
    request: InstallArtifactsStagedCheckpointRequest,
    *,
    runner: CommandRunner | None = None,
    binding_validator=None,
    source_opener=None,
    source_revalidator=None,
    target_validator=None,
    sleeper: Sleeper = time.sleep,
) -> InstallArtifactsStagedCheckpointResult:
    request.validate()
    writer = start_control.EvidenceWriter.create(request.output_root)
    writer.write_json("request.json", request.as_json())
    command_runner = runner or input_transfer.SubprocessCommandRunner()
    validate_bindings = binding_validator or validate_checkpoint_bindings
    open_source = source_opener or input_transfer.open_source_bundle
    revalidate_source = source_revalidator or input_transfer.revalidate_open_source_bundle
    validate_target = target_validator or network_ready.validate_target_files

    stage = "binding-preflight"
    reason = "not-run"
    outcome = "precondition-rejected"
    exit_code = EXIT_PRECONDITION_REJECTED
    guest_observation_started = False
    file_pull_invocations = 0
    file_push_invocations = 0
    guest_exec_invocations = 0
    checkpoint_invocations = 0
    operation_id = "not-generated"
    operation_id_sha256: str | None = None
    guest_terminal: dict[str, object] | None = None
    source_bundle: input_transfer.SourceBundle | None = None

    def pull(
        path: str,
        name: str,
        *,
        exact: bytes | None = None,
        allow_empty: bool = False,
    ) -> bytes:
        nonlocal file_pull_invocations, stage
        stage = name
        file_pull_invocations += 1
        observation = command_runner.run(
            ("utmctl", "file", "pull", request.target_uuid, path),
            request.command_timeout_seconds,
        )
        writer.write_json(f"{name}.json", observation.as_json())
        if exact is not None:
            input_transfer._require_exact_small_readback(observation, exact, name)
        else:
            input_transfer._require_successful_small_observation(observation, name)
            if observation.stdout.total_bytes == 0 and not allow_empty:
                raise InstallArtifactsStagedCheckpointError(f"{name}-empty")
        return observation.stdout.prefix

    try:
        binding = validate_bindings(request)
        writer.write_json("binding-preflight.json", binding.evidence)

        stage = "source-bundle-preflight"
        source_bundle = open_source(request)
        writer.write_json("source-bundle-preflight.json", source_bundle.as_json())

        stage = "target-files-preflight"
        writer.write_json("target-files-preflight.json", validate_target(request))

        stage = "host-process-preflight"
        process_observation = command_runner.run(
            PROCESS_COMMAND, request.command_timeout_seconds
        )
        start_control._require_successful_observation(
            process_observation, "host-process-preflight"
        )
        processes = launch_transport.parse_relevant_processes(process_observation)
        if any(item["role"] == "utmctl" for item in processes):
            raise InstallArtifactsStagedCheckpointError(
                "utmctl-process-active-before-checkpoint"
            )
        writer.write_json(
            "host-process-preflight.json",
            {
                "format": EVIDENCE_FORMAT,
                "observation": network_ready._observation_metadata(
                    process_observation
                ),
                "relevant_process_count": len(processes),
            },
        )

        stage = "target-handles-preflight"
        handles_observation = command_runner.run(
            network_ready._lsof_argv(request), request.command_timeout_seconds
        )
        handles = network_ready.parse_target_handles(handles_observation, request)
        writer.write_json(
            "target-handles-preflight.json",
            {
                "format": EVIDENCE_FORMAT,
                "observation": network_ready._observation_metadata(
                    handles_observation
                ),
                **handles,
            },
        )

        guest_observation_started = True
        for index in (1, 2):
            pull(
                binding.network.guest_evidence_path,
                f"network-evidence-live-readback-{index}",
                exact=binding.network.guest_evidence_bytes,
            )
        preflight_readbacks = [
            pull(
                binding.preflight_evidence_path,
                f"negative-preflight-evidence-live-readback-{index}",
                exact=binding.preflight_evidence_bytes,
            )
            for index in (1, 2)
        ]
        checkpoint_evidence.require_equal_payloads(
            preflight_readbacks, "negative-preflight-live"
        )

        stage = "guest-checkpoint-root-create"
        guest_exec_invocations += 1
        root_observation = command_runner.run(
            (
                "utmctl",
                "exec",
                request.target_uuid,
                "--cmd",
                "/bin/mkdir",
                "-m",
                "0700",
                request.guest_checkpoint_root,
            ),
            request.command_timeout_seconds,
        )
        writer.write_json(f"{stage}.json", root_observation.as_json())
        input_transfer._require_successful_small_observation(root_observation, stage)

        stage = "guest-checkpoint-driver-push"
        file_push_invocations += 1
        with (request.repository_root / DRIVER_RELATIVE_PATH).open("rb") as source:
            push_observation = command_runner.run(
                (
                    "utmctl",
                    "file",
                    "push",
                    request.target_uuid,
                    request.guest_driver_incoming,
                ),
                request.command_timeout_seconds,
                stdin_file=source,
            )
        writer.write_json(f"{stage}.json", push_observation.as_json())
        input_transfer._require_successful_small_observation(push_observation, stage)

        for stage, command in (
            (
                "guest-checkpoint-driver-chown",
                ("/bin/chown", "root:root", request.guest_driver_incoming),
            ),
            (
                "guest-checkpoint-driver-chmod",
                ("/bin/chmod", "0600", request.guest_driver_incoming),
            ),
            (
                "guest-checkpoint-driver-publish",
                (
                    "/bin/mv",
                    "--",
                    request.guest_driver_incoming,
                    request.guest_driver_path,
                ),
            ),
        ):
            guest_exec_invocations += 1
            observation = command_runner.run(
                ("utmctl", "exec", request.target_uuid, "--cmd", *command),
                request.command_timeout_seconds,
            )
            writer.write_json(f"{stage}.json", observation.as_json())
            input_transfer._require_successful_small_observation(observation, stage)

        pull(
            request.guest_driver_path,
            "guest-checkpoint-driver-readback",
            exact=binding.driver_bytes,
        )

        stage = "guest-install-artifacts-staged-checkpoint"
        guest_exec_invocations += 1
        checkpoint_invocations = 1
        driver_observation = command_runner.run(
            driver_argv(request, binding), request.checkpoint_timeout_seconds
        )
        writer.write_json(f"{stage}.json", driver_observation.as_json())
        if driver_observation.timed_out:
            raise InstallArtifactsStagedCheckpointError(
                "guest-checkpoint-driver-timed-out"
            )
        if driver_observation.stdout.truncated or driver_observation.stderr.truncated:
            raise InstallArtifactsStagedCheckpointError(
                "guest-checkpoint-driver-output-truncated"
            )
        if driver_observation.stdout.total_bytes or driver_observation.stderr.total_bytes:
            raise InstallArtifactsStagedCheckpointError(
                "guest-checkpoint-driver-output-not-empty"
            )

        sleeper(float(request.evidence_settle_seconds))
        driver_sha256 = hashlib.sha256(binding.driver_bytes).hexdigest()
        expected_marker = checkpoint_driver.canonical_json(
            {
                "checkpoint_attempt_id": request.checkpoint_attempt_id,
                "driver_sha256": driver_sha256,
                "format": checkpoint_driver.MARKER_FORMAT,
            }
        )
        pull(
            request.guest_marker_path,
            "guest-checkpoint-marker-readback",
            exact=expected_marker,
        )
        terminal_payloads = [
            pull(
                request.guest_terminal_path,
                f"guest-checkpoint-evidence-readback-{index}",
            )
            for index in (1, 2)
        ]
        terminal_payload = checkpoint_evidence.require_equal_payloads(
            terminal_payloads, "guest-checkpoint-evidence"
        )
        guest_terminal = checkpoint_evidence.parse_driver_terminal(
            terminal_payload, request, binding
        )
        writer.write_json("guest-checkpoint-evidence.json", guest_terminal)
        pull(
            request.guest_phase_path,
            "guest-checkpoint-phase-readback",
            exact=checkpoint_driver.canonical_json(
                {
                    "format": checkpoint_driver.EVIDENCE_FORMAT,
                    "phase": guest_terminal["phase"],
                }
            ),
        )

        expected_driver_exit = {
            "checkpoint-prepared": 0,
            "checkpoint-rejected": 10,
            "state-indeterminate": 12,
        }[str(guest_terminal["outcome"])]
        if driver_observation.exit_code != expected_driver_exit:
            raise InstallArtifactsStagedCheckpointError(
                "guest-checkpoint-driver-exit-evidence-mismatch"
            )
        operation_id = str(guest_terminal["operation_id"])
        candidate_hash = guest_terminal.get("operation_id_sha256")
        if isinstance(candidate_hash, str) and HEX_64.fullmatch(candidate_hash):
            operation_id_sha256 = candidate_hash

        if guest_terminal["outcome"] == "checkpoint-prepared":
            artifacts = checkpoint_evidence.pull_checkpoint_artifacts(
                request,
                guest_terminal,
                pull,
            )
            checkpoint_evidence.validate_checkpoint_artifacts(
                artifacts, guest_terminal, binding
            )
            outcome = "checkpoint-prepared"
            exit_code = EXIT_CHECKPOINT_PREPARED
            reason = "one-shot-install-artifacts-staged-checkpoint-passed"
        elif guest_terminal["outcome"] == "checkpoint-rejected":
            outcome = "checkpoint-rejected"
            exit_code = EXIT_CHECKPOINT_REJECTED
            reason = f"guest-checkpoint-rejected:{guest_terminal['reason']}"
        else:
            outcome = "state-indeterminate"
            exit_code = EXIT_STATE_INDETERMINATE
            reason = f"guest-checkpoint-state-indeterminate:{guest_terminal['reason']}"

        pull(
            binding.network.guest_evidence_path,
            "network-evidence-postflight-readback",
            exact=binding.network.guest_evidence_bytes,
        )
    except (
        InstallArtifactsStagedCheckpointError,
        input_transfer.CanonicalInputTransferError,
        network_ready.NetworkReadyError,
        start_control.StartControlError,
        transport_bindings.BindingError,
        OSError,
        tarfile.TarError,
        ValueError,
    ) as exc:
        reason = f"{stage}:{exc}"
        if guest_observation_started:
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
                tarfile.TarError,
            ) as exc:
                reason = f"source-bundle-postflight:{exc}"
                outcome = (
                    "state-indeterminate"
                    if guest_observation_started
                    else "precondition-rejected"
                )
                exit_code = (
                    EXIT_STATE_INDETERMINATE
                    if guest_observation_started
                    else EXIT_PRECONDITION_REJECTED
                )
            source_bundle.file_object.close()

    terminal = {
        "acceptance_invocations": (
            guest_terminal.get("acceptance_invocations") if guest_terminal else 0
        ),
        "automatic_cleanup": "not-performed",
        "automatic_quit": "not-performed",
        "automatic_retry": "not-performed",
        "automatic_stop": "not-performed",
        "checkpoint_attempt_id": request.checkpoint_attempt_id,
        "checkpoint_invocations": checkpoint_invocations,
        "file_pull_invocations": file_pull_invocations,
        "file_push_invocations": file_push_invocations,
        "format": EVIDENCE_FORMAT,
        "guest_checkpoint_outcome": (
            guest_terminal.get("outcome") if guest_terminal else None
        ),
        "guest_exec_invocations": guest_exec_invocations,
        "maintenance_resume_invocations": 0,
        "operation_id": operation_id,
        "operation_id_sha256": operation_id_sha256,
        "outcome": outcome,
        "plain_utmctl_list": "not-performed",
        "plain_utmctl_start": "not-performed",
        "plain_utmctl_status": "not-performed",
        "preflight_attempt_id": request.preflight_attempt_id,
        "reason": reason,
        "source_bundle_sha256": request.source_bundle_sha256,
        "source_bundle_size": request.source_bundle_size,
        "target_name": request.target_name,
        "target_uuid": request.target_uuid,
        "transaction": (
            guest_terminal.get("transaction")
            if guest_terminal
            else "not-performed"
        ),
    }
    writer.write_json("terminal.json", terminal)
    manifest_sha256 = writer.write_manifest()
    return InstallArtifactsStagedCheckpointResult(
        outcome=outcome,
        exit_code=exit_code,
        evidence_root=request.output_root,
        manifest_sha256=manifest_sha256,
        checkpoint_invocations=checkpoint_invocations,
    )


def validate_checkpoint_bindings(
    request: InstallArtifactsStagedCheckpointRequest,
) -> checkpoint_bindings.CheckpointBinding:
    if Path(__file__).absolute() != request.repository_root / CONTROL_RELATIVE_PATH:
        raise InstallArtifactsStagedCheckpointError("executed-control-path-mismatch")
    for module_path, expected, label in (
        (
            Path(checkpoint_bindings.__file__).absolute(),
            BINDINGS_RELATIVE_PATH,
            "bindings",
        ),
        (
            Path(checkpoint_evidence.__file__).absolute(),
            EVIDENCE_RELATIVE_PATH,
            "evidence",
        ),
        (Path(checkpoint_driver.__file__).absolute(), DRIVER_RELATIVE_PATH, "driver"),
    ):
        if module_path != request.repository_root / expected:
            raise InstallArtifactsStagedCheckpointError(
                f"executed-{label}-path-mismatch"
            )
    try:
        return checkpoint_bindings.validate_checkpoint_bindings(request)
    except ValueError as exc:
        raise InstallArtifactsStagedCheckpointError(str(exc)) from exc


def driver_argv(
    request: InstallArtifactsStagedCheckpointRequest,
    binding: checkpoint_bindings.CheckpointBinding,
) -> tuple[str, ...]:
    return (
        "utmctl",
        "exec",
        request.target_uuid,
        "--cmd",
        "/usr/bin/python3",
        "-B",
        request.guest_driver_path,
        "--checkpoint-attempt-id",
        request.checkpoint_attempt_id,
        "--preflight-attempt-id",
        request.preflight_attempt_id,
        "--target-uuid",
        request.target_uuid,
        "--expected-boot-id-sha256",
        binding.boot_id_sha256,
        "--expected-negative-preflight-sha256",
        hashlib.sha256(binding.preflight_evidence_bytes).hexdigest(),
        "--expected-driver-sha256",
        hashlib.sha256(binding.driver_bytes).hexdigest(),
        "--control-root",
        request.guest_checkpoint_root,
        "--negative-preflight-evidence",
        binding.preflight_evidence_path,
    )


def _sha256_text(value: str) -> str:
    return hashlib.sha256(value.encode("utf-8")).hexdigest()


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
            "Run one create-new install_artifacts_staged checkpoint with one "
            "operation ID and no resume, retry, cleanup, stop, or quit."
        )
    )
    parser.add_argument("command", choices=("checkpoint",))
    parser.add_argument("--repository-root", type=Path, required=True)
    parser.add_argument("--expected-repository-head", required=True)
    parser.add_argument("--prior-network-root", type=Path, required=True)
    parser.add_argument("--prior-network-manifest-sha256", required=True)
    parser.add_argument("--prior-transfer-root", type=Path, required=True)
    parser.add_argument("--prior-transfer-manifest-sha256", required=True)
    parser.add_argument("--prior-resolution-root", type=Path, required=True)
    parser.add_argument("--prior-resolution-manifest-sha256", required=True)
    parser.add_argument("--prior-preflight-root", type=Path, required=True)
    parser.add_argument("--prior-preflight-manifest-sha256", required=True)
    parser.add_argument("--source-bundle-path", type=Path, required=True)
    parser.add_argument("--source-bundle-size", type=int, required=True)
    parser.add_argument("--source-bundle-sha256", required=True)
    parser.add_argument("--output-root", type=Path, required=True)
    parser.add_argument("--transfer-attempt-id", required=True)
    parser.add_argument("--resolution-attempt-id", required=True)
    parser.add_argument("--preflight-attempt-id", required=True)
    parser.add_argument("--checkpoint-attempt-id", required=True)
    parser.add_argument("--target-uuid", required=True)
    parser.add_argument("--target-name", required=True)
    parser.add_argument("--target-package-path", type=Path, required=True)
    parser.add_argument("--command-timeout-seconds", type=int, default=60)
    parser.add_argument("--checkpoint-timeout-seconds", type=int, default=1500)
    parser.add_argument("--evidence-settle-seconds", type=int, default=10)
    parser.add_argument(
        "--authorized-install-artifacts-staged-checkpoint", action="store_true"
    )
    parser.add_argument(
        "--authorized-generate-one-operation-id", action="store_true"
    )
    parser.add_argument(
        "--authorized-one-acceptance-checkpoint-and-process-group-termination",
        action="store_true",
    )
    parser.add_argument(
        "--authorized-no-resume-retry-cleanup-stop-or-quit", action="store_true"
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    request = InstallArtifactsStagedCheckpointRequest(
        repository_root=args.repository_root,
        expected_repository_head=args.expected_repository_head,
        prior_network_root=args.prior_network_root,
        prior_network_manifest_sha256=args.prior_network_manifest_sha256,
        prior_transfer_root=args.prior_transfer_root,
        prior_transfer_manifest_sha256=args.prior_transfer_manifest_sha256,
        prior_resolution_root=args.prior_resolution_root,
        prior_resolution_manifest_sha256=args.prior_resolution_manifest_sha256,
        prior_preflight_root=args.prior_preflight_root,
        prior_preflight_manifest_sha256=args.prior_preflight_manifest_sha256,
        source_bundle_path=args.source_bundle_path,
        source_bundle_size=args.source_bundle_size,
        source_bundle_sha256=args.source_bundle_sha256,
        output_root=args.output_root,
        transfer_attempt_id=args.transfer_attempt_id,
        resolution_attempt_id=args.resolution_attempt_id,
        preflight_attempt_id=args.preflight_attempt_id,
        checkpoint_attempt_id=args.checkpoint_attempt_id,
        target_uuid=args.target_uuid,
        target_name=args.target_name,
        target_package_path=args.target_package_path,
        command_timeout_seconds=args.command_timeout_seconds,
        checkpoint_timeout_seconds=args.checkpoint_timeout_seconds,
        evidence_settle_seconds=args.evidence_settle_seconds,
        authorized_install_artifacts_staged_checkpoint=(
            args.authorized_install_artifacts_staged_checkpoint
        ),
        authorized_generate_one_operation_id=args.authorized_generate_one_operation_id,
        authorized_one_acceptance_checkpoint_and_process_group_termination=(
            args.authorized_one_acceptance_checkpoint_and_process_group_termination
        ),
        authorized_no_resume_retry_cleanup_stop_or_quit=(
            args.authorized_no_resume_retry_cleanup_stop_or_quit
        ),
    )
    try:
        result = run_install_artifacts_staged_checkpoint(request)
    except (
        InstallArtifactsStagedCheckpointError,
        input_transfer.CanonicalInputTransferError,
        network_ready.NetworkReadyError,
        start_control.StartControlError,
        transport_bindings.BindingError,
        OSError,
        tarfile.TarError,
        ValueError,
    ) as exc:
        print(f"l6_utm_install_artifacts_staged_checkpoint_error={exc}", file=sys.stderr)
        return EXIT_PRECONDITION_REJECTED
    print(f"install_artifacts_staged_checkpoint_outcome={result.outcome}")
    print(f"evidence_root={result.evidence_root}")
    print(f"files_sha256={result.manifest_sha256}")
    return result.exit_code


if __name__ == "__main__":
    raise SystemExit(main())
