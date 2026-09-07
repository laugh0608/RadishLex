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
import l6_utm_install_artifacts_staged_fresh_boot_classification as classification_control
import l6_utm_install_artifacts_staged_fresh_boot_recovery_result_resolution as resolution_control
import l6_utm_install_artifacts_staged_fresh_boot_recovery_result_resolution_bindings as result_bindings
import l6_utm_install_artifacts_staged_guest_agent_resolution as guest_agent_control
import l6_utm_install_artifacts_staged_new_boot_recovery_preflight as recovery_control
import l6_utm_install_artifacts_staged_resume_evidence as resume_evidence
import l6_utm_install_artifacts_staged_runtime_resolution as runtime_control
import l6_utm_start_once as start_control
import l6_v4_canonical_input_install as guest_installer
import l6_v4_install_artifacts_staged_fresh_boot_resume_driver as guest_driver


EVIDENCE_FORMAT = (
    "radishlex-linux-l6-utm-install-artifacts-staged-fresh-boot-resume-v1"
)
CONTROL_RELATIVE_PATH = Path(
    "scripts/linux-product/"
    "l6_utm_install_artifacts_staged_fresh_boot_resume.py"
)
BINDINGS_RELATIVE_PATH = result_bindings.BINDINGS_RELATIVE_PATH
FROZEN_RESUME_DRIVER_RELATIVE_PATH = Path(
    "scripts/linux-product/l6_v4_install_artifacts_staged_resume_driver.py"
)
FROZEN_RECOVERY_PROBE_RELATIVE_PATH = Path(
    "scripts/linux-product/"
    "l6_v4_install_artifacts_staged_new_boot_recovery_preflight.py"
)
FRESH_BOOT_RESUME_DRIVER_RELATIVE_PATH = Path(
    "scripts/linux-product/"
    "l6_v4_install_artifacts_staged_fresh_boot_resume_driver.py"
)
REQUIRED_ATTEMPT_ID = guest_driver.EXPECTED_ATTEMPT_ID
REQUIRED_PRIOR_ATTEMPT_ID = result_bindings.REQUIRED_PRIOR_ATTEMPT_ID
REQUIRED_PRIOR_MANIFEST_SHA256 = result_bindings.REQUIRED_PRIOR_MANIFEST_SHA256
EXIT_RESUME_COMPLETED = 0
EXIT_RESUME_REJECTED = 10
EXIT_PRECONDITION_REJECTED = 11
EXIT_STATE_INDETERMINATE = 12


class FreshBootResumeError(ValueError):
    pass


