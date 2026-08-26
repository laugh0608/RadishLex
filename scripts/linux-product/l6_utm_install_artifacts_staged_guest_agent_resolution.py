#!/usr/bin/env python3
from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
import time
import uuid
from dataclasses import dataclass
from pathlib import Path
from typing import Callable, Protocol

import l6_utm_canonical_input_transfer as input_transfer
import l6_utm_guest_network_ready as network_ready
import l6_utm_install_artifacts_staged_boot_start_bindings as boot_bindings
import l6_utm_install_artifacts_staged_boot_start_resolution as boot_control
import l6_utm_install_artifacts_staged_boot_transport_resolution as transport_control
import l6_utm_install_artifacts_staged_guest_agent_bindings as bindings
import l6_utm_install_artifacts_staged_reactivation as reactivation_control
import l6_utm_install_artifacts_staged_runtime_resolution as runtime_control
import l6_utm_install_artifacts_staged_runtime_resolution_bindings as runtime_bindings
import l6_utm_install_artifacts_staged_resume_evidence as resume_evidence
import l6_utm_start_once as start_control
import l6_v4_boot_transport_probe as guest_probe
import l6_v4_canonical_input_install as guest_installer


EVIDENCE_FORMAT = bindings.EVIDENCE_FORMAT
CONTROL_RELATIVE_PATH = bindings.CONTROL_RELATIVE_PATH
BINDINGS_RELATIVE_PATH = bindings.BINDINGS_RELATIVE_PATH
PROBE_RELATIVE_PATH = bindings.PROBE_RELATIVE_PATH
EXIT_ORIGINAL_BOOT_RESTORED = 0
EXIT_NEW_BOOT_STARTED = 10
EXIT_PRECONDITION_REJECTED = 11
EXIT_STATE_INDETERMINATE = 12
HEX_40 = re.compile(r"[0-9a-f]{40}")
HEX_64 = re.compile(r"[0-9a-f]{64}")
SAFE_ATTEMPT_ID = re.compile(r"[a-z0-9][a-z0-9-]{0,95}")


class GuestAgentResolutionError(ValueError):
    pass


