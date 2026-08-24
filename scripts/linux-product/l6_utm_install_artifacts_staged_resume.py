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

import l6_utm_canonical_input_transfer as input_transfer
import l6_utm_guest_network_ready as network_ready
import l6_utm_install_artifacts_staged_checkpoint as checkpoint_control
import l6_utm_install_artifacts_staged_resume_bindings as resume_bindings
import l6_utm_install_artifacts_staged_resume_evidence as resume_evidence
import l6_utm_launch_transport_bindings as transport_bindings
import l6_utm_launch_transport_v2 as launch_transport
import l6_utm_start_once as start_control
import l6_v4_canonical_input_install as guest_installer
import l6_v4_install_artifacts_staged_resume_driver as resume_driver


EVIDENCE_FORMAT = resume_bindings.EVIDENCE_FORMAT
CONTROL_RELATIVE_PATH = resume_bindings.CONTROL_RELATIVE_PATH
BINDINGS_RELATIVE_PATH = resume_bindings.BINDINGS_RELATIVE_PATH
DRIVER_RELATIVE_PATH = resume_bindings.DRIVER_RELATIVE_PATH
EVIDENCE_RELATIVE_PATH = resume_bindings.EVIDENCE_RELATIVE_PATH
REQUIRED_PRIOR_NETWORK_MANIFEST_SHA256 = (
    checkpoint_control.REQUIRED_PRIOR_NETWORK_MANIFEST_SHA256
)
REQUIRED_PRIOR_TRANSFER_MANIFEST_SHA256 = (
    checkpoint_control.REQUIRED_PRIOR_TRANSFER_MANIFEST_SHA256
)
REQUIRED_PRIOR_RESOLUTION_MANIFEST_SHA256 = (
    checkpoint_control.REQUIRED_PRIOR_RESOLUTION_MANIFEST_SHA256
)
REQUIRED_PRIOR_PREFLIGHT_MANIFEST_SHA256 = (
    checkpoint_control.REQUIRED_PRIOR_PREFLIGHT_MANIFEST_SHA256
)
REQUIRED_PRIOR_CHECKPOINT_MANIFEST_SHA256 = (
    resume_bindings.REQUIRED_PRIOR_CHECKPOINT_MANIFEST_SHA256
)
REQUIRED_TRANSFER_ATTEMPT_ID = checkpoint_control.REQUIRED_TRANSFER_ATTEMPT_ID
REQUIRED_RESOLUTION_ATTEMPT_ID = checkpoint_control.REQUIRED_RESOLUTION_ATTEMPT_ID
REQUIRED_PREFLIGHT_ATTEMPT_ID = checkpoint_control.REQUIRED_PREFLIGHT_ATTEMPT_ID
REQUIRED_CHECKPOINT_ATTEMPT_ID = (
    resume_bindings.REQUIRED_CHECKPOINT_ATTEMPT_ID
)
REQUIRED_RESUME_ATTEMPT_ID = resume_bindings.REQUIRED_RESUME_ATTEMPT_ID
REQUIRED_SOURCE_BUNDLE_SIZE = checkpoint_control.REQUIRED_SOURCE_BUNDLE_SIZE
REQUIRED_SOURCE_BUNDLE_SHA256 = checkpoint_control.REQUIRED_SOURCE_BUNDLE_SHA256
REQUIRED_TARGET_UUID = checkpoint_control.REQUIRED_TARGET_UUID
REQUIRED_TARGET_NAME = checkpoint_control.REQUIRED_TARGET_NAME
PROCESS_COMMAND = launch_transport.PROCESS_COMMAND
EXIT_RESUME_COMPLETED = 0
EXIT_RESUME_REJECTED = 10
EXIT_PRECONDITION_REJECTED = 11
EXIT_STATE_INDETERMINATE = 12
HEX_40 = re.compile(r"[0-9a-f]{40}")
SAFE_ATTEMPT_ID = re.compile(r"[a-z0-9][a-z0-9-]{0,95}")