@dataclass(frozen=True)
class FreshBootResumeRequest(
    resolution_control.FreshBootRecoveryResultResolutionRequest
):
    prior_fresh_boot_recovery_result_resolution_root: Path
    prior_fresh_boot_recovery_result_resolution_manifest_sha256: str
    prior_fresh_boot_recovery_result_resolution_attempt_id: str
    fresh_boot_resume_attempt_id: str
    resume_timeout_seconds: int
    evidence_settle_seconds: int
    authorized_install_artifacts_staged_fresh_boot_resume: bool
    authorized_bound_qualified_fresh_boot_recovery_result: bool
    authorized_one_private_fresh_boot_resume_delivery_and_execution: bool
    authorized_one_maintenance_resume_and_postflight: bool
    authorized_no_inventory_start_status_probe_retry_cleanup_stop_or_quit: bool

    @property
    def guest_fresh_boot_resume_root(self) -> str:
        return (
            "/var/tmp/radishlex-l6-v4-install-artifacts-staged-"
            f"fresh-boot-resume-{self.fresh_boot_resume_attempt_id}"
        )

    def _guest_file(self, name: str) -> str:
        return f"{self.guest_fresh_boot_resume_root}/{name}"

    @property
    def guest_frozen_resume_driver_incoming(self) -> str:
        return self._guest_file("frozen-resume-driver.incoming.py")

    @property
    def guest_frozen_resume_driver_path(self) -> str:
        return self._guest_file("frozen-resume-driver.py")

    @property
    def guest_frozen_recovery_probe_incoming(self) -> str:
        return self._guest_file("frozen-recovery-probe.incoming.py")

    @property
    def guest_frozen_recovery_probe_path(self) -> str:
        return self._guest_file("frozen-recovery-probe.py")

    @property
    def guest_fresh_boot_resume_driver_incoming(self) -> str:
        return self._guest_file("fresh-boot-resume-driver.incoming.py")

    @property
    def guest_fresh_boot_resume_driver_path(self) -> str:
        return self._guest_file("fresh-boot-resume-driver.py")

    @property
    def guest_fresh_boot_resume_marker_path(self) -> str:
        return self._guest_file("attempt.marker.json")

    @property
    def guest_fresh_boot_resume_phase_path(self) -> str:
        return self._guest_file("phase.json")

    @property
    def guest_fresh_boot_resume_terminal_path(self) -> str:
        return self._guest_file("resume.evidence.json")

    def validate(self) -> None:
        resolution_control.FreshBootRecoveryResultResolutionRequest.validate(self)
        root = self.prior_fresh_boot_recovery_result_resolution_root
        if not root.is_absolute() or ".." in root.parts:
            raise FreshBootResumeError(
                "prior-recovery-result-resolution-root-must-be-absolute-normalized"
            )
        if runtime_control._paths_overlap(self.output_root, root):
            raise FreshBootResumeError(
                "output-root-must-not-overlap-prior-recovery-result-resolution-root"
            )
        if (
            self.prior_fresh_boot_recovery_result_resolution_manifest_sha256
            != REQUIRED_PRIOR_MANIFEST_SHA256
            or self.prior_fresh_boot_recovery_result_resolution_attempt_id
            != REQUIRED_PRIOR_ATTEMPT_ID
            or self.fresh_boot_resume_attempt_id != REQUIRED_ATTEMPT_ID
        ):
            raise FreshBootResumeError("fixed-fresh-boot-resume-input-mismatch")
        if not 1 <= self.resume_timeout_seconds <= 30 * 60:
            raise FreshBootResumeError("resume-timeout-out-of-range")
        if not 1 <= self.evidence_settle_seconds <= 60:
            raise FreshBootResumeError("evidence-settle-out-of-range")
        for authorized, reason in (
            (
                self.authorized_install_artifacts_staged_fresh_boot_resume,
                "fresh-boot-resume-authorization-required",
            ),
            (
                self.authorized_bound_qualified_fresh_boot_recovery_result,
                "qualified-recovery-result-binding-authorization-required",
            ),
            (
                self.authorized_one_private_fresh_boot_resume_delivery_and_execution,
                "one-private-fresh-boot-resume-delivery-authorization-required",
            ),
            (
                self.authorized_one_maintenance_resume_and_postflight,
                "one-maintenance-resume-and-postflight-authorization-required",
            ),
            (
                self.authorized_no_inventory_start_status_probe_retry_cleanup_stop_or_quit,
                "fresh-boot-resume-forbidden-action-boundary-authorization-required",
            ),
        ):
            if not authorized:
                raise FreshBootResumeError(reason)

    def as_json(self) -> dict[str, object]:
        value = resolution_control.FreshBootRecoveryResultResolutionRequest.as_json(
            self
        )
        value.update(
            {
                "authorization": {
                    "bound_qualified_fresh_boot_recovery_result": True,
                    "install_artifacts_staged_fresh_boot_resume": True,
                    (
                        "no_inventory_start_status_probe_retry_cleanup_"
                        "stop_or_quit"
                    ): True,
                    "one_maintenance_resume_and_postflight": True,
                    "one_private_fresh_boot_resume_delivery_and_execution": True,
                },
                "evidence_settle_seconds": self.evidence_settle_seconds,
                "format": EVIDENCE_FORMAT,
                "fresh_boot_resume_attempt_id": self.fresh_boot_resume_attempt_id,
                "guest_fresh_boot_resume_root": self.guest_fresh_boot_resume_root,
                "prior_recovery_result_resolution_attempt_id": (
                    self.prior_fresh_boot_recovery_result_resolution_attempt_id
                ),
                "prior_recovery_result_resolution_manifest_sha256": (
                    self.prior_fresh_boot_recovery_result_resolution_manifest_sha256
                ),
                "resume_timeout_seconds": self.resume_timeout_seconds,
            }
        )
        return value


@dataclass(frozen=True)
class FreshBootResumeBinding:
    evidence: dict[str, object]
    upstream: result_bindings.FreshBootRecoveryResultResolutionResultBinding
    frozen_resume_driver_bytes: bytes
    frozen_recovery_probe_bytes: bytes
    fresh_boot_resume_driver_bytes: bytes
    current_boot_id_sha256: str
    prior_boot_id_sha256: str
    prior_backend_pid: int


