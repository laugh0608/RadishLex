#!/usr/bin/env python3
from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
import tarfile
import time
import uuid
from dataclasses import dataclass
from pathlib import Path
from types import SimpleNamespace
from typing import Callable, Protocol

import l6_utm_canonical_input_preflight_bindings as preflight_bindings
import l6_utm_canonical_input_resolution as input_resolution
import l6_utm_canonical_input_transfer as input_transfer
import l6_utm_guest_network_ready as network_ready
import l6_utm_launch_transport_bindings as transport_bindings
import l6_utm_launch_transport_v2 as launch_transport
import l6_utm_start_once as start_control
import l6_v4_canonical_input_install as guest_installer
import l6_v4_canonical_input_preflight_probe as preflight_probe


EVIDENCE_FORMAT = preflight_bindings.EVIDENCE_FORMAT
CONTROL_RELATIVE_PATH = preflight_bindings.CONTROL_RELATIVE_PATH
BINDINGS_RELATIVE_PATH = preflight_bindings.BINDINGS_RELATIVE_PATH
PROBE_RELATIVE_PATH = preflight_bindings.PROBE_RELATIVE_PATH
REQUIRED_PRIOR_NETWORK_MANIFEST_SHA256 = (
    input_transfer.REQUIRED_PRIOR_NETWORK_MANIFEST_SHA256
)
REQUIRED_PRIOR_TRANSFER_MANIFEST_SHA256 = (
    input_resolution.REQUIRED_PRIOR_TRANSFER_MANIFEST_SHA256
)
REQUIRED_PRIOR_RESOLUTION_MANIFEST_SHA256 = (
    preflight_bindings.REQUIRED_PRIOR_RESOLUTION_MANIFEST_SHA256
)
REQUIRED_TRANSFER_ATTEMPT_ID = input_resolution.REQUIRED_TRANSFER_ATTEMPT_ID
REQUIRED_RESOLUTION_ATTEMPT_ID = (
    preflight_bindings.REQUIRED_RESOLUTION_ATTEMPT_ID
)
REQUIRED_SOURCE_BUNDLE_SIZE = input_resolution.REQUIRED_SOURCE_BUNDLE_SIZE
REQUIRED_SOURCE_BUNDLE_SHA256 = input_resolution.REQUIRED_SOURCE_BUNDLE_SHA256
REQUIRED_TARGET_UUID = input_resolution.REQUIRED_TARGET_UUID
REQUIRED_TARGET_NAME = input_resolution.REQUIRED_TARGET_NAME
PROCESS_COMMAND = launch_transport.PROCESS_COMMAND
EXIT_PREFLIGHT_READY = 0
EXIT_PREFLIGHT_REJECTED = 10
EXIT_PRECONDITION_REJECTED = 11
EXIT_STATE_INDETERMINATE = 12
HEX_40 = re.compile(r"[0-9a-f]{40}")
HEX_64 = re.compile(r"[0-9a-f]{64}")
SAFE_ATTEMPT_ID = re.compile(r"[a-z0-9][a-z0-9-]{0,63}")


class CanonicalInputPreflightError(ValueError):
    pass


