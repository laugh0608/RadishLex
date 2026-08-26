#!/usr/bin/env python3
from __future__ import annotations

import argparse
import hashlib
import json
import sys
import tarfile
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Callable, Protocol

import l6_utm_canonical_input_transfer as input_transfer
import l6_utm_guest_network_ready as network_ready
import l6_utm_install_artifacts_staged_boot_classification_resolution as classification_control
import l6_utm_install_artifacts_staged_guest_agent_resolution as guest_agent_control
import l6_utm_install_artifacts_staged_new_boot_recovery_preflight_bindings as bindings
import l6_utm_install_artifacts_staged_runtime_resolution as runtime_control
import l6_utm_install_artifacts_staged_resume_evidence as resume_evidence
import l6_utm_install_artifacts_staged_boot_start_resolution as boot_start_control
import l6_utm_launch_transport_v2 as launch_transport
import l6_utm_start_once as start_control
import l6_v4_canonical_input_install as guest_installer
import l6_v4_install_artifacts_staged_new_boot_recovery_preflight as guest_probe


EVIDENCE_FORMAT = (
    "radishlex-linux-l6-utm-install-artifacts-staged-"
    "new-boot-recovery-preflight-v1"
)
CONTROL_RELATIVE_PATH = bindings.CONTROL_RELATIVE_PATH
BINDINGS_RELATIVE_PATH = bindings.BINDINGS_RELATIVE_PATH
PROBE_RELATIVE_PATH = bindings.PROBE_RELATIVE_PATH
RESUME_DRIVER_RELATIVE_PATH = bindings.RESUME_DRIVER_RELATIVE_PATH
EXIT_RECOVERY_QUALIFIED = 0
EXIT_RECOVERY_REJECTED = 10
EXIT_PRECONDITION_REJECTED = 11
EXIT_STATE_INDETERMINATE = 12


class RecoveryPreflightError(ValueError):
    pass