@dataclass(frozen=True)
class FreshBootResumeResult:
    outcome: str
    exit_code: int
    evidence_root: Path
    manifest_sha256: str
    resume_invocations: int | str
    postflight_invocations: int | str


class CommandRunner(Protocol):
    def run(
        self,
        argv: tuple[str, ...],
        timeout_seconds: int,
        *,
        stdin_file=None,
    ) -> start_control.CommandObservation: ...


Sleeper = Callable[[float], None]


def validate_fresh_boot_resume_bindings(
    request: FreshBootResumeRequest,
) -> FreshBootResumeBinding:
    paths = (
        (Path(__file__).absolute(), CONTROL_RELATIVE_PATH, "control"),
        (Path(result_bindings.__file__).absolute(), BINDINGS_RELATIVE_PATH, "bindings"),
        (
            request.repository_root / FROZEN_RESUME_DRIVER_RELATIVE_PATH,
            FROZEN_RESUME_DRIVER_RELATIVE_PATH,
            "frozen-resume-driver",
        ),
        (
            request.repository_root / FROZEN_RECOVERY_PROBE_RELATIVE_PATH,
            FROZEN_RECOVERY_PROBE_RELATIVE_PATH,
            "frozen-recovery-probe",
        ),
        (
            Path(guest_driver.__file__).absolute(),
            FRESH_BOOT_RESUME_DRIVER_RELATIVE_PATH,
            "fresh-boot-resume-driver",
        ),
    )
    for actual, relative, label in paths:
        if actual != request.repository_root / relative:
            raise FreshBootResumeError(f"executed-{label}-path-mismatch")
        network_ready._require_committed_regular(actual, label)
    try:
        upstream = result_bindings.validate_fresh_boot_recovery_result_resolution_result_bindings(
            request
        )
    except ValueError as exc:
        raise FreshBootResumeError(str(exc)) from exc
    frozen_resume = (request.repository_root / FROZEN_RESUME_DRIVER_RELATIVE_PATH).read_bytes()
    frozen_recovery = (request.repository_root / FROZEN_RECOVERY_PROBE_RELATIVE_PATH).read_bytes()
    fresh_driver = (request.repository_root / FRESH_BOOT_RESUME_DRIVER_RELATIVE_PATH).read_bytes()
    if hashlib.sha256(frozen_resume).hexdigest() != guest_driver.EXPECTED_RESUME_DRIVER_SHA256:
        raise FreshBootResumeError("frozen-resume-driver-identity-drift")
    if hashlib.sha256(frozen_recovery).hexdigest() != guest_driver.EXPECTED_RECOVERY_PROBE_SHA256:
        raise FreshBootResumeError("frozen-recovery-probe-identity-drift")
    identities: dict[str, object] = {}
    for label, payload in (
        ("frozen_resume_driver", frozen_resume),
        ("frozen_recovery_probe", frozen_recovery),
        ("fresh_boot_resume_driver", fresh_driver),
    ):
        identities[f"{label}_sha256"] = hashlib.sha256(payload).hexdigest()
        identities[f"{label}_size"] = len(payload)
    return FreshBootResumeBinding(
        evidence={
            "current_boot_id_sha256": upstream.current_boot_id_sha256,
            "format": EVIDENCE_FORMAT,
            **identities,
            "fresh_boot_resume_attempt_id": request.fresh_boot_resume_attempt_id,
            "prior_boot_id_sha256": upstream.prior_boot_id_sha256,
            "prior_recovery_result_resolution_attempt_id": (
                request.prior_fresh_boot_recovery_result_resolution_attempt_id
            ),
            "prior_recovery_result_resolution_backend_pid": upstream.prior_backend_pid,
            "prior_recovery_result_resolution_entries_verified": upstream.evidence[
                "prior_recovery_result_resolution_entries_verified"
            ],
            "prior_recovery_result_resolution_manifest_sha256": (
                request.prior_fresh_boot_recovery_result_resolution_manifest_sha256
            ),
            "prior_recovery_result_resolution_outcome": "recovery-qualified",
            "repository_clean": True,
            "repository_head": request.expected_repository_head,
            "target_uuid": request.target_uuid,
        },
        upstream=upstream,
        frozen_resume_driver_bytes=frozen_resume,
        frozen_recovery_probe_bytes=frozen_recovery,
        fresh_boot_resume_driver_bytes=fresh_driver,
        current_boot_id_sha256=upstream.current_boot_id_sha256,
        prior_boot_id_sha256=upstream.prior_boot_id_sha256,
        prior_backend_pid=upstream.prior_backend_pid,
    )