class InstallArtifactsStagedResumeError(ValueError):
    pass


@dataclass(frozen=True)
class InstallArtifactsStagedResumeRequest:
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
    prior_checkpoint_root: Path
    prior_checkpoint_manifest_sha256: str
    source_bundle_path: Path
    source_bundle_size: int
    source_bundle_sha256: str
    output_root: Path
    transfer_attempt_id: str
    resolution_attempt_id: str
    preflight_attempt_id: str
    checkpoint_attempt_id: str
    resume_attempt_id: str
    target_uuid: str
    target_name: str
    target_package_path: Path
    command_timeout_seconds: int
    resume_timeout_seconds: int
    evidence_settle_seconds: int
    authorized_install_artifacts_staged_exact_resume: bool
    authorized_existing_operation_secret_hash_only: bool
    authorized_one_resume_and_one_postflight: bool
    authorized_no_retry_cleanup_stop_quit_or_next_checkpoint: bool

    @property
    def guest_resume_root(self) -> str:
        return (
            "/var/tmp/radishlex-l6-v4-install-artifacts-staged-resume-"
            f"{self.resume_attempt_id}"
        )

    @property
    def guest_driver_incoming(self) -> str:
        return f"{self.guest_resume_root}/resume-driver.incoming.py"

    @property
    def guest_driver_path(self) -> str:
        return f"{self.guest_resume_root}/resume-driver.py"

    @property
    def guest_marker_path(self) -> str:
        return f"{self.guest_resume_root}/attempt.marker.json"

    @property
    def guest_phase_path(self) -> str:
        return f"{self.guest_resume_root}/phase.json"

    @property
    def guest_terminal_path(self) -> str:
        return f"{self.guest_resume_root}/resume.evidence.json"

    def validate(self) -> None:
        paths = (
            (self.repository_root, "repository-root"),
            (self.prior_network_root, "prior-network-root"),
            (self.prior_transfer_root, "prior-transfer-root"),
            (self.prior_resolution_root, "prior-resolution-root"),
            (self.prior_preflight_root, "prior-preflight-root"),
            (self.prior_checkpoint_root, "prior-checkpoint-root"),
            (self.source_bundle_path, "source-bundle-path"),
            (self.output_root, "output-root"),
            (self.target_package_path, "target-package-path"),
        )
        for path, label in paths:
            if not path.is_absolute() or ".." in path.parts:
                raise InstallArtifactsStagedResumeError(
                    f"{label}-must-be-absolute-normalized"
                )
        protected = paths[:-2] + ((self.target_package_path, "target"),)
        for path, label in protected:
            if _paths_overlap(self.output_root, path):
                raise InstallArtifactsStagedResumeError(
                    f"output-root-must-not-overlap-{label}"
                )
        if not HEX_40.fullmatch(self.expected_repository_head):
            raise InstallArtifactsStagedResumeError(
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
                self.prior_checkpoint_manifest_sha256,
                REQUIRED_PRIOR_CHECKPOINT_MANIFEST_SHA256,
                "prior-checkpoint-manifest",
            ),
            (
                self.source_bundle_sha256,
                REQUIRED_SOURCE_BUNDLE_SHA256,
                "source-bundle-sha256",
            ),
        )
        for actual, expected, label in fixed:
            if actual != expected:
                raise InstallArtifactsStagedResumeError(
                    f"required-{label}-mismatch"
                )
        if self.source_bundle_size != REQUIRED_SOURCE_BUNDLE_SIZE:
            raise InstallArtifactsStagedResumeError(
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
            (self.resume_attempt_id, REQUIRED_RESUME_ATTEMPT_ID, "resume"),
        )
        for actual, expected, label in attempts:
            if actual != expected or not SAFE_ATTEMPT_ID.fullmatch(actual):
                raise InstallArtifactsStagedResumeError(
                    f"required-{label}-attempt-id-mismatch"
                )
        if len({attempt[0] for attempt in attempts}) != len(attempts):
            raise InstallArtifactsStagedResumeError("attempt-id-overlap")
        try:
            canonical_uuid = str(uuid.UUID(self.target_uuid)).upper()
        except ValueError as exc:
            raise InstallArtifactsStagedResumeError("target-uuid-invalid") from exc
        if canonical_uuid != self.target_uuid or self.target_uuid != REQUIRED_TARGET_UUID:
            raise InstallArtifactsStagedResumeError(
                "required-target-uuid-mismatch"
            )
        if self.target_name != REQUIRED_TARGET_NAME:
            raise InstallArtifactsStagedResumeError(
                "required-target-name-mismatch"
            )
        if self.target_package_path != transport_bindings.expected_target_package_path(
            self.target_name
        ):
            raise InstallArtifactsStagedResumeError(
                "target-package-path-mismatch"
            )
        for value, low, high, label in (
            (self.command_timeout_seconds, 1, 60, "command-timeout"),
            (self.resume_timeout_seconds, 1, 30 * 60, "resume-timeout"),
            (self.evidence_settle_seconds, 1, 60, "evidence-settle"),
        ):
            if not low <= value <= high:
                raise InstallArtifactsStagedResumeError(f"{label}-out-of-range")
        authorizations = (
            (
                self.authorized_install_artifacts_staged_exact_resume,
                "authorized-install-artifacts-staged-exact-resume-required",
            ),
            (
                self.authorized_existing_operation_secret_hash_only,
                "authorized-existing-operation-secret-hash-only-required",
            ),
            (
                self.authorized_one_resume_and_one_postflight,
                "authorized-one-resume-and-one-postflight-required",
            ),
            (
                self.authorized_no_retry_cleanup_stop_quit_or_next_checkpoint,
                "authorized-no-retry-cleanup-stop-quit-or-next-checkpoint-required",
            ),
        )
        for authorized, reason in authorizations:
            if not authorized:
                raise InstallArtifactsStagedResumeError(reason)

    def as_json(self) -> dict[str, object]:
        return {
            "authorization": {
                "existing_operation_secret_hash_only": True,
                "install_artifacts_staged_exact_resume": True,
                "no_retry_cleanup_stop_quit_or_next_checkpoint": True,
                "one_resume_and_one_postflight": True,
            },
            "checkpoint_attempt_id": self.checkpoint_attempt_id,
            "command_timeout_seconds": self.command_timeout_seconds,
            "evidence_settle_seconds": self.evidence_settle_seconds,
            "expected_repository_head": self.expected_repository_head,
            "format": EVIDENCE_FORMAT,
            "guest_resume_root": self.guest_resume_root,
            "prior_checkpoint_manifest_sha256": (
                self.prior_checkpoint_manifest_sha256
            ),
            "prior_network_manifest_sha256": self.prior_network_manifest_sha256,
            "prior_preflight_manifest_sha256": self.prior_preflight_manifest_sha256,
            "prior_resolution_manifest_sha256": (
                self.prior_resolution_manifest_sha256
            ),
            "prior_transfer_manifest_sha256": self.prior_transfer_manifest_sha256,
            "resume_attempt_id": self.resume_attempt_id,
            "resume_timeout_seconds": self.resume_timeout_seconds,
            "source_bundle_path_sha256": _sha256_text(str(self.source_bundle_path)),
            "source_bundle_sha256": self.source_bundle_sha256,
            "source_bundle_size": self.source_bundle_size,
            "target_name": self.target_name,
            "target_package_path_sha256": _sha256_text(
                str(self.target_package_path)
            ),
            "target_uuid": self.target_uuid,
        }