@dataclass(frozen=True)
class CanonicalInputPreflightRequest:
    repository_root: Path
    expected_repository_head: str
    prior_network_root: Path
    prior_network_manifest_sha256: str
    prior_transfer_root: Path
    prior_transfer_manifest_sha256: str
    prior_resolution_root: Path
    prior_resolution_manifest_sha256: str
    source_bundle_path: Path
    source_bundle_size: int
    source_bundle_sha256: str
    output_root: Path
    transfer_attempt_id: str
    resolution_attempt_id: str
    preflight_attempt_id: str
    target_uuid: str
    target_name: str
    target_package_path: Path
    command_timeout_seconds: int
    preflight_timeout_seconds: int
    evidence_settle_seconds: int
    authorized_canonical_input_negative_preflight: bool
    authorized_existing_resolution_double_readback: bool
    authorized_create_new_guest_preflight_root: bool
    authorized_read_only_startup_without_case_maintenance_acceptance_or_dpkg: bool

    @property
    def guest_resolution_root(self) -> str:
        return (
            "/var/tmp/radishlex-l6-v4-input-resolution-"
            f"{self.resolution_attempt_id}"
        )

    @property
    def guest_resolution_evidence_path(self) -> str:
        return f"{self.guest_resolution_root}/resolution.evidence.json"

    @property
    def guest_preflight_root(self) -> str:
        return (
            "/var/tmp/radishlex-l6-v4-negative-preflight-"
            f"{self.preflight_attempt_id}"
        )

    @property
    def guest_probe_incoming(self) -> str:
        return f"{self.guest_preflight_root}/negative-preflight.incoming.py"

    @property
    def guest_probe_path(self) -> str:
        return f"{self.guest_preflight_root}/negative-preflight.py"

    @property
    def guest_marker_path(self) -> str:
        return f"{self.guest_preflight_root}/attempt.marker.json"

    @property
    def guest_phase_path(self) -> str:
        return f"{self.guest_preflight_root}/phase.json"

    @property
    def guest_evidence_path(self) -> str:
        return f"{self.guest_preflight_root}/negative-preflight.evidence.json"

    def validate(self) -> None:
        for path, label in (
            (self.repository_root, "repository-root"),
            (self.prior_network_root, "prior-network-root"),
            (self.prior_transfer_root, "prior-transfer-root"),
            (self.prior_resolution_root, "prior-resolution-root"),
            (self.source_bundle_path, "source-bundle-path"),
            (self.output_root, "output-root"),
            (self.target_package_path, "target-package-path"),
        ):
            if not path.is_absolute() or ".." in path.parts:
                raise CanonicalInputPreflightError(
                    f"{label}-must-be-absolute-normalized"
                )
        for protected, label in (
            (self.repository_root, "repository"),
            (self.prior_network_root, "prior-network"),
            (self.prior_transfer_root, "prior-transfer"),
            (self.prior_resolution_root, "prior-resolution"),
            (self.source_bundle_path, "source-bundle"),
            (self.target_package_path, "target"),
        ):
            if _paths_overlap(self.output_root, protected):
                raise CanonicalInputPreflightError(
                    f"output-root-must-not-overlap-{label}"
                )
        if not HEX_40.fullmatch(self.expected_repository_head):
            raise CanonicalInputPreflightError(
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
                self.source_bundle_sha256,
                REQUIRED_SOURCE_BUNDLE_SHA256,
                "source-bundle-sha256",
            ),
        )
        for actual, expected, label in fixed:
            if actual != expected:
                raise CanonicalInputPreflightError(f"required-{label}-mismatch")
        if self.source_bundle_size != REQUIRED_SOURCE_BUNDLE_SIZE:
            raise CanonicalInputPreflightError(
                "required-source-bundle-size-mismatch"
            )
        if self.transfer_attempt_id != REQUIRED_TRANSFER_ATTEMPT_ID:
            raise CanonicalInputPreflightError(
                "required-transfer-attempt-id-mismatch"
            )
        if self.resolution_attempt_id != REQUIRED_RESOLUTION_ATTEMPT_ID:
            raise CanonicalInputPreflightError(
                "required-resolution-attempt-id-mismatch"
            )
        if (
            not SAFE_ATTEMPT_ID.fullmatch(self.preflight_attempt_id)
            or self.preflight_attempt_id
            in {self.transfer_attempt_id, self.resolution_attempt_id}
        ):
            raise CanonicalInputPreflightError("preflight-attempt-id-invalid")
        try:
            canonical_uuid = str(uuid.UUID(self.target_uuid)).upper()
        except ValueError as exc:
            raise CanonicalInputPreflightError("target-uuid-invalid") from exc
        if canonical_uuid != self.target_uuid or self.target_uuid != REQUIRED_TARGET_UUID:
            raise CanonicalInputPreflightError("required-target-uuid-mismatch")
        if self.target_name != REQUIRED_TARGET_NAME:
            raise CanonicalInputPreflightError("required-target-name-mismatch")
        if self.target_package_path != transport_bindings.expected_target_package_path(
            self.target_name
        ):
            raise CanonicalInputPreflightError("target-package-path-mismatch")
        for value, low, high, label in (
            (self.command_timeout_seconds, 1, 60, "command-timeout"),
            (self.preflight_timeout_seconds, 1, 300, "preflight-timeout"),
            (self.evidence_settle_seconds, 1, 60, "evidence-settle"),
        ):
            if not low <= value <= high:
                raise CanonicalInputPreflightError(f"{label}-out-of-range")
        for allowed, reason in (
            (
                self.authorized_canonical_input_negative_preflight,
                "authorized-canonical-input-negative-preflight-required",
            ),
            (
                self.authorized_existing_resolution_double_readback,
                "authorized-existing-resolution-double-readback-required",
            ),
            (
                self.authorized_create_new_guest_preflight_root,
                "authorized-create-new-guest-preflight-root-required",
            ),
            (
                self.authorized_read_only_startup_without_case_maintenance_acceptance_or_dpkg,
                "authorized-read-only-startup-without-case-maintenance-acceptance-or-dpkg-required",
            ),
        ):
            if not allowed:
                raise CanonicalInputPreflightError(reason)

    def as_json(self) -> dict[str, object]:
        return {
            "authorization": {
                "canonical_input_negative_preflight": True,
                "create_new_guest_preflight_root": True,
                "existing_resolution_double_readback": True,
                "read_only_startup_without_case_maintenance_acceptance_or_dpkg": True,
            },
            "command_timeout_seconds": self.command_timeout_seconds,
            "evidence_settle_seconds": self.evidence_settle_seconds,
            "expected_repository_head": self.expected_repository_head,
            "format": EVIDENCE_FORMAT,
            "guest_final_input_root": str(guest_installer.FINAL_INPUT_ROOT),
            "guest_preflight_root": self.guest_preflight_root,
            "guest_resolution_root": self.guest_resolution_root,
            "preflight_attempt_id": self.preflight_attempt_id,
            "preflight_timeout_seconds": self.preflight_timeout_seconds,
            "prior_network_manifest_sha256": self.prior_network_manifest_sha256,
            "prior_resolution_manifest_sha256": (
                self.prior_resolution_manifest_sha256
            ),
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
class CanonicalInputPreflightResult:
    outcome: str
    exit_code: int
    evidence_root: Path
    manifest_sha256: str
    preflight_invocations: int


class CommandRunner(Protocol):
    def run(
        self,
        argv: tuple[str, ...],
        timeout_seconds: int,
        *,
        stdin_file=None,
    ) -> start_control.CommandObservation: ...


Sleeper = Callable[[float], None]


def run_canonical_input_preflight(
    request: CanonicalInputPreflightRequest,
    *,
    runner: CommandRunner | None = None,
    binding_validator=None,
    source_opener=None,
    source_revalidator=None,
    target_validator=None,
    sleeper: Sleeper = time.sleep,
) -> CanonicalInputPreflightResult:
    request.validate()
    writer = start_control.EvidenceWriter.create(request.output_root)
    writer.write_json("request.json", request.as_json())
    command_runner = runner or input_transfer.SubprocessCommandRunner()
    validate_bindings = binding_validator or validate_preflight_bindings
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
    preflight_invocations = 0
    parsed_evidence: dict[str, object] | None = None
    source_bundle: input_transfer.SourceBundle | None = None

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
            raise CanonicalInputPreflightError(
                "utmctl-process-active-before-negative-preflight"
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
            writer.write_json(f"{stage}.json", observation.as_json())
            input_transfer._require_exact_small_readback(
                observation, binding.network.guest_evidence_bytes, stage
            )

        resolution_readbacks: list[start_control.CommandObservation] = []
        for index in (1, 2):
            stage = f"existing-resolution-evidence-readback-{index}"
            file_pull_invocations += 1
            observation = command_runner.run(
                (
                    "utmctl",
                    "file",
                    "pull",
                    request.target_uuid,
                    request.guest_resolution_evidence_path,
                ),
                request.command_timeout_seconds,
            )
            writer.write_json(f"{stage}.json", observation.as_json())
            input_transfer._require_exact_small_readback(
                observation, binding.resolution_evidence_bytes, stage
            )
            resolution_readbacks.append(observation)
        _require_double_readback(
            resolution_readbacks, "existing-resolution-evidence"
        )

        stage = "guest-preflight-root-create"
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
                request.guest_preflight_root,
            ),
            request.command_timeout_seconds,
        )
        writer.write_json(f"{stage}.json", root_observation.as_json())
        input_transfer._require_successful_small_observation(
            root_observation, stage
        )

        stage = "guest-preflight-probe-push"
        file_push_invocations += 1
        with (request.repository_root / PROBE_RELATIVE_PATH).open("rb") as source:
            push_observation = command_runner.run(
                (
                    "utmctl",
                    "file",
                    "push",
                    request.target_uuid,
                    request.guest_probe_incoming,
                ),
                request.command_timeout_seconds,
                stdin_file=source,
            )
        writer.write_json(f"{stage}.json", push_observation.as_json())
        input_transfer._require_successful_small_observation(push_observation, stage)

        normalization_commands = (
            (
                "guest-preflight-probe-chown",
                ("/bin/chown", "root:root", request.guest_probe_incoming),
            ),
            (
                "guest-preflight-probe-chmod",
                ("/bin/chmod", "0600", request.guest_probe_incoming),
            ),
            (
                "guest-preflight-probe-publish",
                (
                    "/bin/mv",
                    "--",
                    request.guest_probe_incoming,
                    request.guest_probe_path,
                ),
            ),
        )
        for stage, command in normalization_commands:
            guest_exec_invocations += 1
            observation = command_runner.run(
                ("utmctl", "exec", request.target_uuid, "--cmd", *command),
                request.command_timeout_seconds,
            )
            writer.write_json(f"{stage}.json", observation.as_json())
            input_transfer._require_successful_small_observation(observation, stage)

        stage = "guest-preflight-probe-readback"
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
        writer.write_json(f"{stage}.json", probe_readback.as_json())
        input_transfer._require_exact_small_readback(
            probe_readback, binding.probe_bytes, stage
        )

        stage = "guest-negative-preflight"
        guest_exec_invocations += 1
        preflight_invocations = 1
        probe_observation = command_runner.run(
            probe_argv(request, binding), request.preflight_timeout_seconds
        )
        writer.write_json(f"{stage}.json", probe_observation.as_json())
        if probe_observation.timed_out:
            raise CanonicalInputPreflightError("guest-negative-preflight-timed-out")
        if probe_observation.stdout.truncated or probe_observation.stderr.truncated:
            raise CanonicalInputPreflightError(
                "guest-negative-preflight-output-truncated"
            )
        if probe_observation.stdout.total_bytes or probe_observation.stderr.total_bytes:
            raise CanonicalInputPreflightError(
                "guest-negative-preflight-output-not-empty"
            )

        sleeper(float(request.evidence_settle_seconds))
        stage = "guest-preflight-marker-readback"
        file_pull_invocations += 1
        marker_observation = command_runner.run(
            (
                "utmctl",
                "file",
                "pull",
                request.target_uuid,
                request.guest_marker_path,
            ),
            request.command_timeout_seconds,
        )
        writer.write_json(f"{stage}.json", marker_observation.as_json())
        marker = preflight_probe.expected_marker_bytes(
            request.preflight_attempt_id,
            hashlib.sha256(binding.probe_bytes).hexdigest(),
        )
        input_transfer._require_exact_small_readback(
            marker_observation, marker, stage
        )

        evidence_readbacks: list[start_control.CommandObservation] = []
        for index in (1, 2):
            stage = f"guest-negative-preflight-evidence-readback-{index}"
            file_pull_invocations += 1
            observation = command_runner.run(
                (
                    "utmctl",
                    "file",
                    "pull",
                    request.target_uuid,
                    request.guest_evidence_path,
                ),
                request.command_timeout_seconds,
            )
            writer.write_json(f"{stage}.json", observation.as_json())
            input_transfer._require_successful_small_observation(observation, stage)
            if observation.stdout.total_bytes == 0:
                raise CanonicalInputPreflightError(f"{stage}-empty")
            evidence_readbacks.append(observation)
        evidence_payload = _require_double_readback(
            evidence_readbacks, "guest-negative-preflight-evidence"
        )
        bound_boot_sha = binding.evidence["prior_network_boot_id_sha256"]
        if not isinstance(bound_boot_sha, str):
            raise CanonicalInputPreflightError("bound-boot-id-sha256-invalid")
        parsed_evidence = parse_preflight_evidence(
            evidence_payload, request, bound_boot_sha
        )
        writer.write_json("guest-negative-preflight-evidence.json", parsed_evidence)

        stage = "guest-preflight-phase-readback"
        file_pull_invocations += 1
        phase_observation = command_runner.run(
            (
                "utmctl",
                "file",
                "pull",
                request.target_uuid,
                request.guest_phase_path,
            ),
            request.command_timeout_seconds,
        )
        writer.write_json(f"{stage}.json", phase_observation.as_json())
        expected_phase = preflight_probe.canonical_json(
            {
                "format": preflight_probe.EVIDENCE_FORMAT,
                "phase": parsed_evidence["phase"],
            }
        )
        input_transfer._require_exact_small_readback(
            phase_observation, expected_phase, stage
        )

        stage = "network-evidence-postflight-readback"
        file_pull_invocations += 1
        network_postflight = command_runner.run(
            (
                "utmctl",
                "file",
                "pull",
                request.target_uuid,
                binding.network.guest_evidence_path,
            ),
            request.command_timeout_seconds,
        )
        writer.write_json(f"{stage}.json", network_postflight.as_json())
        input_transfer._require_exact_small_readback(
            network_postflight, binding.network.guest_evidence_bytes, stage
        )

        expected_probe_exit = (
            preflight_probe.EXIT_PASSED
            if parsed_evidence["outcome"] == "passed"
            else preflight_probe.EXIT_REJECTED
        )
        if probe_observation.exit_code != expected_probe_exit:
            raise CanonicalInputPreflightError(
                "guest-negative-preflight-exit-evidence-mismatch"
            )
        if parsed_evidence["outcome"] == "passed":
            outcome = "preflight-ready"
            exit_code = EXIT_PREFLIGHT_READY
            reason = "one-shot-negative-preflight-passed"
        else:
            outcome = "preflight-rejected"
            exit_code = EXIT_PREFLIGHT_REJECTED
            reason = f"guest-negative-preflight-rejected:{parsed_evidence['reason']}"
    except (
        CanonicalInputPreflightError,
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
        "acceptance_invocations": 0,
        "automatic_cleanup": "not-performed",
        "automatic_quit": "not-performed",
        "automatic_retry": "not-performed",
        "automatic_stop": "not-performed",
        "case_invocations": 0,
        "dpkg_invocations": 0,
        "file_pull_invocations": file_pull_invocations,
        "file_push_invocations": file_push_invocations,
        "format": EVIDENCE_FORMAT,
        "guest_exec_invocations": guest_exec_invocations,
        "guest_preflight_outcome": (
            parsed_evidence.get("outcome") if parsed_evidence else None
        ),
        "installer_invocations": 0,
        "maintenance_invocations": 0,
        "operation_id": "not-generated",
        "outcome": outcome,
        "plain_utmctl_list": "not-performed",
        "plain_utmctl_start": "not-performed",
        "plain_utmctl_status": "not-performed",
        "preflight_attempt_id": request.preflight_attempt_id,
        "preflight_invocations": preflight_invocations,
        "reason": reason,
        "resolution_attempt_id": request.resolution_attempt_id,
        "source_bundle_sha256": request.source_bundle_sha256,
        "source_bundle_size": request.source_bundle_size,
        "target_name": request.target_name,
        "target_uuid": request.target_uuid,
        "transaction": "not-performed",
        "transfer_attempt_id": request.transfer_attempt_id,
    }
    writer.write_json("terminal.json", terminal)
    manifest_sha256 = writer.write_manifest()
    return CanonicalInputPreflightResult(
        outcome=outcome,
        exit_code=exit_code,
        evidence_root=request.output_root,
        manifest_sha256=manifest_sha256,
        preflight_invocations=preflight_invocations,
    )