def readiness_argv(request: FreshBootResumeRequest) -> tuple[str, ...]:
    return recovery_control.readiness_argv(request)


def driver_argv(
    request: FreshBootResumeRequest, binding: FreshBootResumeBinding
) -> tuple[str, ...]:
    return (
        "utmctl",
        "exec",
        request.target_uuid,
        "--cmd",
        "/usr/bin/python3",
        "-I",
        "-B",
        request.guest_fresh_boot_resume_driver_path,
        "--resume-attempt-id",
        request.fresh_boot_resume_attempt_id,
        "--checkpoint-attempt-id",
        guest_driver.EXPECTED_CHECKPOINT_ATTEMPT_ID,
        "--target-uuid",
        request.target_uuid,
        "--prior-boot-id-sha256",
        binding.prior_boot_id_sha256,
        "--current-boot-id-sha256",
        binding.current_boot_id_sha256,
        "--expected-operation-id-sha256",
        guest_driver.EXPECTED_OPERATION_ID_SHA256,
        "--expected-checkpoint-sha256",
        guest_driver.EXPECTED_CHECKPOINT_SHA256,
        "--expected-crash-state-sha256",
        guest_driver.EXPECTED_CRASH_STATE_SHA256,
        "--resume-driver",
        request.guest_frozen_resume_driver_path,
        "--expected-resume-driver-sha256",
        hashlib.sha256(binding.frozen_resume_driver_bytes).hexdigest(),
        "--recovery-probe",
        request.guest_frozen_recovery_probe_path,
        "--expected-recovery-probe-sha256",
        hashlib.sha256(binding.frozen_recovery_probe_bytes).hexdigest(),
        "--expected-driver-sha256",
        hashlib.sha256(binding.fresh_boot_resume_driver_bytes).hexdigest(),
        "--control-root",
        request.guest_fresh_boot_resume_root,
    )


def marker_bytes(
    request: FreshBootResumeRequest, binding: FreshBootResumeBinding
) -> bytes:
    return guest_driver.canonical_json(
        {
            "format": guest_driver.MARKER_FORMAT,
            "fresh_boot_resume_driver_sha256": hashlib.sha256(
                binding.fresh_boot_resume_driver_bytes
            ).hexdigest(),
            "recovery_probe_sha256": hashlib.sha256(
                binding.frozen_recovery_probe_bytes
            ).hexdigest(),
            "resume_attempt_id": request.fresh_boot_resume_attempt_id,
            "resume_driver_sha256": hashlib.sha256(
                binding.frozen_resume_driver_bytes
            ).hexdigest(),
        }
    )