@dataclass(frozen=True)
class InstallArtifactsStagedResumeResult:
    outcome: str
    exit_code: int
    evidence_root: Path
    manifest_sha256: str
    resume_invocations: int


class CommandRunner(Protocol):
    def run(
        self,
        argv: tuple[str, ...],
        timeout_seconds: int,
        *,
        stdin_file=None,
    ) -> start_control.CommandObservation: ...


Sleeper = Callable[[float], None]


def run_install_artifacts_staged_resume(
    request: InstallArtifactsStagedResumeRequest,
    *,
    runner: CommandRunner | None = None,
    binding_validator=None,
    source_opener=None,
    source_revalidator=None,
    target_validator=None,
    sleeper: Sleeper = time.sleep,
) -> InstallArtifactsStagedResumeResult:
    request.validate()
    writer = start_control.EvidenceWriter.create(request.output_root)
    writer.write_json("request.json", request.as_json())
    command_runner = runner or input_transfer.SubprocessCommandRunner()
    validate_bindings = binding_validator or validate_resume_bindings
    open_source = source_opener or input_transfer.open_source_bundle
    revalidate_source = source_revalidator or input_transfer.revalidate_open_source_bundle
    validate_target = target_validator or network_ready.validate_target_files

    stage = "binding-preflight"
    reason = "not-run"
    outcome = "precondition-rejected"
    exit_code = EXIT_PRECONDITION_REJECTED
    guest_observation_started = False
    driver_invoked = False
    file_pull_invocations = 0
    file_push_invocations = 0
    guest_exec_invocations = 0
    resume_invocations = 0
    postflight_invocations = 0
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
                raise InstallArtifactsStagedResumeError(f"{name}-empty")
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
            raise InstallArtifactsStagedResumeError(
                "utmctl-process-active-before-resume"
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
        for name, (path, expected) in binding.live_artifacts.items():
            payloads = [
                pull(
                    path,
                    f"{name}-live-readback-{index}",
                    exact=expected,
                )
                for index in (1, 2)
            ]
            resume_evidence.require_equal_payloads(payloads, f"{name}-live")

        stage = "guest-resume-root-create"
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
                request.guest_resume_root,
            ),
            request.command_timeout_seconds,
        )
        writer.write_json(f"{stage}.json", root_observation.as_json())
        input_transfer._require_successful_small_observation(root_observation, stage)

        stage = "guest-resume-driver-push"
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
                "guest-resume-driver-chown",
                ("/bin/chown", "root:root", request.guest_driver_incoming),
            ),
            (
                "guest-resume-driver-chmod",
                ("/bin/chmod", "0600", request.guest_driver_incoming),
            ),
            (
                "guest-resume-driver-publish",
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
            "guest-resume-driver-readback",
            exact=binding.driver_bytes,
        )

        stage = "guest-install-artifacts-staged-resume"
        guest_exec_invocations += 1
        driver_invoked = True
        resume_invocations = 1
        driver_observation = command_runner.run(
            driver_argv(request, binding), request.resume_timeout_seconds
        )
        writer.write_json(f"{stage}.json", driver_observation.as_json())
        if driver_observation.timed_out:
            raise InstallArtifactsStagedResumeError("guest-resume-driver-timed-out")
        if driver_observation.stdout.truncated or driver_observation.stderr.truncated:
            raise InstallArtifactsStagedResumeError(
                "guest-resume-driver-output-truncated"
            )
        if driver_observation.stdout.total_bytes or driver_observation.stderr.total_bytes:
            raise InstallArtifactsStagedResumeError(
                "guest-resume-driver-output-not-empty"
            )

        sleeper(float(request.evidence_settle_seconds))
        driver_sha256 = hashlib.sha256(binding.driver_bytes).hexdigest()
        pull(
            request.guest_marker_path,
            "guest-resume-marker-readback",
            exact=resume_driver.canonical_json(
                {
                    "driver_sha256": driver_sha256,
                    "format": resume_driver.MARKER_FORMAT,
                    "resume_attempt_id": request.resume_attempt_id,
                }
            ),
        )
        terminal_payloads = [
            pull(
                request.guest_terminal_path,
                f"guest-resume-evidence-readback-{index}",
            )
            for index in (1, 2)
        ]
        terminal_payload = resume_evidence.require_equal_payloads(
            terminal_payloads, "guest-resume-evidence"
        )
        guest_terminal = resume_evidence.parse_driver_terminal(
            terminal_payload, request, binding
        )
        writer.write_json("guest-resume-evidence.json", guest_terminal)
        pull(
            request.guest_phase_path,
            "guest-resume-phase-readback",
            exact=resume_driver.canonical_json(
                {
                    "format": resume_driver.EVIDENCE_FORMAT,
                    "phase": guest_terminal["phase"],
                }
            ),
        )
        expected_driver_exit = {
            "resume-completed": 0,
            "resume-rejected": 10,
            "state-indeterminate": 12,
        }[str(guest_terminal["outcome"])]
        if driver_observation.exit_code != expected_driver_exit:
            raise InstallArtifactsStagedResumeError(
                "guest-resume-driver-exit-evidence-mismatch"
            )

        postflight_invocations = int(
            guest_terminal.get("postflight_invocations", 0)
        )
        if guest_terminal["outcome"] == "resume-completed":
            artifacts = resume_evidence.pull_resume_artifacts(
                request, guest_terminal, pull
            )
            resume_evidence.validate_resume_artifacts(
                artifacts, guest_terminal, binding
            )
            outcome = "resume-completed"
            exit_code = EXIT_RESUME_COMPLETED
            reason = "one-shot-install-artifacts-staged-resume-passed"
        elif guest_terminal["outcome"] == "resume-rejected":
            outcome = "resume-rejected"
            exit_code = EXIT_RESUME_REJECTED
            resume_invocations = 0
            reason = f"guest-resume-rejected:{guest_terminal['reason']}"
        else:
            outcome = "state-indeterminate"
            exit_code = EXIT_STATE_INDETERMINATE
            reason = f"guest-resume-state-indeterminate:{guest_terminal['reason']}"

        pull(
            binding.network.guest_evidence_path,
            "network-evidence-postflight-readback",
            exact=binding.network.guest_evidence_bytes,
        )
    except (
        InstallArtifactsStagedResumeError,
        input_transfer.CanonicalInputTransferError,
        network_ready.NetworkReadyError,
        resume_evidence.ResumeEvidenceError,
        start_control.StartControlError,
        transport_bindings.BindingError,
        OSError,
        tarfile.TarError,
        ValueError,
    ) as exc:
        reason = f"{stage}:{exc}"
        if driver_invoked:
            outcome = "state-indeterminate"
            exit_code = EXIT_STATE_INDETERMINATE
        elif guest_observation_started:
            outcome = "resume-rejected"
            exit_code = EXIT_RESUME_REJECTED
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
                    if driver_invoked
                    else (
                        "resume-rejected"
                        if guest_observation_started
                        else "precondition-rejected"
                    )
                )
                exit_code = {
                    "state-indeterminate": EXIT_STATE_INDETERMINATE,
                    "resume-rejected": EXIT_RESUME_REJECTED,
                    "precondition-rejected": EXIT_PRECONDITION_REJECTED,
                }[outcome]
            source_bundle.file_object.close()

    terminal = {
        "automatic_cleanup": "not-performed",
        "automatic_next_checkpoint": "not-performed",
        "automatic_quit": "not-performed",
        "automatic_retry": "not-performed",
        "automatic_stop": "not-performed",
        "checkpoint_attempt_id": request.checkpoint_attempt_id,
        "file_pull_invocations": file_pull_invocations,
        "file_push_invocations": file_push_invocations,
        "format": EVIDENCE_FORMAT,
        "guest_exec_invocations": guest_exec_invocations,
        "guest_resume_outcome": (
            guest_terminal.get("outcome") if guest_terminal else None
        ),
        "maintenance_resume_invocations": resume_invocations,
        "operation_id": "existing-secret-hash-only",
        "operation_id_sha256": resume_bindings.REQUIRED_OPERATION_ID_SHA256,
        "outcome": outcome,
        "plain_utmctl_list": "not-performed",
        "plain_utmctl_start": "not-performed",
        "plain_utmctl_status": "not-performed",
        "postflight_invocations": postflight_invocations,
        "reason": reason,
        "resume_attempt_id": request.resume_attempt_id,
        "source_bundle_sha256": request.source_bundle_sha256,
        "source_bundle_size": request.source_bundle_size,
        "target_name": request.target_name,
        "target_uuid": request.target_uuid,
        "transaction": (
            guest_terminal.get("transaction")
            if guest_terminal
            else (
                "state-indeterminate"
                if driver_invoked
                else "artifacts-staged-preserved"
            )
        ),
    }
    writer.write_json("terminal.json", terminal)
    _require_no_raw_operation_id(request.output_root)
    manifest_sha256 = writer.write_manifest()
    return InstallArtifactsStagedResumeResult(
        outcome=outcome,
        exit_code=exit_code,
        evidence_root=request.output_root,
        manifest_sha256=manifest_sha256,
        resume_invocations=resume_invocations,
    )