def validate_preflight_bindings(
    request: CanonicalInputPreflightRequest,
) -> preflight_bindings.PreflightBinding:
    if Path(__file__).absolute() != request.repository_root / CONTROL_RELATIVE_PATH:
        raise CanonicalInputPreflightError("executed-control-path-mismatch")
    for module_path, expected, label in (
        (Path(preflight_bindings.__file__).absolute(), BINDINGS_RELATIVE_PATH, "bindings"),
        (Path(preflight_probe.__file__).absolute(), PROBE_RELATIVE_PATH, "probe"),
    ):
        if module_path != request.repository_root / expected:
            raise CanonicalInputPreflightError(f"executed-{label}-path-mismatch")
    try:
        return preflight_bindings.validate_preflight_bindings(request)
    except ValueError as exc:
        raise CanonicalInputPreflightError(str(exc)) from exc


def probe_argv(
    request: CanonicalInputPreflightRequest,
    binding: preflight_bindings.PreflightBinding,
) -> tuple[str, ...]:
    boot_sha = binding.evidence["prior_network_boot_id_sha256"]
    if not isinstance(boot_sha, str) or not HEX_64.fullmatch(boot_sha):
        raise CanonicalInputPreflightError("bound-boot-id-sha256-invalid")
    return (
        "utmctl",
        "exec",
        request.target_uuid,
        "--cmd",
        "/usr/bin/python3",
        "-B",
        request.guest_probe_path,
        "--preflight-attempt-id",
        request.preflight_attempt_id,
        "--transfer-attempt-id",
        request.transfer_attempt_id,
        "--resolution-attempt-id",
        request.resolution_attempt_id,
        "--preflight-root",
        request.guest_preflight_root,
        "--probe-path",
        request.guest_probe_path,
        "--expected-probe-size",
        str(len(binding.probe_bytes)),
        "--expected-probe-sha256",
        hashlib.sha256(binding.probe_bytes).hexdigest(),
        "--expected-boot-id-sha256",
        boot_sha,
    )