@dataclass(frozen=True)
class GuestAgentResolutionRequest:
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
    prior_runtime_resolution_root: Path
    prior_runtime_resolution_manifest_sha256: str
    prior_runtime_resolution_v2_root: Path
    prior_runtime_resolution_v2_manifest_sha256: str
    prior_boot_transport_root: Path
    prior_boot_transport_manifest_sha256: str
    prior_boot_start_root: Path
    prior_boot_start_manifest_sha256: str
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
    prior_runtime_resolution_attempt_id: str
    runtime_resolution_attempt_id: str
    prior_runtime_resolution_v2_attempt_id: str
    boot_transport_attempt_id: str
    prior_boot_transport_attempt_id: str
    boot_start_attempt_id: str
    prior_boot_start_attempt_id: str
    guest_agent_attempt_id: str
    target_uuid: str
    target_name: str
    target_package_path: Path
    expected_vm_count: int
    identity_observations: int
    agent_readiness_attempts: int
    poll_interval_seconds: int
    command_timeout_seconds: int
    authorized_install_artifacts_staged_guest_agent_resolution: bool
    authorized_bounded_read_only_guest_agent_readiness: bool
    authorized_stable_target_handle_pid_observations: bool
    authorized_one_private_guest_probe_delivery_and_execution: bool
    authorized_two_independent_result_readbacks: bool
    authorized_no_list_status_start_resume_business_guest_retry_stop_or_quit: bool

    @property
    def guest_control_root(self) -> str:
        return (
            "/var/tmp/radishlex-l6-v4-guest-agent-"
            + self.guest_agent_attempt_id
        )

    @property
    def guest_probe_incoming(self) -> str:
        return f"{self.guest_control_root}/boot-transport-probe.incoming.py"

    @property
    def guest_probe_path(self) -> str:
        return f"{self.guest_control_root}/boot-transport-probe.py"

    @property
    def guest_marker_path(self) -> str:
        return f"{self.guest_control_root}/attempt.marker.json"

    @property
    def guest_result_path(self) -> str:
        return f"{self.guest_control_root}/boot-identity.evidence.json"

    def validate(self) -> None:
        paths = (
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
            (
                self.prior_runtime_resolution_root,
                "prior-runtime-resolution-root",
            ),
            (
                self.prior_runtime_resolution_v2_root,
                "prior-runtime-resolution-v2-root",
            ),
            (self.prior_boot_transport_root, "prior-boot-transport-root"),
            (self.prior_boot_start_root, "prior-boot-start-root"),
            (self.source_bundle_path, "source-bundle-path"),
            (self.output_root, "output-root"),
            (self.target_package_path, "target-package-path"),
        )
        for path, label in paths:
            if not path.is_absolute() or ".." in path.parts:
                raise GuestAgentResolutionError(
                    f"{label}-must-be-absolute-normalized"
                )
        for path, label in paths:
            if label != "output-root" and runtime_control._paths_overlap(
                self.output_root, path
            ):
                raise GuestAgentResolutionError(
                    f"output-root-must-not-overlap-{label}"
                )
        if not HEX_40.fullmatch(self.expected_repository_head):
            raise GuestAgentResolutionError("expected-repository-head-invalid")

        fixed = (
            (
                self.prior_reactivation_manifest_sha256,
                runtime_bindings.REQUIRED_PRIOR_REACTIVATION_MANIFEST_SHA256,
                "prior-reactivation-manifest",
            ),
            (
                self.prior_runtime_resolution_manifest_sha256,
                runtime_bindings.REQUIRED_PRIOR_RUNTIME_RESOLUTION_MANIFEST_SHA256,
                "prior-runtime-resolution-manifest",
            ),
            (
                self.prior_runtime_resolution_v2_manifest_sha256,
                boot_control.boot_bindings.REQUIRED_PRIOR_RUNTIME_RESOLUTION_V2_MANIFEST_SHA256,
                "prior-runtime-resolution-v2-manifest",
            ),
            (
                self.prior_boot_transport_manifest_sha256,
                boot_bindings.REQUIRED_PRIOR_BOOT_TRANSPORT_MANIFEST_SHA256,
                "prior-boot-transport-manifest",
            ),
            (
                self.prior_boot_start_manifest_sha256,
                bindings.REQUIRED_PRIOR_BOOT_START_MANIFEST_SHA256,
                "prior-boot-start-manifest",
            ),
            (
                self.source_bundle_sha256,
                reactivation_control.REQUIRED_SOURCE_BUNDLE_SHA256,
                "source-bundle-sha256",
            ),
        )
        for actual, expected, label in fixed:
            if actual != expected:
                raise GuestAgentResolutionError(f"required-{label}-mismatch")
        if (
            self.source_bundle_size
            != reactivation_control.REQUIRED_SOURCE_BUNDLE_SIZE
        ):
            raise GuestAgentResolutionError(
                "required-source-bundle-size-mismatch"
            )

        attempts = (
            (
                self.transfer_attempt_id,
                reactivation_control.REQUIRED_TRANSFER_ATTEMPT_ID,
                "transfer",
            ),
            (
                self.resolution_attempt_id,
                reactivation_control.REQUIRED_RESOLUTION_ATTEMPT_ID,
                "resolution",
            ),
            (
                self.preflight_attempt_id,
                reactivation_control.REQUIRED_PREFLIGHT_ATTEMPT_ID,
                "preflight",
            ),
            (
                self.checkpoint_attempt_id,
                reactivation_control.REQUIRED_CHECKPOINT_ATTEMPT_ID,
                "checkpoint",
            ),
            (
                self.resume_attempt_id,
                reactivation_control.REQUIRED_RESUME_ATTEMPT_ID,
                "resume",
            ),
            (
                self.backend_resolution_attempt_id,
                reactivation_control.REQUIRED_BACKEND_RESOLUTION_ATTEMPT_ID,
                "backend-resolution",
            ),
            (
                self.reactivation_attempt_id,
                reactivation_control.REQUIRED_REACTIVATION_ATTEMPT_ID,
                "reactivation",
            ),
            (
                self.prior_runtime_resolution_attempt_id,
                runtime_bindings.REQUIRED_PRIOR_RUNTIME_RESOLUTION_ATTEMPT_ID,
                "prior-runtime-resolution",
            ),
            (
                self.runtime_resolution_attempt_id,
                boot_control.boot_bindings.REQUIRED_PRIOR_RUNTIME_RESOLUTION_V2_ATTEMPT_ID,
                "runtime-resolution-v2",
            ),
            (
                self.prior_runtime_resolution_v2_attempt_id,
                boot_control.boot_bindings.REQUIRED_PRIOR_RUNTIME_RESOLUTION_V2_ATTEMPT_ID,
                "prior-runtime-resolution-v2",
            ),
            (
                self.boot_transport_attempt_id,
                boot_bindings.REQUIRED_PRIOR_BOOT_TRANSPORT_ATTEMPT_ID,
                "boot-transport",
            ),
            (
                self.prior_boot_transport_attempt_id,
                boot_bindings.REQUIRED_PRIOR_BOOT_TRANSPORT_ATTEMPT_ID,
                "prior-boot-transport",
            ),
            (
                self.boot_start_attempt_id,
                bindings.REQUIRED_PRIOR_BOOT_START_ATTEMPT_ID,
                "boot-start",
            ),
            (
                self.prior_boot_start_attempt_id,
                bindings.REQUIRED_PRIOR_BOOT_START_ATTEMPT_ID,
                "prior-boot-start",
            ),
            (
                self.guest_agent_attempt_id,
                bindings.REQUIRED_GUEST_AGENT_ATTEMPT_ID,
                "guest-agent",
            ),
        )
        for actual, expected, label in attempts:
            if actual != expected or not SAFE_ATTEMPT_ID.fullmatch(actual):
                raise GuestAgentResolutionError(
                    f"required-{label}-attempt-id-mismatch"
                )
        aliases = {
            "runtime-resolution-v2",
            "prior-boot-transport",
            "prior-boot-start",
        }
        distinct = [
            value
            for value, _expected, label in attempts
            if label not in aliases
        ]
        if len(set(distinct)) != len(distinct):
            raise GuestAgentResolutionError("attempt-id-overlap")

        try:
            canonical_uuid = str(uuid.UUID(self.target_uuid)).upper()
        except ValueError as exc:
            raise GuestAgentResolutionError("target-uuid-invalid") from exc
        if (
            canonical_uuid != self.target_uuid
            or self.target_uuid != reactivation_control.REQUIRED_TARGET_UUID
        ):
            raise GuestAgentResolutionError("required-target-uuid-mismatch")
        if self.target_name != reactivation_control.REQUIRED_TARGET_NAME:
            raise GuestAgentResolutionError("required-target-name-mismatch")
        expected_package = (
            reactivation_control.launch_bindings.expected_target_package_path(
                self.target_name
            )
        )
        if self.target_package_path != expected_package:
            raise GuestAgentResolutionError("target-package-path-mismatch")
        for actual, expected, label in (
            (self.expected_vm_count, 21, "expected-vm-count"),
            (self.identity_observations, 3, "identity-observations"),
            (
                self.agent_readiness_attempts,
                60,
                "agent-readiness-attempts",
            ),
        ):
            if actual != expected:
                raise GuestAgentResolutionError(
                    f"{label}-must-be-exactly-{expected}"
                )
        if not 1 <= self.poll_interval_seconds <= 10:
            raise GuestAgentResolutionError("poll-interval-out-of-range")
        if not 1 <= self.command_timeout_seconds <= 60:
            raise GuestAgentResolutionError("command-timeout-out-of-range")

        authorizations = (
            (
                self.authorized_install_artifacts_staged_guest_agent_resolution,
                "authorized-install-artifacts-staged-guest-agent-resolution-required",
            ),
            (
                self.authorized_bounded_read_only_guest_agent_readiness,
                "authorized-bounded-read-only-guest-agent-readiness-required",
            ),
            (
                self.authorized_stable_target_handle_pid_observations,
                "authorized-stable-target-handle-pid-observations-required",
            ),
            (
                self.authorized_one_private_guest_probe_delivery_and_execution,
                "authorized-one-private-guest-probe-delivery-and-execution-required",
            ),
            (
                self.authorized_two_independent_result_readbacks,
                "authorized-two-independent-result-readbacks-required",
            ),
            (
                self.authorized_no_list_status_start_resume_business_guest_retry_stop_or_quit,
                "authorized-no-list-status-start-resume-business-guest-retry-stop-or-quit-required",
            ),
        )
        for authorized, reason in authorizations:
            if not authorized:
                raise GuestAgentResolutionError(reason)

    def as_json(self) -> dict[str, object]:
        return {
            "agent_readiness_attempts": self.agent_readiness_attempts,
            "authorization": {
                "bounded_read_only_guest_agent_readiness": True,
                "install_artifacts_staged_guest_agent_resolution": True,
                "no_list_status_start_resume_business_guest_retry_stop_or_quit": True,
                "one_private_guest_probe_delivery_and_execution": True,
                "stable_target_handle_pid_observations": True,
                "two_independent_result_readbacks": True,
            },
            "command_timeout_seconds": self.command_timeout_seconds,
            "expected_repository_head": self.expected_repository_head,
            "format": EVIDENCE_FORMAT,
            "guest_agent_attempt_id": self.guest_agent_attempt_id,
            "guest_control_root": self.guest_control_root,
            "identity_observations": self.identity_observations,
            "poll_interval_seconds": self.poll_interval_seconds,
            "prior_boot_start_attempt_id": self.prior_boot_start_attempt_id,
            "prior_boot_start_manifest_sha256": (
                self.prior_boot_start_manifest_sha256
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
        }


@dataclass(frozen=True)
class GuestAgentResolutionResult:
    outcome: str
    exit_code: int
    evidence_root: Path
    manifest_sha256: str
    agent_readiness_invocations: int
    guest_probe_invocations: int
    result_readback_invocations: int


class CommandRunner(Protocol):
    def run(
        self,
        argv: tuple[str, ...],
        timeout_seconds: int,
        *,
        stdin_file=None,
    ) -> start_control.CommandObservation: ...


Sleeper = Callable[[float], None]


def run_guest_agent_resolution(
    request: GuestAgentResolutionRequest,
    *,
    runner: CommandRunner | None = None,
    binding_validator=None,
    source_opener=None,
    source_revalidator=None,
    target_validator=None,
    sleeper: Sleeper = time.sleep,
) -> GuestAgentResolutionResult:
    request.validate()
    writer = start_control.EvidenceWriter.create(request.output_root)
    writer.write_json("request.json", request.as_json())
    command_runner = runner or input_transfer.SubprocessCommandRunner()
    validate_bindings = binding_validator or validate_guest_agent_bindings
    open_source = source_opener or input_transfer.open_source_bundle
    revalidate_source = (
        source_revalidator or input_transfer.revalidate_open_source_bundle
    )
    validate_target = target_validator or network_ready.validate_target_files

    stage = "binding-preflight"
    reason = "not-run"
    outcome = "precondition-rejected"
    exit_code = EXIT_PRECONDITION_REJECTED
    agent_readiness_invocations = 0
    agent_readiness_state = "not-attempted"
    backend_pid: int | None = None
    identity_observation_count = 0
    readiness_identity_observation_count = 0
    guest_exec_invocations = 0
    guest_probe_invocations = 0
    file_push_invocations = 0
    file_pull_invocations = 0
    result_readback_invocations = 0
    target_handles_terminal = "not-observed"
    terminal_processes: tuple[dict[str, object], ...] = ()
    observed_boot_hash: str | None = None
    boot_classification = "not-generated"
    source_bundle: input_transfer.SourceBundle | None = None
    target_preflight: dict[str, object] | None = None

    def pull(path: str, name: str, *, exact: bytes | None = None) -> bytes:
        nonlocal file_pull_invocations, stage
        stage = name
        file_pull_invocations += 1
        observation = command_runner.run(
            ("utmctl", "file", "pull", request.target_uuid, path),
            request.command_timeout_seconds,
        )
        writer.write_json(f"{name}.json", observation.as_json())
        if exact is not None:
            input_transfer._require_exact_small_readback(
                observation, exact, name
            )
        else:
            input_transfer._require_successful_small_observation(
                observation, name
            )
            if observation.stdout.total_bytes == 0:
                raise GuestAgentResolutionError(f"{name}-empty")
        return observation.stdout.prefix

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
        process_observation, terminal_processes = (
            runtime_control._observe_processes(command_runner, request)
        )
        writer.write_json(
            "host-process-preflight.json",
            _process_evidence(process_observation, terminal_processes),
        )
        runtime_control._require_no_utmctl(terminal_processes, "preflight")

        stage = "target-handle-pid-discovery"
        handle_observation = command_runner.run(
            network_ready._lsof_argv(request), request.command_timeout_seconds
        )
        identity = runtime_control.parse_target_handle_process(
            handle_observation,
            request,
            expected_argv=network_ready._lsof_argv(request),
        )
        backend_pid = int(identity["backend_pid"])
        target_handles_terminal = "present"
        writer.write_json(
            "target-handle-pid-discovery.json",
            _handle_identity_evidence(handle_observation, identity),
        )

        stage = "target-handle-pid-confirmation"
        for index in range(1, request.identity_observations + 1):
            argv = runtime_control.targeted_lsof_argv(request, backend_pid)
            observation = command_runner.run(
                argv, request.command_timeout_seconds
            )
            identity = runtime_control.parse_target_handle_process(
                observation, request, expected_argv=argv
            )
            identity_observation_count = index
            writer.write_json(
                f"target-handle-pid-confirmation-{index:03d}.json",
                _handle_identity_evidence(observation, identity),
            )
            if int(identity["backend_pid"]) != backend_pid:
                raise GuestAgentResolutionError("target-backend-pid-drift")
            process_observation, terminal_processes = (
                runtime_control._observe_processes(command_runner, request)
            )
            writer.write_json(
                f"host-process-confirmation-{index:03d}.json",
                _process_evidence(process_observation, terminal_processes),
            )
            runtime_control._require_no_utmctl(
                terminal_processes, f"confirmation-{index:03d}"
            )
            if index < request.identity_observations:
                sleeper(float(request.poll_interval_seconds))

        stage = "target-files-qualified"
        target_qualified = validate_target(request)
        writer.write_json("target-files-qualified.json", target_qualified)
        runtime_control._require_live_target_identity_stable(
            target_preflight, target_qualified
        )

        stage = "guest-agent-readiness"
        for index in range(1, request.agent_readiness_attempts + 1):
            agent_readiness_invocations = index
            guest_exec_invocations += 1
            readiness_observation = command_runner.run(
                readiness_argv(request), request.command_timeout_seconds
            )
            writer.write_json(
                f"guest-agent-readiness-{index:03d}.json",
                readiness_observation.as_json(),
            )

            argv = runtime_control.targeted_lsof_argv(request, backend_pid)
            handle_observation = command_runner.run(
                argv, request.command_timeout_seconds
            )
            identity = runtime_control.parse_target_handle_process(
                handle_observation, request, expected_argv=argv
            )
            readiness_identity_observation_count = index
            writer.write_json(
                f"target-handle-readiness-{index:03d}.json",
                _handle_identity_evidence(handle_observation, identity),
            )
            if int(identity["backend_pid"]) != backend_pid:
                raise GuestAgentResolutionError(
                    "target-backend-pid-readiness-drift"
                )
            process_observation, terminal_processes = (
                runtime_control._observe_processes(command_runner, request)
            )
            writer.write_json(
                f"host-process-readiness-{index:03d}.json",
                _process_evidence(process_observation, terminal_processes),
            )
            runtime_control._require_no_utmctl(
                terminal_processes, f"readiness-{index:03d}"
            )

            agent_readiness_state = classify_agent_readiness(
                readiness_observation, request
            )
            if agent_readiness_state == "ready":
                writer.write_json(
                    "guest-agent-ready.json",
                    {
                        "attempt": index,
                        "format": EVIDENCE_FORMAT,
                        "predicate": "canonical-boot-id-readable",
                        "state": "ready",
                        "target_uuid": request.target_uuid,
                    },
                )
                break
            if index < request.agent_readiness_attempts:
                sleeper(float(request.poll_interval_seconds))
        if agent_readiness_state != "ready":
            raise GuestAgentResolutionError(
                "guest-agent-readiness-budget-exhausted"
            )

        stage = "target-files-agent-ready"
        target_agent_ready = validate_target(request)
        writer.write_json(
            "target-files-agent-ready.json", target_agent_ready
        )
        runtime_control._require_live_target_identity_stable(
            target_qualified, target_agent_ready
        )

        stage = "guest-control-root-create"
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
                request.guest_control_root,
            ),
            request.command_timeout_seconds,
        )
        writer.write_json(f"{stage}.json", root_observation.as_json())
        transport_control._require_successful_empty_observation(
            root_observation, stage
        )

        stage = "guest-probe-push"
        file_push_invocations = 1
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
        transport_control._require_successful_empty_observation(
            push_observation, stage
        )

        stage = "guest-probe-normalize"
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
                "--",
                request.guest_probe_incoming,
                request.guest_probe_path,
            ),
            request.command_timeout_seconds,
        )
        writer.write_json(f"{stage}.json", normalize_observation.as_json())
        transport_control._require_successful_empty_observation(
            normalize_observation, stage
        )
        pull(
            request.guest_probe_path,
            "guest-probe-readback",
            exact=binding.probe_bytes,
        )

        stage = "guest-boot-transport-probe-once"
        guest_exec_invocations += 1
        guest_probe_invocations = 1
        probe_observation = command_runner.run(
            probe_argv(request, binding), request.command_timeout_seconds
        )
        writer.write_json(
            f"{stage}.json",
            {
                "format": EVIDENCE_FORMAT,
                "observation": network_ready._observation_metadata(
                    probe_observation
                ),
                "raw_output_persisted": False,
            },
        )
        probe_sha256 = hashlib.sha256(binding.probe_bytes).hexdigest()
        pull(
            request.guest_marker_path,
            "guest-marker-readback",
            exact=guest_probe.marker_bytes(
                request.guest_agent_attempt_id, probe_sha256
            ),
        )
        result_payloads = []
        for index in (1, 2):
            result_readback_invocations = index
            result_payloads.append(
                pull(
                    request.guest_result_path,
                    f"guest-result-readback-{index}",
                )
            )
        result_payload = resume_evidence.require_equal_payloads(
            result_payloads, "guest-agent-boot-result"
        )
        observed_boot_hash = parse_guest_result(
            result_payload, request, probe_sha256
        )
        transport_control._require_successful_empty_observation(
            probe_observation, "guest-boot-transport-probe-once"
        )
        boot_classification = (
            "original-boot-restored"
            if observed_boot_hash == binding.expected_boot_id_sha256
            else "new-boot-started"
        )
        writer.write_json(
            "boot-classification.json",
            {
                "classification": boot_classification,
                "expected_boot_id_sha256": binding.expected_boot_id_sha256,
                "format": EVIDENCE_FORMAT,
                "observed_boot_id_sha256": observed_boot_hash,
                "transport": "bounded-agent-readiness-private-double-readback",
            },
        )

        stage = "target-handle-pid-terminal"
        terminal_argv = runtime_control.targeted_lsof_argv(
            request, backend_pid
        )
        terminal_handle_observation = command_runner.run(
            terminal_argv, request.command_timeout_seconds
        )
        terminal_identity = runtime_control.parse_target_handle_process(
            terminal_handle_observation,
            request,
            expected_argv=terminal_argv,
        )
        writer.write_json(
            "target-handle-pid-terminal.json",
            _handle_identity_evidence(
                terminal_handle_observation, terminal_identity
            ),
        )
        if int(terminal_identity["backend_pid"]) != backend_pid:
            raise GuestAgentResolutionError(
                "target-backend-pid-terminal-drift"
            )

        stage = "host-process-terminal"
        process_observation, terminal_processes = (
            runtime_control._observe_processes(command_runner, request)
        )
        writer.write_json(
            "host-process-terminal.json",
            _process_evidence(process_observation, terminal_processes),
        )
        runtime_control._require_no_utmctl(terminal_processes, "terminal")

        stage = "target-files-postflight"
        target_postflight = validate_target(request)
        writer.write_json("target-files-postflight.json", target_postflight)
        runtime_control._require_live_target_identity_stable(
            target_agent_ready, target_postflight
        )

        outcome = boot_classification
        if outcome == "original-boot-restored":
            exit_code = EXIT_ORIGINAL_BOOT_RESTORED
            reason = (
                "bounded-agent-readiness-private-double-readback-"
                "original-boot-and-terminal-target-stable"
            )
        else:
            exit_code = EXIT_NEW_BOOT_STARTED
            reason = (
                "bounded-agent-readiness-private-double-readback-"
                "new-boot-and-terminal-target-stable"
            )
    except (
        GuestAgentResolutionError,
        boot_control.BootStartResolutionError,
        input_transfer.CanonicalInputTransferError,
        network_ready.NetworkReadyError,
        runtime_control.RuntimeResolutionError,
        resume_evidence.ResumeEvidenceError,
        start_control.StartControlError,
        OSError,
        ValueError,
    ) as exc:
        reason = f"{stage}:{exc}"
        if agent_readiness_invocations > 0:
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
                if agent_readiness_invocations > 0:
                    outcome = "state-indeterminate"
                    exit_code = EXIT_STATE_INDETERMINATE
                else:
                    outcome = "precondition-rejected"
                    exit_code = EXIT_PRECONDITION_REJECTED
            source_bundle.file_object.close()

    terminal = {
        "agent_readiness_invocations": agent_readiness_invocations,
        "agent_readiness_state": agent_readiness_state,
        "automatic_cleanup": "not-performed",
        "automatic_quit": "not-performed",
        "automatic_retry": "not-performed",
        "automatic_stop": "not-performed",
        "backend_pid": backend_pid,
        "boot_classification": boot_classification,
        "business_guest_action": "not-performed",
        "file_pull_invocations": file_pull_invocations,
        "file_push_invocations": file_push_invocations,
        "format": EVIDENCE_FORMAT,
        "guest_agent_attempt_id": request.guest_agent_attempt_id,
        "guest_exec_invocations": guest_exec_invocations,
        "guest_probe_invocations": guest_probe_invocations,
        "identity_observation_count": identity_observation_count,
        "inventory_probe_invocations": 0,
        "maintenance_resume_invocations": 0,
        "observed_boot_id_sha256": observed_boot_hash,
        "operation_id": "not-read-or-generated",
        "outcome": outcome,
        "plain_utmctl_list": "not-performed",
        "plain_utmctl_start": "not-performed",
        "plain_utmctl_status": "not-performed",
        "readiness_identity_observation_count": (
            readiness_identity_observation_count
        ),
        "reason": reason,
        "result_readback_invocations": result_readback_invocations,
        "target_handles_terminal": target_handles_terminal,
        "target_name": request.target_name,
        "target_uuid": request.target_uuid,
        "terminal_relevant_host_process_count": len(terminal_processes),
        "transaction": "artifacts-staged-preserved-no-resume",
    }
    writer.write_json("terminal.json", terminal)
    runtime_control._require_no_raw_operation_id(request.output_root)
    manifest_sha256 = writer.write_manifest()
    return GuestAgentResolutionResult(
        outcome=outcome,
        exit_code=exit_code,
        evidence_root=request.output_root,
        manifest_sha256=manifest_sha256,
        agent_readiness_invocations=agent_readiness_invocations,
        guest_probe_invocations=guest_probe_invocations,
        result_readback_invocations=result_readback_invocations,
    )


