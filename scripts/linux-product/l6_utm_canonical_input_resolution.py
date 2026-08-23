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

import l6_utm_canonical_input_resolution_bindings as resolution_bindings
import l6_utm_canonical_input_resolution_evidence as resolution_evidence
import l6_utm_canonical_input_transfer as input_transfer
import l6_utm_guest_network_ready as network_ready
import l6_utm_launch_transport_bindings as transport_bindings
import l6_utm_launch_transport_v2 as launch_transport
import l6_utm_start_once as start_control
import l6_v4_canonical_input_install as guest_installer
import l6_v4_canonical_input_resolution_probe as resolution_probe


EVIDENCE_FORMAT = resolution_bindings.EVIDENCE_FORMAT
CONTROL_RELATIVE_PATH = resolution_bindings.CONTROL_RELATIVE_PATH
BINDINGS_RELATIVE_PATH = resolution_bindings.BINDINGS_RELATIVE_PATH
PROBE_RELATIVE_PATH = resolution_bindings.PROBE_RELATIVE_PATH
EVIDENCE_RELATIVE_PATH = resolution_bindings.EVIDENCE_RELATIVE_PATH
REQUIRED_PRIOR_NETWORK_MANIFEST_SHA256 = (
    input_transfer.REQUIRED_PRIOR_NETWORK_MANIFEST_SHA256
)
REQUIRED_PRIOR_TRANSFER_MANIFEST_SHA256 = (
    resolution_bindings.REQUIRED_PRIOR_TRANSFER_MANIFEST_SHA256
)
REQUIRED_TRANSFER_ATTEMPT_ID = (
    resolution_bindings.REQUIRED_TRANSFER_ATTEMPT_ID
)
REQUIRED_SOURCE_BUNDLE_SIZE = resolution_bindings.REQUIRED_SOURCE_BUNDLE_SIZE
REQUIRED_SOURCE_BUNDLE_SHA256 = (
    resolution_bindings.REQUIRED_SOURCE_BUNDLE_SHA256
)
REQUIRED_TARGET_UUID = input_transfer.REQUIRED_TARGET_UUID
REQUIRED_TARGET_NAME = input_transfer.REQUIRED_TARGET_NAME
PROCESS_COMMAND = launch_transport.PROCESS_COMMAND
EXIT_INPUT_READY = 0
EXIT_TRANSFER_FAILED_CLOSED = 10
EXIT_PRECONDITION_REJECTED = 11
EXIT_STATE_INDETERMINATE = 12
HEX_40 = re.compile(r"[0-9a-f]{40}")
SAFE_ATTEMPT_ID = re.compile(r"[a-z0-9][a-z0-9-]{0,63}")


class CanonicalInputResolutionError(ValueError):
    pass