def parse_guest_terminal(
    payload: bytes,
    request: FreshBootResumeRequest,
    binding: FreshBootResumeBinding,
) -> dict[str, object]:
    try:
        value = json.loads(payload.decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise FreshBootResumeError("guest-terminal-json-invalid") from exc
    if not isinstance(value, dict):
        raise FreshBootResumeError("guest-terminal-json-invalid")
    common = {
        "automatic_cleanup",
        "automatic_quit",
        "automatic_retry",
        "automatic_stop",
        "checkpoint_attempt_id",
        "current_boot_id_sha256",
        "dpkg_mutation_executed",
        "format",
        "maintenance_resume_invocations",
        "operation_id",
        "operation_id_sha256",
        "outcome",
        "phase",
        "postflight_invocations",
        "prior_boot_id_sha256",
        "reason",
        "resume_attempt_id",
        "terminal_case_sha256",
        "transaction",
        "transient_secret_reconstructed",
    }
    completed = {
        "checkpoint_sha256",
        "dpkg_delta_sha256",
        "dpkg_delta_size",
        "dpkg_log_sha256",
        "dpkg_log_size",
        "guard_profile_before_resume",
        "receipt_sha256",
        "receipt_size",
        "resume_result_sha256",
        "terminal_postflight_sha256",
    }
    outcome = value.get("outcome")
    if set(value) != (common | completed if outcome == "resume-completed" else common):
        raise FreshBootResumeError("guest-terminal-fields-invalid")
    if (
        value.get("format") != guest_driver.EVIDENCE_FORMAT
        or value.get("checkpoint_attempt_id")
        != guest_driver.EXPECTED_CHECKPOINT_ATTEMPT_ID
        or value.get("current_boot_id_sha256") != binding.current_boot_id_sha256
        or value.get("prior_boot_id_sha256") != binding.prior_boot_id_sha256
        or value.get("resume_attempt_id") != request.fresh_boot_resume_attempt_id
        or value.get("operation_id") != "reconstructed-secret-hash-only"
        or value.get("operation_id_sha256")
        != guest_driver.EXPECTED_OPERATION_ID_SHA256
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
        raise FreshBootResumeError("guest-terminal-semantics-invalid")
    if outcome == "resume-completed":
        if (
            value.get("phase") != "complete"
            or value.get("maintenance_resume_invocations") != 1
            or value.get("postflight_invocations") != 1
            or value.get("dpkg_mutation_executed") is not True
            or value.get("transaction") != "completed"
            or value.get("transient_secret_reconstructed") is not True
            or value.get("guard_profile_before_resume") != "absent-after-reboot"
            or value.get("checkpoint_sha256")
            != guest_driver.EXPECTED_CHECKPOINT_SHA256
        ):
            raise FreshBootResumeError("guest-completed-semantics-invalid")
        for key in (
            "dpkg_delta_sha256",
            "dpkg_log_sha256",
            "receipt_sha256",
            "resume_result_sha256",
            "terminal_postflight_sha256",
        ):
            if not isinstance(value.get(key), str) or not guest_driver.HEX_64.fullmatch(
                str(value[key])
            ):
                raise FreshBootResumeError(f"guest-completed-{key}-invalid")
        for key in ("dpkg_delta_size", "dpkg_log_size", "receipt_size"):
            if not isinstance(value.get(key), int) or int(value[key]) <= 0:
                raise FreshBootResumeError(f"guest-completed-{key}-invalid")
    elif outcome == "resume-rejected":
        if (
            value.get("maintenance_resume_invocations") != 0
            or value.get("postflight_invocations") != 0
            or value.get("dpkg_mutation_executed") is not False
            or value.get("transaction") != "artifacts-staged-preserved-no-resume"
            or not isinstance(value.get("reason"), str)
        ):
            raise FreshBootResumeError("guest-rejected-semantics-invalid")
    elif outcome == "state-indeterminate":
        if (
            value.get("maintenance_resume_invocations") != 1
            or value.get("transaction") != "state-indeterminate"
            or value.get("dpkg_mutation_executed") != "unknown"
            or not isinstance(value.get("reason"), str)
        ):
            raise FreshBootResumeError("guest-indeterminate-semantics-invalid")
    else:
        raise FreshBootResumeError("guest-terminal-outcome-invalid")
    return value


def run_fresh_boot_resume(
    request: FreshBootResumeRequest,
    *,
    runner: CommandRunner | None = None,
    binding_validator=None,
    source_opener=None,
    source_revalidator=None,
    target_validator=None,
    sleeper: Sleeper = time.sleep,
) -> FreshBootResumeResult:
    request.validate()
    writer = start_control.EvidenceWriter.create(request.output_root)
    writer.write_json("request.json", request.as_json())
    command_runner = runner or input_transfer.SubprocessCommandRunner()
    validate_bindings = binding_validator or validate_fresh_boot_resume_bindings
    open_source = source_opener or input_transfer.open_source_bundle
    revalidate_source = source_revalidator or input_transfer.revalidate_open_source_bundle
    validate_target = target_validator or network_ready.validate_target_files

    stage = "binding-preflight"
    reason = "not-run"
    outcome = "precondition-rejected"
    exit_code = EXIT_PRECONDITION_REJECTED
    source_bundle: input_transfer.SourceBundle | None = None
    target_before: dict[str, object] | None = None
    backend_pid: int | None = None
    terminal_processes: tuple[dict[str, object], ...] = ()
    identity_observation_count = 0
    agent_readiness_invocations = 0
    file_pull_invocations = 0
    file_push_invocations = 0
    guest_exec_invocations = 0
    driver_invoked = False
    resume_invocations: int | str = 0
    postflight_invocations: int | str = 0
    guest_terminal: dict[str, object] | None = None
    driver_transport_exit_code: int | None = None

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
                raise FreshBootResumeError(f"{name}-empty")
        else:
            input_transfer._require_exact_small_readback(observation, exact, name)
        return observation.stdout.prefix

    def observe_identity(name: str, *, discovery: bool = False) -> None:
        nonlocal backend_pid, terminal_processes, stage
        stage = name
        argv = (
            network_ready._lsof_argv(request)
            if discovery
            else runtime_control.targeted_lsof_argv(request, int(backend_pid))
        )
        observation = command_runner.run(argv, request.command_timeout_seconds)
        identity = runtime_control.parse_target_handle_process(
            observation, request, expected_argv=argv
        )
        observed_pid = int(identity["backend_pid"])
        if backend_pid is None:
            backend_pid = observed_pid
        elif observed_pid != backend_pid:
            raise FreshBootResumeError("target-backend-pid-drift")
        writer.write_json(
            f"{name}.json",
            runtime_control._handle_identity_evidence(observation, identity),
        )
        process_observation, terminal_processes = runtime_control._observe_processes(
            command_runner, request
        )
        writer.write_json(
            f"host-process-{name}.json",
            runtime_control._process_evidence(process_observation, terminal_processes),
        )
        runtime_control._require_no_utmctl(terminal_processes, name)

    try:
        binding = validate_bindings(request)
        writer.write_json("binding-preflight.json", binding.evidence)

        stage = "source-bundle-preflight"
        source_bundle = open_source(request)
        writer.write_json(stage + ".json", source_bundle.as_json())

        stage = "target-files-preflight"
        target_before = validate_target(request)
        writer.write_json(stage + ".json", target_before)

        observe_identity("target-handle-pid-discovery", discovery=True)
        if backend_pid != binding.prior_backend_pid:
            raise FreshBootResumeError("qualified-recovery-backend-pid-drift")
        for index in range(1, request.identity_observations + 1):
            identity_observation_count = index
            observe_identity(f"target-handle-pid-confirmation-{index:03d}")

        stage = "guest-agent-readiness"
        for index in range(1, request.agent_readiness_attempts + 1):
            agent_readiness_invocations = index
            readiness = command_runner.run(
                readiness_argv(request), request.command_timeout_seconds
            )
            writer.write_json(
                f"guest-agent-readiness-{index:03d}.json", readiness.as_json()
            )
            if guest_agent_control.classify_agent_readiness(readiness, request) == "ready":
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
            raise FreshBootResumeError("guest-agent-readiness-budget-exhausted")

        stage = "target-files-agent-ready"
        target_ready = validate_target(request)
        writer.write_json(stage + ".json", target_ready)
        runtime_control._require_live_target_identity_stable(target_before, target_ready)
        observe_identity("target-handle-agent-ready")

        stage = "guest-fresh-boot-resume-root-create"
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
                request.guest_fresh_boot_resume_root,
            ),
            request.command_timeout_seconds,
        )
        writer.write_json(stage + ".json", observation.as_json())
        input_transfer._require_successful_small_observation(observation, stage)

        deliveries = (
            (
                "frozen-resume-driver",
                FROZEN_RESUME_DRIVER_RELATIVE_PATH,
                request.guest_frozen_resume_driver_incoming,
                request.guest_frozen_resume_driver_path,
                binding.frozen_resume_driver_bytes,
            ),
            (
                "frozen-recovery-probe",
                FROZEN_RECOVERY_PROBE_RELATIVE_PATH,
                request.guest_frozen_recovery_probe_incoming,
                request.guest_frozen_recovery_probe_path,
                binding.frozen_recovery_probe_bytes,
            ),
            (
                "fresh-boot-resume-driver",
                FRESH_BOOT_RESUME_DRIVER_RELATIVE_PATH,
                request.guest_fresh_boot_resume_driver_incoming,
                request.guest_fresh_boot_resume_driver_path,
                binding.fresh_boot_resume_driver_bytes,
            ),
        )
        for label, relative, incoming, final, expected in deliveries:
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
                input_transfer._require_successful_small_observation(observation, stage)
            pull(final, f"guest-{label}-readback", exact=expected)

        stage = "target-files-pre-resume"
        target_pre_resume = validate_target(request)
        writer.write_json(stage + ".json", target_pre_resume)
        runtime_control._require_live_target_identity_stable(target_ready, target_pre_resume)
        observe_identity("target-handle-pre-resume")

        stage = "guest-fresh-boot-resume-once"
        guest_exec_invocations += 1
        driver_invoked = True
        resume_invocations = "unknown-after-driver-invocation"
        postflight_invocations = "unknown-after-driver-invocation"
        observation = command_runner.run(
            driver_argv(request, binding), request.resume_timeout_seconds
        )
        driver_transport_exit_code = observation.exit_code
        writer.write_json(
            stage + ".json",
            {
                "format": EVIDENCE_FORMAT,
                "observation": network_ready._observation_metadata(observation),
                "raw_output_persisted": False,
            },
        )
        sleeper(float(request.evidence_settle_seconds))
        pull(
            request.guest_fresh_boot_resume_marker_path,
            "guest-fresh-boot-resume-marker-readback",
            exact=marker_bytes(request, binding),
        )
        payloads = [
            pull(
                request.guest_fresh_boot_resume_terminal_path,
                f"guest-fresh-boot-resume-result-readback-{index}",
            )
            for index in (1, 2)
        ]
        terminal_payload = resume_evidence.require_equal_payloads(
            payloads, "guest-fresh-boot-resume-result"
        )
        guest_terminal = parse_guest_terminal(terminal_payload, request, binding)
        writer.write_json("guest-fresh-boot-resume-result.json", guest_terminal)
        expected_phase = {
            "resume-completed": "complete",
            "resume-rejected": "rejected",
            "state-indeterminate": "indeterminate",
        }[str(guest_terminal["outcome"])]
        pull(
            request.guest_fresh_boot_resume_phase_path,
            "guest-fresh-boot-resume-phase-readback",
            exact=guest_driver.canonical_json(
                {"format": guest_driver.PHASE_FORMAT, "phase": expected_phase}
            ),
        )
        resume_invocations = int(guest_terminal["maintenance_resume_invocations"])
        postflight_invocations = int(guest_terminal["postflight_invocations"])

        stage = "target-files-postflight"
        target_after = validate_target(request)
        writer.write_json(stage + ".json", target_after)
        runtime_control._require_live_target_identity_stable(target_pre_resume, target_after)
        observe_identity("target-handle-terminal")

        if guest_terminal["outcome"] == "resume-completed":
            outcome = "resume-completed"
            exit_code = EXIT_RESUME_COMPLETED
            reason = "one-shot-fresh-boot-install-artifacts-staged-resume-passed"
        elif guest_terminal["outcome"] == "resume-rejected":
            outcome = "resume-rejected"
            exit_code = EXIT_RESUME_REJECTED
            reason = f"guest-resume-rejected:{guest_terminal['reason']}"
        else:
            outcome = "state-indeterminate"
            exit_code = EXIT_STATE_INDETERMINATE
            reason = f"guest-resume-state-indeterminate:{guest_terminal['reason']}"
    except (
        FreshBootResumeError,
        input_transfer.CanonicalInputTransferError,
        network_ready.NetworkReadyError,
        recovery_control.RecoveryPreflightError,
        resume_evidence.ResumeEvidenceError,
        runtime_control.RuntimeResolutionError,
        start_control.StartControlError,
        OSError,
        tarfile.TarError,
        ValueError,
    ) as exc:
        reason = f"{stage}:{exc}"
        if driver_invoked:
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
                if driver_invoked:
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
            "driver_transport_exit_code": driver_transport_exit_code,
            "file_pull_invocations": file_pull_invocations,
            "file_push_invocations": file_push_invocations,
            "format": EVIDENCE_FORMAT,
            "fresh_boot_resume_attempt_id": request.fresh_boot_resume_attempt_id,
            "guest_exec_invocations": guest_exec_invocations,
            "guest_resume_outcome": guest_terminal.get("outcome") if guest_terminal else None,
            "identity_observation_count": identity_observation_count,
            "inventory_probe_invocations": 0,
            "maintenance_resume_invocations": resume_invocations,
            "operation_id": "reconstructed-secret-hash-only",
            "operation_id_sha256": guest_driver.EXPECTED_OPERATION_ID_SHA256,
            "outcome": outcome,
            "plain_utmctl_list": "not-performed",
            "plain_utmctl_start": "not-performed",
            "plain_utmctl_status": "not-performed",
            "postflight_invocations": postflight_invocations,
            "reason": reason,
            "target_name": request.target_name,
            "target_uuid": request.target_uuid,
            "terminal_relevant_host_process_count": len(terminal_processes),
            "transaction": (
                guest_terminal.get("transaction")
                if guest_terminal
                else ("state-indeterminate" if driver_invoked else "artifacts-staged-preserved-no-resume")
            ),
            "transport_exit_disambiguated_by_terminal": (
                guest_terminal is not None
                and driver_transport_exit_code
                != {"resume-completed": 0, "resume-rejected": 10, "state-indeterminate": 12}.get(
                    str(guest_terminal.get("outcome"))
                )
            ),
        },
    )
    runtime_control._require_no_raw_operation_id(request.output_root)
    manifest_sha256 = writer.write_manifest()
    return FreshBootResumeResult(
        outcome=outcome,
        exit_code=exit_code,
        evidence_root=request.output_root,
        manifest_sha256=manifest_sha256,
        resume_invocations=resume_invocations,
        postflight_invocations=postflight_invocations,
    )


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Consume one frozen recovery-qualified fresh-boot result and execute "
            "one fail-closed resume plus one postflight without retry or stop."
        )
    )
    parser.add_argument("command", choices=("resume-qualified-fresh-boot",))
    classification_control._add_chain_arguments(parser)
    parser.add_argument("--prior-fresh-boot-classification-root", type=Path, required=True)
    parser.add_argument("--prior-fresh-boot-classification-manifest-sha256", required=True)
    parser.add_argument("--prior-fresh-boot-classification-attempt-id", required=True)
    parser.add_argument("--prior-fresh-boot-recovery-preflight-root", type=Path, required=True)
    parser.add_argument("--prior-fresh-boot-recovery-preflight-manifest-sha256", required=True)
    parser.add_argument("--prior-fresh-boot-recovery-preflight-attempt-id", required=True)
    parser.add_argument("--result-resolution-attempt-id", required=True)
    parser.add_argument("--prior-fresh-boot-recovery-result-resolution-root", type=Path, required=True)
    parser.add_argument("--prior-fresh-boot-recovery-result-resolution-manifest-sha256", required=True)
    parser.add_argument("--prior-fresh-boot-recovery-result-resolution-attempt-id", required=True)
    parser.add_argument("--fresh-boot-resume-attempt-id", required=True)
    parser.add_argument("--resume-timeout-seconds", type=int, default=1500)
    parser.add_argument("--evidence-settle-seconds", type=int, default=10)
    for name in (
        "authorized-install-artifacts-staged-fresh-boot-resume",
        "authorized-bound-qualified-fresh-boot-recovery-result",
        "authorized-one-private-fresh-boot-resume-delivery-and-execution",
        "authorized-one-maintenance-resume-and-postflight",
        "authorized-no-inventory-start-status-probe-retry-cleanup-stop-or-quit",
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
            "authorized_install_artifacts_staged_fresh_boot_classification": True,
            "authorized_bound_prior_recovery_preflight_result": True,
            "authorized_bounded_fresh_boot_agent_readiness": True,
            "authorized_install_artifacts_staged_new_boot_recovery_preflight": True,
            "authorized_bound_prior_boot_classification_result": True,
            "authorized_bounded_read_only_recovery_probe": True,
            "authorized_one_private_recovery_probe_delivery_and_execution": True,
            "authorized_no_inventory_start_status_resume_dpkg_mutation_retry_stop_or_quit": True,
            "authorized_install_artifacts_staged_boot_classification_resolution": True,
            "authorized_bound_prior_guest_agent_result": True,
            "authorized_bounded_read_only_guest_agent_readiness": True,
            "authorized_install_artifacts_staged_fresh_boot_recovery_preflight": True,
            "authorized_bound_prior_fresh_boot_classification_result": True,
            "authorized_bounded_read_only_fresh_boot_recovery_probe": True,
            "authorized_one_private_fresh_boot_recovery_probe_delivery_and_execution": True,
            "authorized_install_artifacts_staged_fresh_boot_recovery_result_resolution": True,
            "authorized_bound_prior_fresh_boot_recovery_preflight_result": True,
            "authorized_two_existing_recovery_result_readbacks": True,
            "authorized_one_existing_recovery_phase_readback": True,
            "authorized_no_inventory_start_status_probe_push_exec_resume_dpkg_retry_stop_or_quit": True,
        }
    )
    request = FreshBootResumeRequest(**values)
    try:
        result = run_fresh_boot_resume(request)
    except (FreshBootResumeError, ValueError) as exc:
        print(f"fresh_boot_resume_error={exc}", file=sys.stderr)
        return EXIT_PRECONDITION_REJECTED
    print(f"fresh_boot_resume_outcome={result.outcome}")
    print(f"evidence_root={result.evidence_root}")
    print(f"files_sha256={result.manifest_sha256}")
    return result.exit_code


if __name__ == "__main__":
    raise SystemExit(main())