def validate_guest_agent_bindings(
    request: GuestAgentResolutionRequest,
) -> bindings.GuestAgentBinding:
    expected_paths = (
        (Path(__file__).absolute(), CONTROL_RELATIVE_PATH, "control"),
        (Path(bindings.__file__).absolute(), BINDINGS_RELATIVE_PATH, "bindings"),
        (Path(guest_probe.__file__).absolute(), PROBE_RELATIVE_PATH, "probe"),
    )
    for actual, relative, label in expected_paths:
        if actual != request.repository_root / relative:
            raise GuestAgentResolutionError(
                f"executed-{label}-path-mismatch"
            )
    try:
        return bindings.validate_guest_agent_bindings(request)
    except ValueError as exc:
        raise GuestAgentResolutionError(str(exc)) from exc


def readiness_argv(
    request: GuestAgentResolutionRequest,
) -> tuple[str, ...]:
    return (
        "utmctl",
        "exec",
        request.target_uuid,
        "--cmd",
        "/usr/bin/test",
        "-r",
        "/proc/sys/kernel/random/boot_id",
    )


def classify_agent_readiness(
    observation: start_control.CommandObservation,
    request: GuestAgentResolutionRequest,
) -> str:
    if observation.argv != readiness_argv(request) or observation.timed_out:
        raise GuestAgentResolutionError("guest-agent-readiness-command-invalid")
    if (
        observation.exit_code == 0
        and observation.stdout.total_bytes == 0
        and observation.stderr.total_bytes == 0
    ):
        return "ready"
    if (
        observation.exit_code == 0
        and observation.stdout.total_bytes == 0
        and observation.stderr.prefix
        == bindings.REQUIRED_AGENT_UNAVAILABLE_STDERR
        and observation.stderr.total_bytes
        == len(bindings.REQUIRED_AGENT_UNAVAILABLE_STDERR)
        and not observation.stderr.truncated
    ):
        return "unavailable"
    raise GuestAgentResolutionError("guest-agent-readiness-result-unknown")