@dataclass(frozen=True)
class CanonicalInputResolutionRequest:
    repository_root: Path
    expected_repository_head: str
    prior_network_root: Path
    prior_network_manifest_sha256: str
    prior_transfer_root: Path
    prior_transfer_manifest_sha256: str
    source_bundle_path: Path
    source_bundle_size: int
    source_bundle_sha256: str
    output_root: Path
    transfer_attempt_id: str
    resolution_attempt_id: str
    target_uuid: str
    target_name: str
    target_package_path: Path
    command_timeout_seconds: int
    probe_timeout_seconds: int
    probe_settle_seconds: int
    authorized_canonical_input_resolution: bool
    authorized_existing_result_double_readback: bool
    authorized_create_new_read_only_probe: bool
    authorized_no_transfer_operation_transaction_stop_or_retry: bool

    @property
    def guest_transfer_root(self) -> str:
        return (
            "/var/tmp/radishlex-l6-v4-input-transfer-"
            f"{self.transfer_attempt_id}"
        )

    @property
    def guest_transfer_evidence_path(self) -> str:
        return f"{self.guest_transfer_root}/transfer.evidence.json"

    @property
    def guest_resolution_root(self) -> str:
        return (
            "/var/tmp/radishlex-l6-v4-input-resolution-"
            f"{self.resolution_attempt_id}"
        )

    @property
    def guest_probe_incoming(self) -> str:
        return f"{self.guest_resolution_root}/resolution-probe.incoming.py"

    @property
    def guest_probe_path(self) -> str:
        return f"{self.guest_resolution_root}/resolution-probe.py"

    @property
    def guest_probe_evidence_path(self) -> str:
        return f"{self.guest_resolution_root}/resolution.evidence.json"

    def validate(self) -> None:
        for path, label in (
            (self.repository_root, "repository-root"),
            (self.prior_network_root, "prior-network-root"),
            (self.prior_transfer_root, "prior-transfer-root"),
            (self.source_bundle_path, "source-bundle-path"),
            (self.output_root, "output-root"),
            (self.target_package_path, "target-package-path"),
        ):
            if not path.is_absolute() or ".." in path.parts:
                raise CanonicalInputResolutionError(
                    f"{label}-must-be-absolute-normalized"
                )
        for root, reason in (
            (self.repository_root, "repository"),
            (self.prior_network_root, "prior-network"),
            (self.prior_transfer_root, "prior-transfer"),
            (self.source_bundle_path, "source-bundle"),
            (self.target_package_path, "target"),
        ):
            if _paths_overlap(self.output_root, root):
                raise CanonicalInputResolutionError(
                    f"output-root-must-not-overlap-{reason}"
                )
        if not HEX_40.fullmatch(self.expected_repository_head):
            raise CanonicalInputResolutionError(
                "expected-repository-head-invalid"
            )
        if (
            self.prior_network_manifest_sha256
            != REQUIRED_PRIOR_NETWORK_MANIFEST_SHA256
        ):
            raise CanonicalInputResolutionError(
                "required-prior-network-manifest-mismatch"
            )
        if (
            self.prior_transfer_manifest_sha256
            != REQUIRED_PRIOR_TRANSFER_MANIFEST_SHA256
        ):
            raise CanonicalInputResolutionError(
                "required-prior-transfer-manifest-mismatch"
            )
        if self.source_bundle_size != REQUIRED_SOURCE_BUNDLE_SIZE:
            raise CanonicalInputResolutionError(
                "required-source-bundle-size-mismatch"
            )
        if self.source_bundle_sha256 != REQUIRED_SOURCE_BUNDLE_SHA256:
            raise CanonicalInputResolutionError(
                "required-source-bundle-sha256-mismatch"
            )
        if self.transfer_attempt_id != REQUIRED_TRANSFER_ATTEMPT_ID:
            raise CanonicalInputResolutionError(
                "required-transfer-attempt-id-mismatch"
            )
        if (
            not SAFE_ATTEMPT_ID.fullmatch(self.resolution_attempt_id)
            or self.resolution_attempt_id == self.transfer_attempt_id
        ):
            raise CanonicalInputResolutionError(
                "resolution-attempt-id-invalid"
            )
        try:
            canonical_uuid = str(uuid.UUID(self.target_uuid)).upper()
        except ValueError as exc:
            raise CanonicalInputResolutionError("target-uuid-invalid") from exc
        if canonical_uuid != self.target_uuid:
            raise CanonicalInputResolutionError("target-uuid-not-canonical")
        if self.target_uuid != REQUIRED_TARGET_UUID:
            raise CanonicalInputResolutionError("required-target-uuid-mismatch")
        if self.target_name != REQUIRED_TARGET_NAME:
            raise CanonicalInputResolutionError("required-target-name-mismatch")
        if self.target_package_path != transport_bindings.expected_target_package_path(
            self.target_name
        ):
            raise CanonicalInputResolutionError("target-package-path-mismatch")
        if not 1 <= self.command_timeout_seconds <= 60:
            raise CanonicalInputResolutionError(
                "command-timeout-seconds-out-of-range"
            )
        if not 1 <= self.probe_timeout_seconds <= 300:
            raise CanonicalInputResolutionError(
                "probe-timeout-seconds-out-of-range"
            )
        if not 1 <= self.probe_settle_seconds <= 60:
            raise CanonicalInputResolutionError(
                "probe-settle-seconds-out-of-range"
            )
        for allowed, reason in (
            (
                self.authorized_canonical_input_resolution,
                "authorized-canonical-input-resolution-required",
            ),
            (
                self.authorized_existing_result_double_readback,
                "authorized-existing-result-double-readback-required",
            ),
            (
                self.authorized_create_new_read_only_probe,
                "authorized-create-new-read-only-probe-required",
            ),
            (
                self.authorized_no_transfer_operation_transaction_stop_or_retry,
                "authorized-no-transfer-operation-transaction-stop-or-retry-required",
            ),
        ):
            if not allowed:
                raise CanonicalInputResolutionError(reason)

    def as_json(self) -> dict[str, object]:
        return {
            "authorization": {
                "canonical_input_resolution": True,
                "create_new_read_only_probe": True,
                "existing_result_double_readback": True,
                "no_transfer_operation_transaction_stop_or_retry": True,
            },
            "command_timeout_seconds": self.command_timeout_seconds,
            "expected_repository_head": self.expected_repository_head,
            "format": EVIDENCE_FORMAT,
            "guest_final_input_root": str(guest_installer.FINAL_INPUT_ROOT),
            "guest_resolution_root": self.guest_resolution_root,
            "guest_transfer_root": self.guest_transfer_root,
            "prior_network_manifest_sha256": (
                self.prior_network_manifest_sha256
            ),
            "prior_transfer_manifest_sha256": (
                self.prior_transfer_manifest_sha256
            ),
            "probe_settle_seconds": self.probe_settle_seconds,
            "probe_timeout_seconds": self.probe_timeout_seconds,
            "resolution_attempt_id": self.resolution_attempt_id,
            "source_bundle_path_sha256": _sha256_text(
                str(self.source_bundle_path)
            ),
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
class CanonicalInputResolutionResult:
    outcome: str
    exit_code: int
    evidence_root: Path
    manifest_sha256: str
    transfer_result_readback_invocations: int
    probe_invocations: int


class CommandRunner(Protocol):
    def run(
        self,
        argv: tuple[str, ...],
        timeout_seconds: int,
        *,
        stdin_file=None,
    ) -> start_control.CommandObservation: ...


Sleeper = Callable[[float], None]


def run_canonical_input_resolution(
    request: CanonicalInputResolutionRequest,
    *,
    runner: CommandRunner | None = None,
    binding_validator=None,
    source_opener=None,
    source_revalidator=None,
    target_validator=None,
    sleeper: Sleeper = time.sleep,
) -> CanonicalInputResolutionResult:
    request.validate()
    writer = start_control.EvidenceWriter.create(request.output_root)
    writer.write_json("request.json", request.as_json())
    command_runner = runner or input_transfer.SubprocessCommandRunner()
    validate_bindings = binding_validator or validate_resolution_bindings
    open_source = source_opener or input_transfer.open_source_bundle
    revalidate_source = (
        source_revalidator or input_transfer.revalidate_open_source_bundle
    )
    validate_target = target_validator or network_ready.validate_target_files

    stage = "binding-preflight"
    reason = "not-run"
    outcome = "precondition-rejected"
    exit_code = EXIT_PRECONDITION_REJECTED
    resolution_observation_started = False
    file_pull_invocations = 0
    file_push_invocations = 0
    guest_exec_invocations = 0
    transfer_result_readback_invocations = 0
    probe_result_readback_invocations = 0
    probe_invocations = 0
    parsed_transfer_evidence: dict[str, object] | None = None
    parsed_probe_evidence: dict[str, object] | None = None
    source_bundle: input_transfer.SourceBundle | None = None

    try:
        binding = validate_bindings(request)
        writer.write_json("binding-preflight.json", binding.evidence)

        stage = "source-bundle-preflight"
        source_bundle = open_source(request)
        writer.write_json("source-bundle-preflight.json", source_bundle.as_json())

        stage = "target-files-preflight"
        writer.write_json(
            "target-files-preflight.json", validate_target(request)
        )

        stage = "host-process-preflight"
        process_observation = command_runner.run(
            PROCESS_COMMAND, request.command_timeout_seconds
        )
        start_control._require_successful_observation(
            process_observation, "host-process-preflight"
        )
        processes = launch_transport.parse_relevant_processes(process_observation)
        if any(item["role"] == "utmctl" for item in processes):
            raise CanonicalInputResolutionError(
                "utmctl-process-active-before-input-resolution"
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
        lsof_observation = command_runner.run(
            network_ready._lsof_argv(request),
            request.command_timeout_seconds,
        )
        handles = network_ready.parse_target_handles(lsof_observation, request)
        writer.write_json(
            "target-handles-preflight.json",
            {
                "format": EVIDENCE_FORMAT,
                "observation": network_ready._observation_metadata(
                    lsof_observation
                ),
                **handles,
            },
        )

        for index in (1, 2):
            stage = f"network-evidence-live-readback-{index}"
            file_pull_invocations += 1
            observation = command_runner.run(
                (
                    "utmctl",
                    "file",
                    "pull",
                    request.target_uuid,
                    binding.network.guest_evidence_path,
                ),
                request.command_timeout_seconds,
            )
            writer.write_json(
                f"network-evidence-live-readback-{index}.json",
                observation.as_json(),
            )
            input_transfer._require_exact_small_readback(
                observation,
                binding.network.guest_evidence_bytes,
                stage,
            )

        transfer_readbacks: list[start_control.CommandObservation] = []
        resolution_observation_started = True
        for index in (1, 2):
            stage = f"existing-transfer-evidence-readback-{index}"
            transfer_result_readback_invocations += 1
            file_pull_invocations += 1
            observation = command_runner.run(
                (
                    "utmctl",
                    "file",
                    "pull",
                    request.target_uuid,
                    request.guest_transfer_evidence_path,
                ),
                request.command_timeout_seconds,
            )
            writer.write_json(
                f"existing-transfer-evidence-readback-{index}.json",
                observation.as_json(),
            )
            input_transfer._require_successful_small_observation(
                observation, stage
            )
            if observation.stdout.total_bytes == 0:
                raise CanonicalInputResolutionError(f"{stage}-empty")
            transfer_readbacks.append(observation)
        transfer_payload = _require_double_readback(
            transfer_readbacks, "existing-transfer-evidence"
        )
        parsed_transfer_evidence = resolution_evidence.parse_transfer_evidence_for_resolution(
            transfer_payload, request, source_bundle.members
        )
        writer.write_json(
            "existing-transfer-evidence.json", parsed_transfer_evidence
        )
        if (
            parsed_transfer_evidence["outcome"] == "failed"
            and parsed_transfer_evidence["final_switch"] == "performed"
        ):
            raise CanonicalInputResolutionError(
                "transfer-result-failed-after-final-switch"
            )

        transfer_evidence_sha256 = hashlib.sha256(transfer_payload).hexdigest()
        transfer_evidence_size = len(transfer_payload)
        probe_bytes = binding.probe_bytes

        stage = "guest-resolution-root-create"
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
                request.guest_resolution_root,
            ),
            request.command_timeout_seconds,
        )
        writer.write_json(
            "guest-resolution-root-create.json", root_observation.as_json()
        )
        input_transfer._require_successful_small_observation(
            root_observation, stage
        )

        stage = "guest-resolution-probe-push"
        file_push_invocations += 1
        with (request.repository_root / PROBE_RELATIVE_PATH).open(
            "rb"
        ) as probe_source:
            push_observation = command_runner.run(
                (
                    "utmctl",
                    "file",
                    "push",
                    request.target_uuid,
                    request.guest_probe_incoming,
                ),
                request.command_timeout_seconds,
                stdin_file=probe_source,
            )
        writer.write_json(
            "guest-resolution-probe-push.json", push_observation.as_json()
        )
        input_transfer._require_successful_small_observation(
            push_observation, stage
        )

        stage = "guest-resolution-probe-normalize"
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
                request.guest_probe_incoming,
                request.guest_probe_path,
            ),
            request.command_timeout_seconds,
        )
        writer.write_json(
            "guest-resolution-probe-normalize.json",
            normalize_observation.as_json(),
        )
        input_transfer._require_successful_small_observation(
            normalize_observation, stage
        )

        stage = "guest-resolution-probe-readback"
        file_pull_invocations += 1
        probe_readback = command_runner.run(
            (
                "utmctl",
                "file",
                "pull",
                request.target_uuid,
                request.guest_probe_path,
            ),
            request.command_timeout_seconds,
        )
        writer.write_json(
            "guest-resolution-probe-readback.json", probe_readback.as_json()
        )
        input_transfer._require_exact_small_readback(
            probe_readback, probe_bytes, stage
        )

        stage = "guest-resolution-probe"
        guest_exec_invocations += 1
        probe_invocations = 1
        probe_observation = command_runner.run(
            probe_argv(
                request,
                installer_size=len(binding.installer_bytes),
                installer_sha256=hashlib.sha256(
                    binding.installer_bytes
                ).hexdigest(),
                transfer_evidence_size=transfer_evidence_size,
                transfer_evidence_sha256=transfer_evidence_sha256,
            ),
            request.probe_timeout_seconds,
        )
        writer.write_json(
            "guest-resolution-probe.json", probe_observation.as_json()
        )
        if probe_observation.timed_out:
            raise CanonicalInputResolutionError(
                "guest-resolution-probe-timed-out"
            )
        if (
            probe_observation.stdout.truncated
            or probe_observation.stderr.truncated
        ):
            raise CanonicalInputResolutionError(
                "guest-resolution-probe-output-truncated"
            )
        if (
            probe_observation.stdout.total_bytes != 0
            or probe_observation.stderr.total_bytes != 0
        ):
            raise CanonicalInputResolutionError(
                "guest-resolution-probe-output-not-empty"
            )

        sleeper(float(request.probe_settle_seconds))
        probe_readbacks: list[start_control.CommandObservation] = []
        for index in (1, 2):
            stage = f"guest-resolution-evidence-readback-{index}"
            probe_result_readback_invocations += 1
            file_pull_invocations += 1
            observation = command_runner.run(
                (
                    "utmctl",
                    "file",
                    "pull",
                    request.target_uuid,
                    request.guest_probe_evidence_path,
                ),
                request.command_timeout_seconds,
            )
            writer.write_json(
                f"guest-resolution-evidence-readback-{index}.json",
                observation.as_json(),
            )
            input_transfer._require_successful_small_observation(
                observation, stage
            )
            if observation.stdout.total_bytes == 0:
                raise CanonicalInputResolutionError(f"{stage}-empty")
            probe_readbacks.append(observation)
        probe_payload = _require_double_readback(
            probe_readbacks, "guest-resolution-evidence"
        )
        parsed_probe_evidence = resolution_evidence.parse_probe_evidence(
            probe_payload, request
        )
        writer.write_json(
            "guest-resolution-evidence.json", parsed_probe_evidence
        )
        expected_probe_exit_code = (
            resolution_probe.EXIT_PASSED
            if parsed_probe_evidence["outcome"] == "passed"
            else resolution_probe.EXIT_INDETERMINATE
        )
        if probe_observation.exit_code != expected_probe_exit_code:
            raise CanonicalInputResolutionError(
                "guest-resolution-probe-exit-evidence-mismatch"
            )

        if parsed_probe_evidence["outcome"] != "passed":
            raise CanonicalInputResolutionError(
                "guest-resolution-probe-indeterminate"
            )
        if parsed_transfer_evidence["outcome"] == "passed":
            if (
                parsed_probe_evidence["final_input_root_state"]
                != "private-directory"
                or parsed_probe_evidence["staging_root_state"] != "absent"
            ):
                raise CanonicalInputResolutionError(
                    "passed-transfer-path-state-invalid"
                )
            outcome = "input-ready"
            exit_code = EXIT_INPUT_READY
            reason = "stable-transfer-result-and-read-only-probe-passed"
        elif parsed_transfer_evidence["final_switch"] == "not-performed":
            if parsed_probe_evidence["final_input_root_state"] != "absent":
                raise CanonicalInputResolutionError(
                    "failed-transfer-final-root-not-absent"
                )
            outcome = "transfer-failed-closed"
            exit_code = EXIT_TRANSFER_FAILED_CLOSED
            reason = (
                "stable-transfer-failure-before-switch-and-paths-observed"
            )
        else:
            raise CanonicalInputResolutionError(
                "transfer-result-semantics-indeterminate"
            )
    except (
        CanonicalInputResolutionError,
        input_transfer.CanonicalInputTransferError,
        network_ready.NetworkReadyError,
        resolution_probe.ResolutionProbeError,
        start_control.StartControlError,
        transport_bindings.BindingError,
        OSError,
        tarfile.TarError,
        ValueError,
    ) as exc:
        reason = f"{stage}:{exc}"
        if resolution_observation_started:
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
                    if resolution_observation_started
                    else "precondition-rejected"
                )
                exit_code = (
                    EXIT_STATE_INDETERMINATE
                    if resolution_observation_started
                    else EXIT_PRECONDITION_REJECTED
                )
            source_bundle.file_object.close()

    terminal = {
        "automatic_cleanup": "not-performed",
        "automatic_quit": "not-performed",
        "automatic_retry": "not-performed",
        "automatic_stop": "not-performed",
        "bundle_push_invocations": 0,
        "file_pull_invocations": file_pull_invocations,
        "file_push_invocations": file_push_invocations,
        "format": EVIDENCE_FORMAT,
        "guest_exec_invocations": guest_exec_invocations,
        "guest_probe_outcome": (
            parsed_probe_evidence.get("outcome")
            if parsed_probe_evidence
            else None
        ),
        "guest_transfer_outcome": (
            parsed_transfer_evidence.get("outcome")
            if parsed_transfer_evidence
            else None
        ),
        "installer_invocations": 0,
        "operation_id": "not-generated",
        "outcome": outcome,
        "plain_utmctl_list": "not-performed",
        "plain_utmctl_start": "not-performed",
        "plain_utmctl_status": "not-performed",
        "probe_invocations": probe_invocations,
        "probe_result_readback_invocations": (
            probe_result_readback_invocations
        ),
        "reason": reason,
        "resolution_attempt_id": request.resolution_attempt_id,
        "source_bundle_sha256": request.source_bundle_sha256,
        "source_bundle_size": request.source_bundle_size,
        "target_name": request.target_name,
        "target_uuid": request.target_uuid,
        "transaction": "not-performed",
        "transfer_attempt_id": request.transfer_attempt_id,
        "transfer_result_readback_invocations": (
            transfer_result_readback_invocations
        ),
    }
    writer.write_json("terminal.json", terminal)
    manifest_sha256 = writer.write_manifest()
    return CanonicalInputResolutionResult(
        outcome=outcome,
        exit_code=exit_code,
        evidence_root=request.output_root,
        manifest_sha256=manifest_sha256,
        transfer_result_readback_invocations=(
            transfer_result_readback_invocations
        ),
        probe_invocations=probe_invocations,
    )