def parse_preflight_evidence(
    payload: bytes,
    request: CanonicalInputPreflightRequest,
    expected_boot_id_sha256: str,
) -> dict[str, object]:
    try:
        text = payload.decode("utf-8")
        value = json.loads(text)
    except (UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise CanonicalInputPreflightError(
            "negative-preflight-evidence-json-invalid"
        ) from exc
    expected_keys = set(
        preflight_probe.base_evidence(
            SimpleNamespace(
                preflight_attempt_id=request.preflight_attempt_id,
                resolution_attempt_id=request.resolution_attempt_id,
                transfer_attempt_id=request.transfer_attempt_id,
            )
        )
    )
    if (
        not text.endswith("\n")
        or "\r" in text
        or "\x00" in text
        or not isinstance(value, dict)
        or set(value) != expected_keys
    ):
        raise CanonicalInputPreflightError(
            "negative-preflight-evidence-shape-invalid"
        )
    fixed = {
        "acceptance_invocations": 0,
        "automatic_cleanup": "not-performed",
        "automatic_retry": "not-performed",
        "automatic_stop": "not-performed",
        "case_invocations": 0,
        "dpkg_invocations": 0,
        "final_input_root": str(guest_installer.FINAL_INPUT_ROOT),
        "format": preflight_probe.EVIDENCE_FORMAT,
        "maintenance_invocations": 0,
        "operation_id": "not-generated",
        "preflight_attempt_id": request.preflight_attempt_id,
        "resolution_attempt_id": request.resolution_attempt_id,
        "transaction": "not-performed",
        "transfer_attempt_id": request.transfer_attempt_id,
    }
    if any(value.get(key) != expected for key, expected in fixed.items()):
        raise CanonicalInputPreflightError(
            "negative-preflight-evidence-identity-invalid"
        )
    if value.get("outcome") == "passed":
        expected_passed = {
            "checkpoint_evidence": "absent",
            "dependency_count": 20,
            "dependency_identity": "matched",
            "dpkg_config_sha256": preflight_probe.EXPECTED_DPKG_CONFIG_SHA256,
            "dpkg_status_sha256": preflight_probe.EXPECTED_DPKG_STATUS_SHA256,
            "fcitx_startup": preflight_probe.EXPECTED_STARTUP_OUTPUT,
            "font_identity": "dejavu+noto-cjk|matched",
            "guard": "absent",
            "input_inventory_count": len(preflight_probe.INPUT_MEMBERS),
            "input_inventory_identity": "matched",
            "input_root_identity": "private-directory",
            "manager_startup": preflight_probe.EXPECTED_STARTUP_OUTPUT,
            "network": "loopback-only-main-routes-empty",
            "package": "not-installed",
            "phase": "complete",
            "product_processes": "absent",
            "reason": "none",
            "receipt_terminal": "absent",
            "release_pair_identity": "matched",
            "startup_reason": "ReceiptMissing",
            "state_root": "absent",
            "user_xdg": "absent",
        }
        if not HEX_64.fullmatch(expected_boot_id_sha256):
            raise CanonicalInputPreflightError(
                "negative-preflight-evidence-boot-binding-invalid"
            )
        expected_passed["boot_id_sha256"] = expected_boot_id_sha256
        if any(value.get(key) != expected for key, expected in expected_passed.items()):
            raise CanonicalInputPreflightError(
                "negative-preflight-evidence-passed-semantics-invalid"
            )
        if (
            not HEX_64.fullmatch(str(value.get("dpkg_log_sha256")))
            or not isinstance(value.get("dpkg_log_size"), int)
            or isinstance(value.get("dpkg_log_size"), bool)
            or value["dpkg_log_size"] < 0
        ):
            raise CanonicalInputPreflightError(
                "negative-preflight-evidence-dpkg-log-invalid"
            )
    elif value.get("outcome") == "predicate_failed":
        if (
            not isinstance(value.get("reason"), str)
            or value.get("reason") in {"", "none", "not-run"}
            or not isinstance(value.get("phase"), str)
            or value.get("phase") in {"", "complete"}
        ):
            raise CanonicalInputPreflightError(
                "negative-preflight-evidence-rejected-semantics-invalid"
            )
    else:
        raise CanonicalInputPreflightError(
            "negative-preflight-evidence-outcome-invalid"
        )
    return value


def _require_double_readback(
    observations: list[start_control.CommandObservation], label: str
) -> bytes:
    if len(observations) != 2:
        raise CanonicalInputPreflightError(f"{label}-count-invalid")
    first, second = observations
    if (
        first.stdout.total_bytes != second.stdout.total_bytes
        or first.stdout.sha256 != second.stdout.sha256
        or first.stdout.prefix != second.stdout.prefix
    ):
        raise CanonicalInputPreflightError(f"{label}-drift")
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
            "Run one create-new L6 canonical input negative preflight without "
            "case, maintenance, acceptance, dpkg, operation, transaction, retry, "
            "cleanup, or VM lifecycle actions."
        )
    )
    parser.add_argument("command", choices=("preflight",))
    parser.add_argument("--repository-root", type=Path, required=True)
    parser.add_argument("--expected-repository-head", required=True)
    parser.add_argument("--prior-network-root", type=Path, required=True)
    parser.add_argument("--prior-network-manifest-sha256", required=True)
    parser.add_argument("--prior-transfer-root", type=Path, required=True)
    parser.add_argument("--prior-transfer-manifest-sha256", required=True)
    parser.add_argument("--prior-resolution-root", type=Path, required=True)
    parser.add_argument("--prior-resolution-manifest-sha256", required=True)
    parser.add_argument("--source-bundle-path", type=Path, required=True)
    parser.add_argument("--source-bundle-size", type=int, required=True)
    parser.add_argument("--source-bundle-sha256", required=True)
    parser.add_argument("--output-root", type=Path, required=True)
    parser.add_argument("--transfer-attempt-id", required=True)
    parser.add_argument("--resolution-attempt-id", required=True)
    parser.add_argument("--preflight-attempt-id", required=True)
    parser.add_argument("--target-uuid", required=True)
    parser.add_argument("--target-name", required=True)
    parser.add_argument("--target-package-path", type=Path, required=True)
    parser.add_argument("--command-timeout-seconds", type=int, default=60)
    parser.add_argument("--preflight-timeout-seconds", type=int, default=120)
    parser.add_argument("--evidence-settle-seconds", type=int, default=10)
    parser.add_argument(
        "--authorized-canonical-input-negative-preflight", action="store_true"
    )
    parser.add_argument(
        "--authorized-existing-resolution-double-readback", action="store_true"
    )
    parser.add_argument(
        "--authorized-create-new-guest-preflight-root", action="store_true"
    )
    parser.add_argument(
        "--authorized-read-only-startup-without-case-maintenance-acceptance-or-dpkg",
        action="store_true",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    request = CanonicalInputPreflightRequest(
        repository_root=args.repository_root,
        expected_repository_head=args.expected_repository_head,
        prior_network_root=args.prior_network_root,
        prior_network_manifest_sha256=args.prior_network_manifest_sha256,
        prior_transfer_root=args.prior_transfer_root,
        prior_transfer_manifest_sha256=args.prior_transfer_manifest_sha256,
        prior_resolution_root=args.prior_resolution_root,
        prior_resolution_manifest_sha256=args.prior_resolution_manifest_sha256,
        source_bundle_path=args.source_bundle_path,
        source_bundle_size=args.source_bundle_size,
        source_bundle_sha256=args.source_bundle_sha256,
        output_root=args.output_root,
        transfer_attempt_id=args.transfer_attempt_id,
        resolution_attempt_id=args.resolution_attempt_id,
        preflight_attempt_id=args.preflight_attempt_id,
        target_uuid=args.target_uuid,
        target_name=args.target_name,
        target_package_path=args.target_package_path,
        command_timeout_seconds=args.command_timeout_seconds,
        preflight_timeout_seconds=args.preflight_timeout_seconds,
        evidence_settle_seconds=args.evidence_settle_seconds,
        authorized_canonical_input_negative_preflight=(
            args.authorized_canonical_input_negative_preflight
        ),
        authorized_existing_resolution_double_readback=(
            args.authorized_existing_resolution_double_readback
        ),
        authorized_create_new_guest_preflight_root=(
            args.authorized_create_new_guest_preflight_root
        ),
        authorized_read_only_startup_without_case_maintenance_acceptance_or_dpkg=(
            args.authorized_read_only_startup_without_case_maintenance_acceptance_or_dpkg
        ),
    )
    try:
        result = run_canonical_input_preflight(request)
    except (
        CanonicalInputPreflightError,
        input_transfer.CanonicalInputTransferError,
        network_ready.NetworkReadyError,
        start_control.StartControlError,
        transport_bindings.BindingError,
        OSError,
        tarfile.TarError,
        ValueError,
    ) as exc:
        print(f"l6_utm_canonical_input_preflight_error={exc}", file=sys.stderr)
        return EXIT_PRECONDITION_REJECTED
    print(f"canonical_input_preflight_outcome={result.outcome}")
    print(f"evidence_root={result.evidence_root}")
    print(f"files_sha256={result.manifest_sha256}")
    return result.exit_code


if __name__ == "__main__":
    raise SystemExit(main())