def probe_argv(
    request: GuestAgentResolutionRequest,
    binding: bindings.GuestAgentBinding,
) -> tuple[str, ...]:
    return (
        "utmctl",
        "exec",
        request.target_uuid,
        "--cmd",
        "/usr/bin/python3",
        "-I",
        "-B",
        request.guest_probe_path,
        "--attempt-id",
        request.guest_agent_attempt_id,
        "--target-uuid",
        request.target_uuid,
        "--expected-probe-sha256",
        hashlib.sha256(binding.probe_bytes).hexdigest(),
        "--control-scope",
        guest_probe.CONTROL_SCOPE_GUEST_AGENT,
        "--control-root",
        request.guest_control_root,
    )


def parse_guest_result(
    payload: bytes,
    request: GuestAgentResolutionRequest,
    expected_probe_sha256: str,
) -> str:
    try:
        value = json.loads(payload.decode("ascii"))
    except (UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise GuestAgentResolutionError("guest-result-json-invalid") from exc
    if not isinstance(value, dict) or set(value) != {
        "attempt_id",
        "boot_id_sha256",
        "format",
        "outcome",
        "probe_sha256",
        "target_uuid",
    }:
        raise GuestAgentResolutionError("guest-result-fields-invalid")
    boot_hash = value.get("boot_id_sha256")
    if (
        value.get("attempt_id") != request.guest_agent_attempt_id
        or value.get("format") != guest_probe.EVIDENCE_FORMAT
        or value.get("outcome") != "passed"
        or value.get("probe_sha256") != expected_probe_sha256
        or value.get("target_uuid") != request.target_uuid
        or not isinstance(boot_hash, str)
        or not HEX_64.fullmatch(boot_hash)
    ):
        raise GuestAgentResolutionError("guest-result-semantics-invalid")
    if payload != guest_probe.result_bytes(
        request.guest_agent_attempt_id,
        request.target_uuid,
        expected_probe_sha256,
        boot_hash,
    ):
        raise GuestAgentResolutionError("guest-result-not-canonical")
    return boot_hash


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


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Resolve one frozen artifacts_staged boot through stable target "
            "handles, bounded read-only guest-agent readiness, and one "
            "private double-readback boot probe."
        )
    )
    parser.add_argument(
        "command", choices=("resolve-guest-agent-transport-once",)
    )
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
        "prior-runtime-resolution-root",
        "prior-runtime-resolution-v2-root",
        "prior-boot-transport-root",
        "prior-boot-start-root",
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
        "prior-runtime-resolution-manifest-sha256",
        "prior-runtime-resolution-v2-manifest-sha256",
        "prior-boot-transport-manifest-sha256",
        "prior-boot-start-manifest-sha256",
        "source-bundle-sha256",
        "transfer-attempt-id",
        "resolution-attempt-id",
        "preflight-attempt-id",
        "checkpoint-attempt-id",
        "resume-attempt-id",
        "backend-resolution-attempt-id",
        "reactivation-attempt-id",
        "prior-runtime-resolution-attempt-id",
        "runtime-resolution-attempt-id",
        "prior-runtime-resolution-v2-attempt-id",
        "boot-transport-attempt-id",
        "prior-boot-transport-attempt-id",
        "boot-start-attempt-id",
        "prior-boot-start-attempt-id",
        "guest-agent-attempt-id",
        "target-uuid",
        "target-name",
    ):
        parser.add_argument(f"--{name}", required=True)
    parser.add_argument("--source-bundle-size", type=int, required=True)
    parser.add_argument("--expected-vm-count", type=int, default=21)
    parser.add_argument("--identity-observations", type=int, default=3)
    parser.add_argument("--agent-readiness-attempts", type=int, default=60)
    parser.add_argument("--poll-interval-seconds", type=int, default=1)
    parser.add_argument("--command-timeout-seconds", type=int, default=60)
    for name in (
        "authorized-install-artifacts-staged-guest-agent-resolution",
        "authorized-bounded-read-only-guest-agent-readiness",
        "authorized-stable-target-handle-pid-observations",
        "authorized-one-private-guest-probe-delivery-and-execution",
        "authorized-two-independent-result-readbacks",
        "authorized-no-list-status-start-resume-business-guest-retry-stop-or-quit",
    ):
        parser.add_argument(f"--{name}", action="store_true")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    values = vars(args).copy()
    values.pop("command")
    request = GuestAgentResolutionRequest(**values)
    try:
        result = run_guest_agent_resolution(request)
    except GuestAgentResolutionError as exc:
        print(
            f"guest agent resolution rejected before evidence creation: {exc}",
            file=sys.stderr,
        )
        return EXIT_PRECONDITION_REJECTED
    print(f"outcome={result.outcome}")
    print(f"evidence_root={result.evidence_root}")
    print(f"manifest_sha256={result.manifest_sha256}")
    print(
        f"agent_readiness_invocations={result.agent_readiness_invocations}"
    )
    print(f"guest_probe_invocations={result.guest_probe_invocations}")
    print(
        f"result_readback_invocations={result.result_readback_invocations}"
    )
    return result.exit_code


if __name__ == "__main__":
    raise SystemExit(main())