def validate_resolution_bindings(
    request: CanonicalInputResolutionRequest,
) -> resolution_bindings.ResolutionBinding:
    if Path(__file__).absolute() != request.repository_root / CONTROL_RELATIVE_PATH:
        raise CanonicalInputResolutionError("executed-control-path-mismatch")
    for module_path, relative_path, label in (
        (
            Path(resolution_bindings.__file__).absolute(),
            BINDINGS_RELATIVE_PATH,
            "bindings",
        ),
        (
            Path(resolution_probe.__file__).absolute(),
            PROBE_RELATIVE_PATH,
            "probe",
        ),
        (
            Path(resolution_evidence.__file__).absolute(),
            EVIDENCE_RELATIVE_PATH,
            "evidence",
        ),
    ):
        if module_path != request.repository_root / relative_path:
            raise CanonicalInputResolutionError(
                f"executed-{label}-path-mismatch"
            )
    try:
        return resolution_bindings.validate_resolution_bindings(request)
    except ValueError as exc:
        raise CanonicalInputResolutionError(str(exc)) from exc


def probe_argv(
    request: CanonicalInputResolutionRequest,
    *,
    installer_size: int,
    installer_sha256: str,
    transfer_evidence_size: int,
    transfer_evidence_sha256: str,
) -> tuple[str, ...]:
    return (
        "utmctl",
        "exec",
        request.target_uuid,
        "--cmd",
        "/usr/bin/python3",
        request.guest_probe_path,
        "--resolution-attempt-id",
        request.resolution_attempt_id,
        "--transfer-attempt-id",
        request.transfer_attempt_id,
        "--resolution-root",
        request.guest_resolution_root,
        "--transfer-root",
        request.guest_transfer_root,
        "--expected-installer-size",
        str(installer_size),
        "--expected-installer-sha256",
        installer_sha256,
        "--expected-bundle-size",
        str(request.source_bundle_size),
        "--expected-bundle-sha256",
        request.source_bundle_sha256,
        "--expected-transfer-evidence-size",
        str(transfer_evidence_size),
        "--expected-transfer-evidence-sha256",
        transfer_evidence_sha256,
    )


