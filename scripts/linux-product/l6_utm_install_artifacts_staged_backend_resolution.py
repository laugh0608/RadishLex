#!/usr/bin/env python3
from __future__ import annotations

import argparse
import re
import sys
import time
import uuid
from dataclasses import dataclass
from pathlib import Path
from typing import Callable, Protocol

import l6_utm_canonical_input_transfer as input_transfer
import l6_utm_guest_network_ready as network_ready
import l6_utm_install_artifacts_staged_backend_resolution_bindings as bindings
import l6_utm_install_artifacts_staged_checkpoint as checkpoint_control
import l6_utm_install_artifacts_staged_resume_bindings as resume_bindings
import l6_utm_launch_transport_bindings as transport_bindings
import l6_utm_launch_transport_v2 as launch_transport
import l6_utm_start_once as start_control
import l6_v4_canonical_input_install as guest_installer


EVIDENCE_FORMAT = bindings.EVIDENCE_FORMAT
CONTROL_RELATIVE_PATH = bindings.CONTROL_RELATIVE_PATH
BINDINGS_RELATIVE_PATH = bindings.BINDINGS_RELATIVE_PATH
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
REQUIRED_PRIOR_RESUME_FAILURE_MANIFEST_SHA256 = (
    bindings.REQUIRED_PRIOR_FAILURE_MANIFEST_SHA256
)
REQUIRED_TRANSFER_ATTEMPT_ID = checkpoint_control.REQUIRED_TRANSFER_ATTEMPT_ID
REQUIRED_RESOLUTION_ATTEMPT_ID = checkpoint_control.REQUIRED_RESOLUTION_ATTEMPT_ID
REQUIRED_PREFLIGHT_ATTEMPT_ID = checkpoint_control.REQUIRED_PREFLIGHT_ATTEMPT_ID
REQUIRED_CHECKPOINT_ATTEMPT_ID = resume_bindings.REQUIRED_CHECKPOINT_ATTEMPT_ID
REQUIRED_RESUME_ATTEMPT_ID = resume_bindings.REQUIRED_RESUME_ATTEMPT_ID
REQUIRED_BACKEND_RESOLUTION_ATTEMPT_ID = (
    bindings.REQUIRED_BACKEND_RESOLUTION_ATTEMPT_ID
)
REQUIRED_SOURCE_BUNDLE_SIZE = checkpoint_control.REQUIRED_SOURCE_BUNDLE_SIZE
REQUIRED_SOURCE_BUNDLE_SHA256 = checkpoint_control.REQUIRED_SOURCE_BUNDLE_SHA256
REQUIRED_TARGET_UUID = checkpoint_control.REQUIRED_TARGET_UUID
REQUIRED_TARGET_NAME = checkpoint_control.REQUIRED_TARGET_NAME
PROCESS_COMMAND = launch_transport.PROCESS_COMMAND
EXIT_RUNTIME_REACTIVATED = 0
EXIT_REGISTERED_STOPPED = 10
EXIT_PRECONDITION_REJECTED = 11
EXIT_STATE_INDETERMINATE = 12
HEX_40 = re.compile(r"[0-9a-f]{40}")
SAFE_ATTEMPT_ID = re.compile(r"[a-z0-9][a-z0-9-]{0,95}")


class BackendResolutionError(ValueError):
    pass