@dataclass(frozen=True)
class RecoveryPreflightRequest(
    classification_control.BootClassificationResolutionRequest
):
    prior_boot_classification_root: Path
    prior_boot_classification_manifest_sha256: str
    prior_boot_classification_attempt_id: str
    recovery_preflight_attempt_id: str
    authorized_install_artifacts_staged_new_boot_recovery_preflight: bool
    authorized_bound_prior_boot_classification_result: bool
    authorized_bounded_read_only_recovery_probe: bool
    authorized_one_private_recovery_probe_delivery_and_execution: bool
    authorized_no_inventory_start_status_resume_dpkg_mutation_retry_stop_or_quit: (
        bool
    )

    @property
    def guest_recovery_root(self) -> str:
        return (
            "/var/tmp/radishlex-l6-v4-install-artifacts-staged-"
            "new-boot-recovery-preflight-"
            + self.recovery_preflight_attempt_id
        )

    @property
    def guest_probe_incoming(self) -> str:
        return f"{self.guest_recovery_root}/recovery-preflight.incoming.py"

    @property
    def guest_probe_path(self) -> str:
        return f"{self.guest_recovery_root}/recovery-preflight.py"

    @property
    def guest_resume_driver_incoming(self) -> str:
        return f"{self.guest_recovery_root}/frozen-resume-driver.incoming.py"

    @property
    def guest_resume_driver_path(self) -> str:
        return f"{self.guest_recovery_root}/frozen-resume-driver.py"

    @property
    def guest_marker_path(self) -> str:
        return f"{self.guest_recovery_root}/attempt.marker.json"

    @property
    def guest_phase_path(self) -> str:
        return f"{self.guest_recovery_root}/phase.json"

    @property
    def guest_terminal_path(self) -> str:
        return f"{self.guest_recovery_root}/preflight.evidence.json"

    def validate(self) -> None:
        super().validate()
        if (
            not self.prior_boot_classification_root.is_absolute()
            or ".." in self.prior_boot_classification_root.parts
        ):
            raise RecoveryPreflightError(
                "prior-boot-classification-root-must-be-absolute-normalized"
            )
        if runtime_control._paths_overlap(
            self.output_root, self.prior_boot_classification_root
        ):
            raise RecoveryPreflightError(
                "output-root-must-not-overlap-prior-boot-classification-root"
            )
        if (
            self.prior_boot_classification_manifest_sha256
            != bindings.REQUIRED_PRIOR_MANIFEST_SHA256
            or self.prior_boot_classification_attempt_id
            != bindings.REQUIRED_PRIOR_ATTEMPT_ID
            or self.recovery_preflight_attempt_id != bindings.REQUIRED_ATTEMPT_ID
        ):
            raise RecoveryPreflightError("fixed-recovery-input-mismatch")
        for authorized, reason in (
            (
                self.authorized_install_artifacts_staged_new_boot_recovery_preflight,
                "new-boot-recovery-preflight-authorization-required",
            ),
            (
                self.authorized_bound_prior_boot_classification_result,
                "prior-boot-classification-binding-authorization-required",
            ),
            (
                self.authorized_bounded_read_only_recovery_probe,
                "bounded-readonly-recovery-probe-authorization-required",
            ),
            (
                self.authorized_one_private_recovery_probe_delivery_and_execution,
                "one-private-recovery-probe-authorization-required",
            ),
            (
                self.authorized_no_inventory_start_status_resume_dpkg_mutation_retry_stop_or_quit,
                "forbidden-action-boundary-authorization-required",
            ),
        ):
            if not authorized:
                raise RecoveryPreflightError(reason)

    def as_json(self) -> dict[str, object]:
        value = super().as_json()
        value.update(
            {
                "authorization": {
                    "bound_prior_boot_classification_result": True,
                    "bounded_read_only_guest_agent_readiness": True,
                    "install_artifacts_staged_new_boot_recovery_preflight": True,
                    (
                        "no_inventory_start_status_resume_dpkg_mutation_retry_"
                        "stop_or_quit"
                    ): True,
                    "one_private_recovery_probe_delivery_and_execution": True,
                    "two_independent_result_readbacks": True,
                },
                "format": EVIDENCE_FORMAT,
                "prior_boot_classification_attempt_id": (
                    self.prior_boot_classification_attempt_id
                ),
                "prior_boot_classification_manifest_sha256": (
                    self.prior_boot_classification_manifest_sha256
                ),
                "recovery_preflight_attempt_id": (
                    self.recovery_preflight_attempt_id
                ),
            }
        )
        return value


@dataclass(frozen=True)
class RecoveryPreflightResult:
    outcome: str
    exit_code: int
    evidence_root: Path
    manifest_sha256: str
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


def validate_recovery_preflight_bindings(
    request: RecoveryPreflightRequest,
) -> bindings.RecoveryPreflightBinding:
    expected_paths = (
        (Path(__file__).absolute(), CONTROL_RELATIVE_PATH, "control"),
        (Path(bindings.__file__).absolute(), BINDINGS_RELATIVE_PATH, "bindings"),
        (Path(guest_probe.__file__).absolute(), PROBE_RELATIVE_PATH, "probe"),
    )
    for actual, relative, label in expected_paths:
        if actual != request.repository_root / relative:
            raise RecoveryPreflightError(f"executed-{label}-path-mismatch")
    try:
        return bindings.validate_recovery_preflight_bindings(request)
    except ValueError as exc:
        raise RecoveryPreflightError(str(exc)) from exc


def readiness_argv(request: RecoveryPreflightRequest) -> tuple[str, ...]:
    return (
        "utmctl",
        "exec",
        request.target_uuid,
        "--cmd",
        "/usr/bin/test",
        "-r",
        "/proc/sys/kernel/random/boot_id",
    )


def probe_argv(
    request: RecoveryPreflightRequest,
    binding: bindings.RecoveryPreflightBinding,
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
        request.recovery_preflight_attempt_id,
        "--target-uuid",
        request.target_uuid,
        "--prior-boot-id-sha256",
        binding.prior_boot_id_sha256,
        "--current-boot-id-sha256",
        binding.current_boot_id_sha256,
        "--resume-driver",
        request.guest_resume_driver_path,
        "--expected-resume-driver-sha256",
        hashlib.sha256(binding.resume_driver_bytes).hexdigest(),
        "--expected-probe-sha256",
        hashlib.sha256(binding.probe_bytes).hexdigest(),
        "--control-root",
        request.guest_recovery_root,
    )