def _require_double_readback(
    observations: list[start_control.CommandObservation], label: str
) -> bytes:
    if len(observations) != 2:
        raise CanonicalInputResolutionError(f"{label}-count-invalid")
    first, second = observations
    if (
        first.stdout.total_bytes != second.stdout.total_bytes
        or first.stdout.sha256 != second.stdout.sha256
        or first.stdout.prefix != second.stdout.prefix
    ):
        raise CanonicalInputResolutionError(f"{label}-drift")
    return first.stdout.prefix


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
            "Resolve one frozen L6 canonical input transfer from its existing "
            "result plus a create-new read-only guest observation probe."
        )
    )
    parser.add_argument("command", choices=("resolve",))
    parser.add_argument("--repository-root", type=Path, required=True)
    parser.add_argument("--expected-repository-head", required=True)
    parser.add_argument("--prior-network-root", type=Path, required=True)
    parser.add_argument("--prior-network-manifest-sha256", required=True)
    parser.add_argument("--prior-transfer-root", type=Path, required=True)
    parser.add_argument("--prior-transfer-manifest-sha256", required=True)
    parser.add_argument("--source-bundle-path", type=Path, required=True)
    parser.add_argument("--source-bundle-size", type=int, required=True)
    parser.add_argument("--source-bundle-sha256", required=True)
    parser.add_argument("--output-root", type=Path, required=True)
    parser.add_argument("--transfer-attempt-id", required=True)
    parser.add_argument("--resolution-attempt-id", required=True)
    parser.add_argument("--target-uuid", required=True)
    parser.add_argument("--target-name", required=True)
    parser.add_argument("--target-package-path", type=Path, required=True)
    parser.add_argument("--command-timeout-seconds", type=int, default=60)
    parser.add_argument("--probe-timeout-seconds", type=int, default=120)
    parser.add_argument("--probe-settle-seconds", type=int, default=10)
    parser.add_argument(
        "--authorized-canonical-input-resolution", action="store_true"
    )
    parser.add_argument(
        "--authorized-existing-result-double-readback", action="store_true"
    )
    parser.add_argument(
        "--authorized-create-new-read-only-probe", action="store_true"
    )
    parser.add_argument(
        "--authorized-no-transfer-operation-transaction-stop-or-retry",
        action="store_true",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    request = CanonicalInputResolutionRequest(
        repository_root=args.repository_root,
        expected_repository_head=args.expected_repository_head,
        prior_network_root=args.prior_network_root,
        prior_network_manifest_sha256=args.prior_network_manifest_sha256,
        prior_transfer_root=args.prior_transfer_root,
        prior_transfer_manifest_sha256=args.prior_transfer_manifest_sha256,
        source_bundle_path=args.source_bundle_path,
        source_bundle_size=args.source_bundle_size,
        source_bundle_sha256=args.source_bundle_sha256,
        output_root=args.output_root,
        transfer_attempt_id=args.transfer_attempt_id,
        resolution_attempt_id=args.resolution_attempt_id,
        target_uuid=args.target_uuid,
        target_name=args.target_name,
        target_package_path=args.target_package_path,
        command_timeout_seconds=args.command_timeout_seconds,
        probe_timeout_seconds=args.probe_timeout_seconds,
        probe_settle_seconds=args.probe_settle_seconds,
        authorized_canonical_input_resolution=(
            args.authorized_canonical_input_resolution
        ),
        authorized_existing_result_double_readback=(
            args.authorized_existing_result_double_readback
        ),
        authorized_create_new_read_only_probe=(
            args.authorized_create_new_read_only_probe
        ),
        authorized_no_transfer_operation_transaction_stop_or_retry=(
            args.authorized_no_transfer_operation_transaction_stop_or_retry
        ),
    )
    try:
        result = run_canonical_input_resolution(request)
    except (
        CanonicalInputResolutionError,
        input_transfer.CanonicalInputTransferError,
        network_ready.NetworkReadyError,
        resolution_probe.ResolutionProbeError,
        start_control.StartControlError,
        transport_bindings.BindingError,
        OSError,
        tarfile.TarError,
        ValueError,
    ) as exc:
        print(f"l6_utm_canonical_input_resolution_error={exc}", file=sys.stderr)
        return EXIT_PRECONDITION_REJECTED
    print(f"canonical_input_resolution_outcome={result.outcome}")
    print(f"evidence_root={result.evidence_root}")
    print(f"files_sha256={result.manifest_sha256}")
    return result.exit_code


if __name__ == "__main__":
    raise SystemExit(main())