@dataclass(frozen=True)
class BackendResolutionRequest:
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
    prior_resume_failure_root: Path
    prior_resume_failure_manifest_sha256: str
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
    target_uuid: str
    target_name: str
    target_package_path: Path
    poll_attempts: int
    poll_interval_seconds: int
    command_timeout_seconds: int
    authorized_install_artifacts_staged_backend_resolution: bool
    authorized_one_potential_backend_reactivation_status: bool
    authorized_no_list_start_resume_guest_retry_stop_or_quit: bool

    def validate(self) -> None:
        paths = (
            (self.repository_root, "repository-root"),
            (self.prior_network_root, "prior-network-root"),
            (self.prior_transfer_root, "prior-transfer-root"),
            (self.prior_resolution_root, "prior-resolution-root"),
            (self.prior_preflight_root, "prior-preflight-root"),
            (self.prior_checkpoint_root, "prior-checkpoint-root"),
            (self.prior_resume_failure_root, "prior-resume-failure-root"),
            (self.source_bundle_path, "source-bundle-path"),
            (self.output_root, "output-root"),
            (self.target_package_path, "target-package-path"),
        )
        for path, label in paths:
            if not path.is_absolute() or ".." in path.parts:
                raise BackendResolutionError(
                    f"{label}-must-be-absolute-normalized"
                )
        for path, label in paths[:-2] + ((self.target_package_path, "target"),):
            if _paths_overlap(self.output_root, path):
                raise BackendResolutionError(
                    f"output-root-must-not-overlap-{label}"
                )
        if not HEX_40.fullmatch(self.expected_repository_head):
            raise BackendResolutionError("expected-repository-head-invalid")
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
                self.prior_resume_failure_manifest_sha256,
                REQUIRED_PRIOR_RESUME_FAILURE_MANIFEST_SHA256,
                "prior-resume-failure-manifest",
            ),
            (
                self.source_bundle_sha256,
                REQUIRED_SOURCE_BUNDLE_SHA256,
                "source-bundle-sha256",
            ),
        )
        for actual, expected, label in fixed:
            if actual != expected:
                raise BackendResolutionError(f"required-{label}-mismatch")
        if self.source_bundle_size != REQUIRED_SOURCE_BUNDLE_SIZE:
            raise BackendResolutionError("required-source-bundle-size-mismatch")
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
            (
                self.backend_resolution_attempt_id,
                REQUIRED_BACKEND_RESOLUTION_ATTEMPT_ID,
                "backend-resolution",
            ),
        )
        for actual, expected, label in attempts:
            if actual != expected or not SAFE_ATTEMPT_ID.fullmatch(actual):
                raise BackendResolutionError(
                    f"required-{label}-attempt-id-mismatch"
                )
        if len({attempt[0] for attempt in attempts}) != len(attempts):
            raise BackendResolutionError("attempt-id-overlap")
        try:
            canonical_uuid = str(uuid.UUID(self.target_uuid)).upper()
        except ValueError as exc:
            raise BackendResolutionError("target-uuid-invalid") from exc
        if canonical_uuid != self.target_uuid or self.target_uuid != REQUIRED_TARGET_UUID:
            raise BackendResolutionError("required-target-uuid-mismatch")
        if self.target_name != REQUIRED_TARGET_NAME:
            raise BackendResolutionError("required-target-name-mismatch")
        if self.target_package_path != transport_bindings.expected_target_package_path(
            self.target_name
        ):
            raise BackendResolutionError("target-package-path-mismatch")
        for value, low, high, label in (
            (self.poll_attempts, 1, 60, "poll-attempts"),
            (self.poll_interval_seconds, 1, 10, "poll-interval"),
            (self.command_timeout_seconds, 1, 60, "command-timeout"),
        ):
            if not low <= value <= high:
                raise BackendResolutionError(f"{label}-out-of-range")
        authorizations = (
            (
                self.authorized_install_artifacts_staged_backend_resolution,
                "authorized-install-artifacts-staged-backend-resolution-required",
            ),
            (
                self.authorized_one_potential_backend_reactivation_status,
                "authorized-one-potential-backend-reactivation-status-required",
            ),
            (
                self.authorized_no_list_start_resume_guest_retry_stop_or_quit,
                "authorized-no-list-start-resume-guest-retry-stop-or-quit-required",
            ),
        )
        for authorized, reason in authorizations:
            if not authorized:
                raise BackendResolutionError(reason)

    def as_json(self) -> dict[str, object]:
        return {
            "authorization": {
                "install_artifacts_staged_backend_resolution": True,
                "no_list_start_resume_guest_retry_stop_or_quit": True,
                "one_potential_backend_reactivation_status": True,
            },
            "backend_resolution_attempt_id": (
                self.backend_resolution_attempt_id
            ),
            "checkpoint_attempt_id": self.checkpoint_attempt_id,
            "command_timeout_seconds": self.command_timeout_seconds,
            "expected_repository_head": self.expected_repository_head,
            "format": EVIDENCE_FORMAT,
            "poll_attempts": self.poll_attempts,
            "poll_interval_seconds": self.poll_interval_seconds,
            "prior_checkpoint_manifest_sha256": (
                self.prior_checkpoint_manifest_sha256
            ),
            "prior_network_manifest_sha256": self.prior_network_manifest_sha256,
            "prior_preflight_manifest_sha256": (
                self.prior_preflight_manifest_sha256
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
            "resume_attempt_id": self.resume_attempt_id,
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
        }


@dataclass(frozen=True)
class BackendResolutionResult:
    outcome: str
    exit_code: int
    evidence_root: Path
    manifest_sha256: str
    backend_reactivation_probe_invocations: int
    observation_poll_count: int


class CommandRunner(Protocol):
    def run(
        self, argv: tuple[str, ...], timeout_seconds: int
    ) -> start_control.CommandObservation: ...


Sleeper = Callable[[float], None]


def run_backend_resolution(
    request: BackendResolutionRequest,
    *,
    runner: CommandRunner | None = None,
    binding_validator=None,
    source_opener=None,
    source_revalidator=None,
    target_validator=None,
    sleeper: Sleeper = time.sleep,
) -> BackendResolutionResult:
    request.validate()
    writer = start_control.EvidenceWriter.create(request.output_root)
    writer.write_json("request.json", request.as_json())
    command_runner = runner or start_control.SubprocessCommandRunner()
    validate_bindings = binding_validator or validate_backend_resolution_bindings
    open_source = source_opener or input_transfer.open_source_bundle
    revalidate_source = (
        source_revalidator or input_transfer.revalidate_open_source_bundle
    )
    validate_target = target_validator or network_ready.validate_target_files

    stage = "binding-preflight"
    reason = "not-run"
    outcome = "precondition-rejected"
    exit_code = EXIT_PRECONDITION_REJECTED
    status_invocations = 0
    observation_poll_count = 0
    registered_status: str | None = None
    status_parse_error: str | None = None
    source_bundle: input_transfer.SourceBundle | None = None
    terminal_processes: tuple[dict[str, object], ...] = ()
    terminal_handles = "not-observed"
    observed_runtime = False
    all_postflight_quiescent = True

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
        preflight_processes = launch_transport.parse_relevant_processes(
            process_observation
        )
        writer.write_json(
            "host-process-preflight.json",
            _process_evidence(process_observation, preflight_processes),
        )
        if preflight_processes:
            raise BackendResolutionError(
                "relevant-host-process-before-backend-resolution"
            )

        stage = "target-handles-preflight"
        handles_observation = command_runner.run(
            network_ready._lsof_argv(request), request.command_timeout_seconds
        )
        handles_state, handles = classify_target_handles(
            handles_observation, request
        )
        writer.write_json(
            "target-handles-preflight.json",
            _handles_evidence(handles_observation, handles_state, handles),
        )
        if handles_state != "absent":
            raise BackendResolutionError(
                "target-handles-present-before-backend-resolution"
            )

        stage = "utmctl-status-backend-resolution"
        status_invocations = 1
        status_observation = command_runner.run(
            ("utmctl", "status", request.target_uuid),
            request.command_timeout_seconds,
        )
        writer.write_json("utmctl-status-once.json", status_observation.as_json())
        try:
            registered_status = start_control.parse_utmctl_status(
                status_observation
            )
        except start_control.StartControlError as exc:
            status_parse_error = str(exc)

        stage = "post-status-observation"
        for attempt in range(1, request.poll_attempts + 1):
            observation_poll_count = attempt
            process_observation = command_runner.run(
                PROCESS_COMMAND, request.command_timeout_seconds
            )
            terminal_processes = launch_transport.parse_relevant_processes(
                process_observation
            )
            writer.write_json(
                f"host-process-poll-{attempt:03d}.json",
                _process_evidence(process_observation, terminal_processes),
            )

            handles_observation = command_runner.run(
                network_ready._lsof_argv(request),
                request.command_timeout_seconds,
            )
            terminal_handles, handles = classify_target_handles(
                handles_observation, request
            )
            writer.write_json(
                f"target-handles-poll-{attempt:03d}.json",
                _handles_evidence(
                    handles_observation, terminal_handles, handles
                ),
            )

            backend_count = launch_transport._backend_process_count(
                terminal_processes
            )
            utmctl_count = launch_transport._role_count(
                terminal_processes, "utmctl"
            )
            if terminal_processes or terminal_handles != "absent":
                all_postflight_quiescent = False
            if (
                registered_status == "started"
                and backend_count >= 1
                and utmctl_count == 0
                and terminal_handles == "present"
            ):
                observed_runtime = True
                break
            if attempt < request.poll_attempts:
                sleeper(float(request.poll_interval_seconds))

        stage = "target-files-postflight"
        writer.write_json("target-files-postflight.json", validate_target(request))

        if observed_runtime:
            outcome = "runtime-reactivated"
            exit_code = EXIT_RUNTIME_REACTIVATED
            reason = "single-status-and-target-handles-observed-runtime"
        elif registered_status == "stopped" and all_postflight_quiescent:
            outcome = "registered-stopped"
            exit_code = EXIT_REGISTERED_STOPPED
            reason = "single-status-stopped-and-bounded-host-quiescence"
        elif registered_status == "started" and all_postflight_quiescent:
            outcome = "registered-started-runtime-unavailable"
            exit_code = EXIT_STATE_INDETERMINATE
            reason = "single-status-started-without-bounded-backend-or-handles"
        else:
            outcome = "state-indeterminate"
            exit_code = EXIT_STATE_INDETERMINATE
            if status_parse_error is not None:
                reason = f"utmctl-status-unresolved:{status_parse_error}"
            else:
                reason = "registered-status-and-host-runtime-observations-inconsistent"
    except (
        BackendResolutionError,
        input_transfer.CanonicalInputTransferError,
        network_ready.NetworkReadyError,
        start_control.StartControlError,
        launch_transport.LaunchTransportError,
        transport_bindings.BindingError,
        OSError,
        ValueError,
    ) as exc:
        reason = f"{stage}:{exc}"
        if status_invocations == 1:
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
                    if status_invocations == 1
                    else "precondition-rejected"
                )
                exit_code = (
                    EXIT_STATE_INDETERMINATE
                    if status_invocations == 1
                    else EXIT_PRECONDITION_REJECTED
                )
            source_bundle.file_object.close()

    terminal = {
        "automatic_cleanup": "not-performed",
        "automatic_quit": "not-performed",
        "automatic_retry": "not-performed",
        "automatic_stop": "not-performed",
        "backend_reactivation_probe_invocations": status_invocations,
        "backend_resolution_attempt_id": request.backend_resolution_attempt_id,
        "file_pull_invocations": 0,
        "file_push_invocations": 0,
        "format": EVIDENCE_FORMAT,
        "guest_exec_invocations": 0,
        "maintenance_resume_invocations": 0,
        "observation_poll_count": observation_poll_count,
        "operation_id": "not-read-or-generated",
        "outcome": outcome,
        "plain_utmctl_list": "not-performed",
        "plain_utmctl_start": "not-performed",
        "plain_utmctl_status": (
            "attempted-once-as-potential-backend-reactivation"
            if status_invocations == 1
            else "not-performed"
        ),
        "reason": reason,
        "registered_status": registered_status,
        "saved_state": _saved_state_disposition(
            outcome, registered_status
        ),
        "target_handles_terminal": terminal_handles,
        "target_name": request.target_name,
        "target_uuid": request.target_uuid,
        "terminal_relevant_host_process_count": len(terminal_processes),
        "transaction": "artifacts-staged-preserved-no-resume",
    }
    writer.write_json("terminal.json", terminal)
    manifest_sha256 = writer.write_manifest()
    return BackendResolutionResult(
        outcome=outcome,
        exit_code=exit_code,
        evidence_root=request.output_root,
        manifest_sha256=manifest_sha256,
        backend_reactivation_probe_invocations=status_invocations,
        observation_poll_count=observation_poll_count,
    )