def validate_resume_bindings(
    request: InstallArtifactsStagedResumeRequest,
) -> resume_bindings.ResumeBinding:
    if Path(__file__).absolute() != request.repository_root / CONTROL_RELATIVE_PATH:
        raise InstallArtifactsStagedResumeError("executed-control-path-mismatch")
    for module_path, expected, label in (
        (Path(resume_bindings.__file__).absolute(), BINDINGS_RELATIVE_PATH, "bindings"),
        (Path(resume_evidence.__file__).absolute(), EVIDENCE_RELATIVE_PATH, "evidence"),
        (Path(resume_driver.__file__).absolute(), DRIVER_RELATIVE_PATH, "driver"),
    ):
        if module_path != request.repository_root / expected:
            raise InstallArtifactsStagedResumeError(
                f"executed-{label}-path-mismatch"
            )
    try:
        return resume_bindings.validate_resume_bindings(request)
    except ValueError as exc:
        raise InstallArtifactsStagedResumeError(str(exc)) from exc


def driver_argv(
    request: InstallArtifactsStagedResumeRequest,
    binding: resume_bindings.ResumeBinding,
) -> tuple[str, ...]:
    return (
        "utmctl",
        "exec",
        request.target_uuid,
        "--cmd",
        "/usr/bin/python3",
        "-B",
        request.guest_driver_path,
        "--resume-attempt-id",
        request.resume_attempt_id,
        "--checkpoint-attempt-id",
        request.checkpoint_attempt_id,
        "--target-uuid",
        request.target_uuid,
        "--expected-boot-id-sha256",
        binding.boot_id_sha256,
        "--expected-operation-id-sha256",
        binding.operation_id_sha256,
        "--expected-checkpoint-sha256",
        binding.checkpoint_sha256,
        "--expected-crash-state-sha256",
        binding.crash_state_sha256,
        "--expected-driver-sha256",
        hashlib.sha256(binding.driver_bytes).hexdigest(),
        "--control-root",
        request.guest_resume_root,
    )