def marker_bytes(
    request: RecoveryPreflightRequest,
    binding: bindings.RecoveryPreflightBinding,
) -> bytes:
    return guest_probe.canonical_json(
        {
            "attempt_id": request.recovery_preflight_attempt_id,
            "format": guest_probe.MARKER_FORMAT,
            "probe_sha256": hashlib.sha256(binding.probe_bytes).hexdigest(),
            "resume_driver_sha256": hashlib.sha256(
                binding.resume_driver_bytes
            ).hexdigest(),
        }
    )


def parse_guest_terminal(
    payload: bytes,
    request: RecoveryPreflightRequest,
    binding: bindings.RecoveryPreflightBinding,
) -> dict[str, object]:
    try:
        value = json.loads(payload.decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise RecoveryPreflightError("guest-terminal-json-invalid") from exc
    common = {
        "automatic_cleanup",
        "automatic_quit",
        "automatic_retry",
        "automatic_stop",
        "current_boot_id_sha256",
        "dpkg_mutation_executed",
        "format",
        "guard_profile",
        "maintenance_resume_invocations",
        "operation_id",
        "operation_id_sha256",
        "outcome",
        "phase",
        "prior_boot_id_sha256",
        "reason",
        "target_uuid",
        "transaction",
    }
    success_only = {"receipt_state", "startup_gate"}
    if not isinstance(value, dict) or set(value) not in (common, common | success_only):
        raise RecoveryPreflightError("guest-terminal-fields-invalid")
    if (
        value.get("format") != guest_probe.EVIDENCE_FORMAT
        or value.get("target_uuid") != request.target_uuid
        or value.get("prior_boot_id_sha256") != binding.prior_boot_id_sha256
        or value.get("current_boot_id_sha256") != binding.current_boot_id_sha256
        or value.get("operation_id") != "existing-receipt-hash-only"
        or value.get("operation_id_sha256")
        != guest_probe.EXPECTED_OPERATION_ID_SHA256
        or value.get("maintenance_resume_invocations") != 0
        or value.get("dpkg_mutation_executed") is not False
        or value.get("transaction") != "artifacts-staged-preserved-no-resume"
        or any(
            value.get(key) != "not-performed"
            for key in (
                "automatic_cleanup",
                "automatic_quit",
                "automatic_retry",
                "automatic_stop",
            )
        )
    ):
        raise RecoveryPreflightError("guest-terminal-semantics-invalid")
    outcome = value.get("outcome")
    if outcome == "recovery-qualified":
        expected_startup = {
            "absent-after-reboot": guest_probe.EXPECTED_OPERATION_IN_PROGRESS_STARTUP,
            "present-valid-unlocked": guest_probe.EXPECTED_ACTIVE_GUARD_STARTUP,
        }.get(str(value.get("guard_profile")))
        if (
            set(value) != common | success_only
            or value.get("phase") != "complete"
            or value.get("receipt_state") != "artifacts_staged"
            or value.get("startup_gate") != expected_startup
        ):
            raise RecoveryPreflightError("guest-qualified-semantics-invalid")
    elif outcome == "recovery-rejected":
        if set(value) != common or not isinstance(value.get("reason"), str):
            raise RecoveryPreflightError("guest-rejected-semantics-invalid")
    else:
        raise RecoveryPreflightError("guest-terminal-outcome-invalid")
    return value


def run_recovery_preflight(
    request: RecoveryPreflightRequest,
    *,
    runner: CommandRunner | None = None,
    binding_validator=None,
    source_opener=None,
    source_revalidator=None,
    target_validator=None,
    sleeper: Sleeper = time.sleep,
) -> RecoveryPreflightResult:
    request.validate()
    writer = start_control.EvidenceWriter.create(request.output_root)
    writer.write_json("request.json", request.as_json())
    command_runner = runner or input_transfer.SubprocessCommandRunner()
    validate_bindings = binding_validator or validate_recovery_preflight_bindings
    open_source = source_opener or input_transfer.open_source_bundle
    revalidate_source = (
        source_revalidator or input_transfer.revalidate_open_source_bundle
    )
    validate_target = target_validator or network_ready.validate_target_files

    stage = "binding-preflight"
    reason = "not-run"
    outcome = "precondition-rejected"
    exit_code = EXIT_PRECONDITION_REJECTED
    source_bundle: input_transfer.SourceBundle | None = None
    target_before: dict[str, object] | None = None
    target_ready: dict[str, object] | None = None
    backend_pid: int | None = None
    terminal_processes: tuple[dict[str, object], ...] = ()
    agent_readiness_invocations = 0
    identity_observation_count = 0
    guest_exec_invocations = 0
    file_push_invocations = 0
    file_pull_invocations = 0
    guest_probe_invocations = 0
    result_readback_invocations = 0
    probe_invoked = False
    guest_terminal: dict[str, object] | None = None

    def pull(path: str, name: str, *, exact: bytes | None = None) -> bytes:
        nonlocal file_pull_invocations, stage
        stage = name
        file_pull_invocations += 1
        observation = command_runner.run(
            ("utmctl", "file", "pull", request.target_uuid, path),
            request.command_timeout_seconds,
        )
        writer.write_json(f"{name}.json", observation.as_json())
        if exact is None:
            input_transfer._require_successful_small_observation(observation, name)
            if observation.stdout.total_bytes == 0:
                raise RecoveryPreflightError(f"{name}-empty")
        else:
            input_transfer._require_exact_small_readback(observation, exact, name)
        return observation.stdout.prefix

    try:
        binding = validate_bindings(request)
        writer.write_json("binding-preflight.json", binding.evidence)

        stage = "source-bundle-preflight"
        source_bundle = open_source(request)
        writer.write_json(stage + ".json", source_bundle.as_json())

        stage = "target-files-preflight"
        target_before = validate_target(request)
        writer.write_json(stage + ".json", target_before)

        stage = "host-process-preflight"
        process_observation, terminal_processes = runtime_control._observe_processes(
            command_runner, request
        )
        writer.write_json(
            stage + ".json",
            runtime_control._process_evidence(process_observation, terminal_processes),
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
        writer.write_json(
            stage + ".json",
            runtime_control._handle_identity_evidence(handle_observation, identity),
        )

        for index in range(1, request.identity_observations + 1):
            stage = f"target-handle-pid-confirmation-{index:03d}"
            argv = runtime_control.targeted_lsof_argv(request, backend_pid)
            observation = command_runner.run(argv, request.command_timeout_seconds)
            identity = runtime_control.parse_target_handle_process(
                observation, request, expected_argv=argv
            )
            identity_observation_count = index
            writer.write_json(
                stage + ".json",
                runtime_control._handle_identity_evidence(observation, identity),
            )
            if int(identity["backend_pid"]) != backend_pid:
                raise RecoveryPreflightError("target-backend-pid-drift")
            process_observation, terminal_processes = (
                runtime_control._observe_processes(command_runner, request)
            )
            writer.write_json(
                f"host-process-confirmation-{index:03d}.json",
                runtime_control._process_evidence(
                    process_observation, terminal_processes
                ),
            )
            runtime_control._require_no_utmctl(
                terminal_processes, f"confirmation-{index:03d}"
            )

        stage = "guest-agent-readiness"
        for index in range(1, request.agent_readiness_attempts + 1):
            agent_readiness_invocations = index
            readiness = command_runner.run(
                readiness_argv(request), request.command_timeout_seconds
            )
            writer.write_json(
                f"guest-agent-readiness-{index:03d}.json", readiness.as_json()
            )
            state = guest_agent_control.classify_agent_readiness(
                readiness, request
            )
            if state == "ready":
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
        else:
            raise RecoveryPreflightError("guest-agent-readiness-budget-exhausted")

        stage = "target-files-agent-ready"
        target_ready = validate_target(request)
        writer.write_json(stage + ".json", target_ready)
        runtime_control._require_live_target_identity_stable(
            target_before, target_ready
        )

        stage = "target-handle-agent-ready"
        argv = runtime_control.targeted_lsof_argv(request, backend_pid)
        observation = command_runner.run(argv, request.command_timeout_seconds)
        identity = runtime_control.parse_target_handle_process(
            observation, request, expected_argv=argv
        )
        writer.write_json(
            stage + ".json",
            runtime_control._handle_identity_evidence(observation, identity),
        )
        if int(identity["backend_pid"]) != backend_pid:
            raise RecoveryPreflightError("target-backend-pid-agent-ready-drift")
        process_observation, terminal_processes = runtime_control._observe_processes(
            command_runner, request
        )
        writer.write_json(
            "host-process-agent-ready.json",
            runtime_control._process_evidence(
                process_observation, terminal_processes
            ),
        )
        runtime_control._require_no_utmctl(terminal_processes, "agent-ready")

        stage = "guest-recovery-root-create"
        guest_exec_invocations += 1
        observation = command_runner.run(
            (
                "utmctl",
                "exec",
                request.target_uuid,
                "--cmd",
                "/bin/mkdir",
                "-m",
                "0700",
                request.guest_recovery_root,
            ),
            request.command_timeout_seconds,
        )
        writer.write_json(stage + ".json", observation.as_json())
        input_transfer._require_successful_small_observation(observation, stage)

        for label, relative, incoming, final, expected in (
            (
                "frozen-resume-driver",
                RESUME_DRIVER_RELATIVE_PATH,
                request.guest_resume_driver_incoming,
                request.guest_resume_driver_path,
                binding.resume_driver_bytes,
            ),
            (
                "recovery-preflight-probe",
                PROBE_RELATIVE_PATH,
                request.guest_probe_incoming,
                request.guest_probe_path,
                binding.probe_bytes,
            ),
        ):
            stage = f"guest-{label}-push"
            file_push_invocations += 1
            with (request.repository_root / relative).open("rb") as source:
                observation = command_runner.run(
                    ("utmctl", "file", "push", request.target_uuid, incoming),
                    request.command_timeout_seconds,
                    stdin_file=source,
                )
            writer.write_json(stage + ".json", observation.as_json())
            input_transfer._require_successful_small_observation(observation, stage)
            for action, command in (
                ("chown", ("/bin/chown", "root:root", incoming)),
                ("chmod", ("/bin/chmod", "0600", incoming)),
                ("publish", ("/bin/mv", "--", incoming, final)),
            ):
                stage = f"guest-{label}-{action}"
                guest_exec_invocations += 1
                observation = command_runner.run(
                    ("utmctl", "exec", request.target_uuid, "--cmd", *command),
                    request.command_timeout_seconds,
                )
                writer.write_json(stage + ".json", observation.as_json())
                input_transfer._require_successful_small_observation(
                    observation, stage
                )
            pull(final, f"guest-{label}-readback", exact=expected)

        stage = "target-handle-pre-probe"
        argv = runtime_control.targeted_lsof_argv(request, backend_pid)
        observation = command_runner.run(argv, request.command_timeout_seconds)
        identity = runtime_control.parse_target_handle_process(
            observation, request, expected_argv=argv
        )
        writer.write_json(
            stage + ".json",
            runtime_control._handle_identity_evidence(observation, identity),
        )
        if int(identity["backend_pid"]) != backend_pid:
            raise RecoveryPreflightError("target-backend-pid-pre-probe-drift")
        process_observation, terminal_processes = runtime_control._observe_processes(
            command_runner, request
        )
        writer.write_json(
            "host-process-pre-probe.json",
            runtime_control._process_evidence(
                process_observation, terminal_processes
            ),
        )
        runtime_control._require_no_utmctl(terminal_processes, "pre-probe")

        stage = "guest-recovery-preflight-once"
        guest_exec_invocations += 1
        guest_probe_invocations = 1
        probe_invoked = True
        probe_observation = command_runner.run(
            probe_argv(request, binding), request.command_timeout_seconds
        )
        writer.write_json(
            stage + ".json",
            {
                "format": EVIDENCE_FORMAT,
                "observation": network_ready._observation_metadata(
                    probe_observation
                ),
                "raw_output_persisted": False,
            },
        )
        pull(
            request.guest_marker_path,
            "guest-recovery-marker-readback",
            exact=marker_bytes(request, binding),
        )
        payloads = []
        for index in (1, 2):
            result_readback_invocations = index
            payloads.append(
                pull(
                    request.guest_terminal_path,
                    f"guest-recovery-result-readback-{index}",
                )
            )
        terminal_payload = resume_evidence.require_equal_payloads(
            payloads, "guest-recovery-result"
        )
        guest_terminal = parse_guest_terminal(
            terminal_payload, request, binding
        )
        writer.write_json("guest-recovery-result.json", guest_terminal)
        pull(
            request.guest_phase_path,
            "guest-recovery-phase-readback",
            exact=guest_probe.canonical_json(
                {
                    "format": guest_probe.PHASE_FORMAT,
                    "phase": (
                        "complete"
                        if guest_terminal["outcome"] == "recovery-qualified"
                        else "rejected"
                    ),
                }
            ),
        )
        expected_exit = (
            0 if guest_terminal["outcome"] == "recovery-qualified" else 10
        )
        if probe_observation.exit_code != expected_exit:
            raise RecoveryPreflightError("probe-exit-evidence-mismatch")

        stage = "target-handle-pid-terminal"
        argv = runtime_control.targeted_lsof_argv(request, backend_pid)
        observation = command_runner.run(argv, request.command_timeout_seconds)
        identity = runtime_control.parse_target_handle_process(
            observation, request, expected_argv=argv
        )
        writer.write_json(
            stage + ".json",
            runtime_control._handle_identity_evidence(observation, identity),
        )
        if int(identity["backend_pid"]) != backend_pid:
            raise RecoveryPreflightError("target-backend-pid-terminal-drift")

        stage = "host-process-terminal"
        process_observation, terminal_processes = runtime_control._observe_processes(
            command_runner, request
        )
        writer.write_json(
            stage + ".json",
            runtime_control._process_evidence(process_observation, terminal_processes),
        )
        runtime_control._require_no_utmctl(terminal_processes, "terminal")

        stage = "target-files-postflight"
        target_after = validate_target(request)
        writer.write_json(stage + ".json", target_after)
        runtime_control._require_live_target_identity_stable(target_ready, target_after)

        if guest_terminal["outcome"] == "recovery-qualified":
            outcome = "recovery-qualified"
            exit_code = EXIT_RECOVERY_QUALIFIED
            reason = "persistent-artifacts-staged-new-boot-readonly-qualified"
        else:
            outcome = "recovery-rejected"
            exit_code = EXIT_RECOVERY_REJECTED
            reason = f"guest-recovery-rejected:{guest_terminal['reason']}"
    except (
        RecoveryPreflightError,
        boot_start_control.BootStartResolutionError,
        input_transfer.CanonicalInputTransferError,
        network_ready.NetworkReadyError,
        runtime_control.RuntimeResolutionError,
        resume_evidence.ResumeEvidenceError,
        start_control.StartControlError,
        OSError,
        tarfile.TarError,
        ValueError,
    ) as exc:
        reason = f"{stage}:{exc}"
        if probe_invoked:
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
                ValueError,
            ) as exc:
                reason = f"source-bundle-postflight:{exc}"
                if probe_invoked:
                    outcome = "state-indeterminate"
                    exit_code = EXIT_STATE_INDETERMINATE
            source_bundle.file_object.close()

    writer.write_json(
        "terminal.json",
        {
            "agent_readiness_invocations": agent_readiness_invocations,
            "automatic_cleanup": "not-performed",
            "automatic_quit": "not-performed",
            "automatic_retry": "not-performed",
            "automatic_stop": "not-performed",
            "backend_pid": backend_pid,
            "business_guest_action": "not-performed",
            "file_pull_invocations": file_pull_invocations,
            "file_push_invocations": file_push_invocations,
            "format": EVIDENCE_FORMAT,
            "guest_exec_invocations": guest_exec_invocations,
            "guest_probe_invocations": guest_probe_invocations,
            "identity_observation_count": identity_observation_count,
            "inventory_probe_invocations": 0,
            "maintenance_resume_invocations": 0,
            "operation_id": "existing-receipt-hash-only",
            "outcome": outcome,
            "plain_utmctl_list": "not-performed",
            "plain_utmctl_start": "not-performed",
            "plain_utmctl_status": "not-performed",
            "reason": reason,
            "recovery_preflight_attempt_id": request.recovery_preflight_attempt_id,
            "result_readback_invocations": result_readback_invocations,
            "target_name": request.target_name,
            "target_uuid": request.target_uuid,
            "terminal_relevant_host_process_count": len(terminal_processes),
            "transaction": "artifacts-staged-preserved-no-resume",
        },
    )
    runtime_control._require_no_raw_operation_id(request.output_root)
    manifest_sha256 = writer.write_manifest()
    return RecoveryPreflightResult(
        outcome=outcome,
        exit_code=exit_code,
        evidence_root=request.output_root,
        manifest_sha256=manifest_sha256,
        guest_probe_invocations=guest_probe_invocations,
        result_readback_invocations=result_readback_invocations,
    )


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Revalidate the frozen artifacts_staged transaction and cross-reboot "
            "startup guard semantics without resume or package mutation."
        )
    )
    parser.add_argument("command", choices=("qualify-new-boot-recovery",))
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
        "prior-guest-agent-root",
        "prior-boot-classification-root",
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
        "prior-guest-agent-manifest-sha256",
        "prior-boot-classification-manifest-sha256",
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
        "prior-guest-agent-attempt-id",
        "boot-classification-attempt-id",
        "prior-boot-classification-attempt-id",
        "recovery-preflight-attempt-id",
        "target-uuid",
        "target-name",
    ):
        parser.add_argument(f"--{name}", required=True)
    parser.add_argument("--source-bundle-size", type=int, required=True)
    parser.add_argument("--expected-vm-count", type=int, default=21)
    parser.add_argument("--quiescence-observations", type=int, default=3)
    parser.add_argument("--runtime-poll-attempts", type=int, default=60)
    parser.add_argument("--identity-observations", type=int, default=3)
    parser.add_argument("--agent-readiness-attempts", type=int, default=60)
    parser.add_argument("--poll-interval-seconds", type=int, default=1)
    parser.add_argument("--transport-timeout-seconds", type=int, default=60)
    parser.add_argument("--command-timeout-seconds", type=int, default=60)
    for name in (
        "authorized-install-artifacts-staged-new-boot-recovery-preflight",
        "authorized-bound-prior-boot-classification-result",
        "authorized-bounded-read-only-recovery-probe",
        "authorized-one-private-recovery-probe-delivery-and-execution",
        "authorized-no-inventory-start-status-resume-dpkg-mutation-retry-stop-or-quit",
    ):
        parser.add_argument(f"--{name}", action="store_true")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    values = vars(args).copy()
    values.pop("command")
    values.update(
        {
            "authorized_install_artifacts_staged_boot_start_resolution": True,
            "authorized_one_potential_backend_reactivation_list": True,
            "authorized_one_foreground_start_from_stopped": True,
            "authorized_bounded_target_runtime_observation": True,
            "authorized_one_private_guest_probe_delivery_and_execution": True,
            "authorized_two_independent_result_readbacks": True,
            "authorized_no_status_resume_business_guest_retry_stop_or_quit": True,
            "authorized_install_artifacts_staged_boot_classification_resolution": True,
            "authorized_bound_prior_guest_agent_result": True,
            "authorized_bounded_read_only_guest_agent_readiness": True,
        }
    )
    request = RecoveryPreflightRequest(**values)
    try:
        result = run_recovery_preflight(request)
    except (RecoveryPreflightError, boot_start_control.BootStartResolutionError) as exc:
        print(
            f"recovery preflight rejected before evidence creation: {exc}",
            file=sys.stderr,
        )
        return EXIT_PRECONDITION_REJECTED
    print(f"outcome={result.outcome}")
    print(f"evidence_root={result.evidence_root}")
    print(f"manifest_sha256={result.manifest_sha256}")
    print(f"guest_probe_invocations={result.guest_probe_invocations}")
    print(f"result_readback_invocations={result.result_readback_invocations}")
    return result.exit_code


if __name__ == "__main__":
    raise SystemExit(main())