def validate_backend_resolution_bindings(
    request: BackendResolutionRequest,
) -> bindings.BackendResolutionBinding:
    if Path(__file__).absolute() != request.repository_root / CONTROL_RELATIVE_PATH:
        raise BackendResolutionError("executed-control-path-mismatch")
    if Path(bindings.__file__).absolute() != (
        request.repository_root / BINDINGS_RELATIVE_PATH
    ):
        raise BackendResolutionError("executed-bindings-path-mismatch")
    try:
        return bindings.validate_backend_resolution_bindings(request)
    except ValueError as exc:
        raise BackendResolutionError(str(exc)) from exc


def classify_target_handles(
    observation: start_control.CommandObservation,
    request: BackendResolutionRequest,
) -> tuple[str, dict[str, object]]:
    if (
        not observation.timed_out
        and observation.exit_code == 1
        and not observation.stdout.truncated
        and not observation.stderr.truncated
        and observation.stdout.total_bytes == 0
        and observation.stderr.total_bytes == 0
    ):
        return "absent", {
            "backend_command": None,
            "efi_handle_count": 0,
            "process_record_count": 0,
            "qcow2_handle_count": 0,
        }
    try:
        parsed = network_ready.parse_target_handles(observation, request)
    except (network_ready.NetworkReadyError, start_control.StartControlError) as exc:
        raise BackendResolutionError(str(exc)) from exc
    return "present", parsed


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