def _require_no_raw_operation_id(root: Path) -> None:
    for path in root.iterdir():
        if path.is_file() and resume_evidence.RAW_OPERATION_TOKEN.search(
            path.read_bytes()
        ):
            raise InstallArtifactsStagedResumeError(
                f"host-evidence-contains-raw-operation-id:{path.name}"
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
            "Run one create-new install_artifacts_staged exact resume and one "
            "postflight without retry, cleanup, stop, quit, or next checkpoint."
        )
    )
    parser.add_argument("command", choices=("resume",))
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
    parser.add_argument("--prior-checkpoint-root", type=Path, required=True)
    parser.add_argument("--prior-checkpoint-manifest-sha256", required=True)
    parser.add_argument("--source-bundle-path", type=Path, required=True)
    parser.add_argument("--source-bundle-size", type=int, required=True)
    parser.add_argument("--source-bundle-sha256", required=True)
    parser.add_argument("--output-root", type=Path, required=True)
    parser.add_argument("--transfer-attempt-id", required=True)
    parser.add_argument("--resolution-attempt-id", required=True)
    parser.add_argument("--preflight-attempt-id", required=True)
    parser.add_argument("--checkpoint-attempt-id", required=True)
    parser.add_argument("--resume-attempt-id", required=True)
    parser.add_argument("--target-uuid", required=True)
    parser.add_argument("--target-name", required=True)
    parser.add_argument("--target-package-path", type=Path, required=True)
    parser.add_argument("--command-timeout-seconds", type=int, default=60)
    parser.add_argument("--resume-timeout-seconds", type=int, default=1500)
    parser.add_argument("--evidence-settle-seconds", type=int, default=10)
    parser.add_argument(
        "--authorized-install-artifacts-staged-exact-resume",
        action="store_true",
    )
    parser.add_argument(
        "--authorized-existing-operation-secret-hash-only",
        action="store_true",
    )
    parser.add_argument(
        "--authorized-one-resume-and-one-postflight", action="store_true"
    )
    parser.add_argument(
        "--authorized-no-retry-cleanup-stop-quit-or-next-checkpoint",
        action="store_true",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    request = InstallArtifactsStagedResumeRequest(
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
        prior_checkpoint_root=args.prior_checkpoint_root,
        prior_checkpoint_manifest_sha256=args.prior_checkpoint_manifest_sha256,
        source_bundle_path=args.source_bundle_path,
        source_bundle_size=args.source_bundle_size,
        source_bundle_sha256=args.source_bundle_sha256,
        output_root=args.output_root,
        transfer_attempt_id=args.transfer_attempt_id,
        resolution_attempt_id=args.resolution_attempt_id,
        preflight_attempt_id=args.preflight_attempt_id,
        checkpoint_attempt_id=args.checkpoint_attempt_id,
        resume_attempt_id=args.resume_attempt_id,
        target_uuid=args.target_uuid,
        target_name=args.target_name,
        target_package_path=args.target_package_path,
        command_timeout_seconds=args.command_timeout_seconds,
        resume_timeout_seconds=args.resume_timeout_seconds,
        evidence_settle_seconds=args.evidence_settle_seconds,
        authorized_install_artifacts_staged_exact_resume=(
            args.authorized_install_artifacts_staged_exact_resume
        ),
        authorized_existing_operation_secret_hash_only=(
            args.authorized_existing_operation_secret_hash_only
        ),
        authorized_one_resume_and_one_postflight=(
            args.authorized_one_resume_and_one_postflight
        ),
        authorized_no_retry_cleanup_stop_quit_or_next_checkpoint=(
            args.authorized_no_retry_cleanup_stop_quit_or_next_checkpoint
        ),
    )
    try:
        result = run_install_artifacts_staged_resume(request)
    except (
        InstallArtifactsStagedResumeError,
        input_transfer.CanonicalInputTransferError,
        network_ready.NetworkReadyError,
        resume_evidence.ResumeEvidenceError,
        start_control.StartControlError,
        transport_bindings.BindingError,
        OSError,
        tarfile.TarError,
        ValueError,
    ) as exc:
        print(f"l6_utm_install_artifacts_staged_resume_error={exc}", file=sys.stderr)
        return EXIT_PRECONDITION_REJECTED
    print(f"install_artifacts_staged_resume_outcome={result.outcome}")
    print(f"evidence_root={result.evidence_root}")
    print(f"files_sha256={result.manifest_sha256}")
    return result.exit_code


if __name__ == "__main__":
    raise SystemExit(main())