def _handles_evidence(
    observation: start_control.CommandObservation,
    state: str,
    handles: dict[str, object],
) -> dict[str, object]:
    return {
        "format": EVIDENCE_FORMAT,
        "observation": network_ready._observation_metadata(observation),
        "state": state,
        **handles,
    }


def _saved_state_disposition(outcome: str, status: str | None) -> str:
    if outcome == "runtime-reactivated":
        return "runtime-active-observed-saved-state-not-inferred"
    if outcome == "registered-stopped":
        return "registered-stopped-saved-state-not-inferred"
    if status == "started":
        return "saved-or-runtime-unavailable-not-distinguished-by-status"
    return "not-determined"


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
            "Run one authorized utmctl status that may reactivate the target "
            "backend, then classify bounded host state without list, start, "
            "resume, guest access, retry, stop, or quit."
        )
    )
    parser.add_argument("command", choices=("resolve-once",))
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
    parser.add_argument("--prior-resume-failure-root", type=Path, required=True)
    parser.add_argument(
        "--prior-resume-failure-manifest-sha256", required=True
    )
    parser.add_argument("--source-bundle-path", type=Path, required=True)
    parser.add_argument("--source-bundle-size", type=int, required=True)
    parser.add_argument("--source-bundle-sha256", required=True)
    parser.add_argument("--output-root", type=Path, required=True)
    parser.add_argument("--transfer-attempt-id", required=True)
    parser.add_argument("--resolution-attempt-id", required=True)
    parser.add_argument("--preflight-attempt-id", required=True)
    parser.add_argument("--checkpoint-attempt-id", required=True)
    parser.add_argument("--resume-attempt-id", required=True)
    parser.add_argument("--backend-resolution-attempt-id", required=True)
    parser.add_argument("--target-uuid", required=True)
    parser.add_argument("--target-name", required=True)
    parser.add_argument("--target-package-path", type=Path, required=True)
    parser.add_argument("--poll-attempts", type=int, default=10)
    parser.add_argument("--poll-interval-seconds", type=int, default=1)
    parser.add_argument("--command-timeout-seconds", type=int, default=60)
    parser.add_argument(
        "--authorized-install-artifacts-staged-backend-resolution",
        action="store_true",
    )
    parser.add_argument(
        "--authorized-one-potential-backend-reactivation-status",
        action="store_true",
    )
    parser.add_argument(
        "--authorized-no-list-start-resume-guest-retry-stop-or-quit",
        action="store_true",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    request = BackendResolutionRequest(
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
        prior_resume_failure_root=args.prior_resume_failure_root,
        prior_resume_failure_manifest_sha256=(
            args.prior_resume_failure_manifest_sha256
        ),
        source_bundle_path=args.source_bundle_path,
        source_bundle_size=args.source_bundle_size,
        source_bundle_sha256=args.source_bundle_sha256,
        output_root=args.output_root,
        transfer_attempt_id=args.transfer_attempt_id,
        resolution_attempt_id=args.resolution_attempt_id,
        preflight_attempt_id=args.preflight_attempt_id,
        checkpoint_attempt_id=args.checkpoint_attempt_id,
        resume_attempt_id=args.resume_attempt_id,
        backend_resolution_attempt_id=args.backend_resolution_attempt_id,
        target_uuid=args.target_uuid,
        target_name=args.target_name,
        target_package_path=args.target_package_path,
        poll_attempts=args.poll_attempts,
        poll_interval_seconds=args.poll_interval_seconds,
        command_timeout_seconds=args.command_timeout_seconds,
        authorized_install_artifacts_staged_backend_resolution=(
            args.authorized_install_artifacts_staged_backend_resolution
        ),
        authorized_one_potential_backend_reactivation_status=(
            args.authorized_one_potential_backend_reactivation_status
        ),
        authorized_no_list_start_resume_guest_retry_stop_or_quit=(
            args.authorized_no_list_start_resume_guest_retry_stop_or_quit
        ),
    )
    try:
        result = run_backend_resolution(request)
    except BackendResolutionError as exc:
        print(
            f"backend resolution rejected before evidence creation: {exc}",
            file=sys.stderr,
        )
        return EXIT_PRECONDITION_REJECTED
    print(f"outcome={result.outcome}")
    print(f"evidence_root={result.evidence_root}")
    print(f"manifest_sha256={result.manifest_sha256}")
    print(
        "backend_reactivation_probe_invocations="
        f"{result.backend_reactivation_probe_invocations}"
    )
    print(f"observation_poll_count={result.observation_poll_count}")
    return result.exit_code


if __name__ == "__main__":
    raise SystemExit(main())
