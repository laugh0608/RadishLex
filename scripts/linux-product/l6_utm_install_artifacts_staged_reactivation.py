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
import l6_utm_install_artifacts_staged_backend_resolution as backend_control
import l6_utm_install_artifacts_staged_checkpoint as checkpoint_control
import l6_utm_install_artifacts_staged_reactivation_bindings as bindings
import l6_utm_install_artifacts_staged_resume_bindings as resume_bindings
import l6_utm_launch_transport_bindings as launch_bindings
import l6_utm_launch_transport_v2 as launch_transport
import l6_utm_start_once as start_control
import l6_v4_canonical_input_install as guest_installer


EVIDENCE_FORMAT = bindings.EVIDENCE_FORMAT
CONTROL_RELATIVE_PATH = bindings.CONTROL_RELATIVE_PATH
BINDINGS_RELATIVE_PATH = bindings.BINDINGS_RELATIVE_PATH
REQUIRED_PRIOR_V7_MANIFEST_SHA256 = launch_bindings.REQUIRED_V7_MANIFEST_SHA256
REQUIRED_PRIOR_PREPARED_MANIFEST_SHA256 = (
    launch_bindings.REQUIRED_PREPARED_MANIFEST_SHA256
)
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
    backend_control.REQUIRED_PRIOR_RESUME_FAILURE_MANIFEST_SHA256
)
REQUIRED_PRIOR_BACKEND_RESOLUTION_MANIFEST_SHA256 = (
    bindings.REQUIRED_PRIOR_BACKEND_RESOLUTION_MANIFEST_SHA256
)
REQUIRED_TRANSFER_ATTEMPT_ID = checkpoint_control.REQUIRED_TRANSFER_ATTEMPT_ID
REQUIRED_RESOLUTION_ATTEMPT_ID = (
    checkpoint_control.REQUIRED_RESOLUTION_ATTEMPT_ID
)
REQUIRED_PREFLIGHT_ATTEMPT_ID = checkpoint_control.REQUIRED_PREFLIGHT_ATTEMPT_ID
REQUIRED_CHECKPOINT_ATTEMPT_ID = resume_bindings.REQUIRED_CHECKPOINT_ATTEMPT_ID
REQUIRED_RESUME_ATTEMPT_ID = resume_bindings.REQUIRED_RESUME_ATTEMPT_ID
REQUIRED_BACKEND_RESOLUTION_ATTEMPT_ID = (
    backend_control.REQUIRED_BACKEND_RESOLUTION_ATTEMPT_ID
)
REQUIRED_REACTIVATION_ATTEMPT_ID = bindings.REQUIRED_REACTIVATION_ATTEMPT_ID
REQUIRED_SOURCE_BUNDLE_SIZE = checkpoint_control.REQUIRED_SOURCE_BUNDLE_SIZE
REQUIRED_SOURCE_BUNDLE_SHA256 = checkpoint_control.REQUIRED_SOURCE_BUNDLE_SHA256
REQUIRED_TARGET_UUID = checkpoint_control.REQUIRED_TARGET_UUID
REQUIRED_TARGET_NAME = checkpoint_control.REQUIRED_TARGET_NAME
PROCESS_COMMAND = launch_transport.PROCESS_COMMAND
BOOT_ID_PATH = "/proc/sys/kernel/random/boot_id"
BOOT_ID_HASH_PROGRAM = (
    "import hashlib,pathlib,uuid;"
    f"raw=pathlib.Path('{BOOT_ID_PATH}').read_text(encoding='ascii');"
    "value=raw[:-1] if raw.endswith('\\n') else raw;"
    "assert '\\n' not in value and str(uuid.UUID(value)) == value;"
    "print(hashlib.sha256(value.encode('ascii')).hexdigest())"
)
EXIT_ORIGINAL_BOOT_RESTORED = 0
EXIT_NEW_BOOT_STARTED = 10
EXIT_PRECONDITION_REJECTED = 11
EXIT_STATE_INDETERMINATE = 12
HEX_40 = re.compile(r"[0-9a-f]{40}")
HEX_64 = re.compile(r"[0-9a-f]{64}")
SAFE_ATTEMPT_ID = re.compile(r"[a-z0-9][a-z0-9-]{0,95}")


class ReactivationError(ValueError):
    pass


@dataclass(frozen=True)
class ReactivationRequest:
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
    target_uuid: str
    target_name: str
    target_package_path: Path
    expected_vm_count: int
    settle_attempts: int
    runtime_poll_attempts: int
    poll_interval_seconds: int
    transport_timeout_seconds: int
    command_timeout_seconds: int
    authorized_install_artifacts_staged_reactivation: bool
    authorized_one_potential_backend_reactivation_list: bool
    authorized_one_foreground_start_if_stopped_quiescent: bool
    authorized_one_read_only_boot_id_hash: bool
    authorized_no_business_guest_resume_retry_stop_or_quit: bool

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
            (self.source_bundle_path, "source-bundle-path"),
            (self.output_root, "output-root"),
            (self.target_package_path, "target-package-path"),
        )
        for path, label in paths:
            if not path.is_absolute() or ".." in path.parts:
                raise ReactivationError(f"{label}-must-be-absolute-normalized")
        for path, label in paths:
            if label != "output-root" and _paths_overlap(self.output_root, path):
                raise ReactivationError(f"output-root-must-not-overlap-{label}")
        if not HEX_40.fullmatch(self.expected_repository_head):
            raise ReactivationError("expected-repository-head-invalid")
        fixed = (
            (
                self.prior_v7_manifest_sha256,
                REQUIRED_PRIOR_V7_MANIFEST_SHA256,
                "prior-v7-manifest",
            ),
            (
                self.prior_prepared_manifest_sha256,
                REQUIRED_PRIOR_PREPARED_MANIFEST_SHA256,
                "prior-prepared-manifest",
            ),
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
                self.prior_backend_resolution_manifest_sha256,
                REQUIRED_PRIOR_BACKEND_RESOLUTION_MANIFEST_SHA256,
                "prior-backend-resolution-manifest",
            ),
            (
                self.source_bundle_sha256,
                REQUIRED_SOURCE_BUNDLE_SHA256,
                "source-bundle-sha256",
            ),
        )
        for actual, expected, label in fixed:
            if actual != expected:
                raise ReactivationError(f"required-{label}-mismatch")
        if self.source_bundle_size != REQUIRED_SOURCE_BUNDLE_SIZE:
            raise ReactivationError("required-source-bundle-size-mismatch")
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
            (
                self.reactivation_attempt_id,
                REQUIRED_REACTIVATION_ATTEMPT_ID,
                "reactivation",
            ),
        )
        for actual, expected, label in attempts:
            if actual != expected or not SAFE_ATTEMPT_ID.fullmatch(actual):
                raise ReactivationError(f"required-{label}-attempt-id-mismatch")
        if len({item[0] for item in attempts}) != len(attempts):
            raise ReactivationError("attempt-id-overlap")
        try:
            canonical_uuid = str(uuid.UUID(self.target_uuid)).upper()
        except ValueError as exc:
            raise ReactivationError("target-uuid-invalid") from exc
        if (
            canonical_uuid != self.target_uuid
            or self.target_uuid != REQUIRED_TARGET_UUID
        ):
            raise ReactivationError("required-target-uuid-mismatch")
        if self.target_name != REQUIRED_TARGET_NAME:
            raise ReactivationError("required-target-name-mismatch")
        if self.target_package_path != launch_bindings.expected_target_package_path(
            self.target_name
        ):
            raise ReactivationError("target-package-path-mismatch")
        if self.expected_vm_count != 21:
            raise ReactivationError("expected-vm-count-must-be-21")
        for value, low, high, label in (
            (self.settle_attempts, 1, 60, "settle-attempts"),
            (self.runtime_poll_attempts, 1, 120, "runtime-poll-attempts"),
            (self.poll_interval_seconds, 1, 10, "poll-interval"),
            (self.transport_timeout_seconds, 1, 300, "transport-timeout"),
            (self.command_timeout_seconds, 1, 60, "command-timeout"),
        ):
            if not low <= value <= high:
                raise ReactivationError(f"{label}-out-of-range")
        authorizations = (
            (
                self.authorized_install_artifacts_staged_reactivation,
                "authorized-install-artifacts-staged-reactivation-required",
            ),
            (
                self.authorized_one_potential_backend_reactivation_list,
                "authorized-one-potential-backend-reactivation-list-required",
            ),
            (
                self.authorized_one_foreground_start_if_stopped_quiescent,
                "authorized-one-foreground-start-if-stopped-quiescent-required",
            ),
            (
                self.authorized_one_read_only_boot_id_hash,
                "authorized-one-read-only-boot-id-hash-required",
            ),
            (
                self.authorized_no_business_guest_resume_retry_stop_or_quit,
                "authorized-no-business-guest-resume-retry-stop-or-quit-required",
            ),
        )
        for authorized, reason in authorizations:
            if not authorized:
                raise ReactivationError(reason)

    def as_json(self) -> dict[str, object]:
        return {
            "authorization": {
                "install_artifacts_staged_reactivation": True,
                "no_business_guest_resume_retry_stop_or_quit": True,
                "one_foreground_start_if_stopped_quiescent": True,
                "one_potential_backend_reactivation_list": True,
                "one_read_only_boot_id_hash": True,
            },
            "backend_resolution_attempt_id": self.backend_resolution_attempt_id,
            "checkpoint_attempt_id": self.checkpoint_attempt_id,
            "command_timeout_seconds": self.command_timeout_seconds,
            "expected_repository_head": self.expected_repository_head,
            "expected_vm_count": self.expected_vm_count,
            "format": EVIDENCE_FORMAT,
            "poll_interval_seconds": self.poll_interval_seconds,
            "prior_backend_resolution_manifest_sha256": (
                self.prior_backend_resolution_manifest_sha256
            ),
            "prior_checkpoint_manifest_sha256": self.prior_checkpoint_manifest_sha256,
            "prior_network_manifest_sha256": self.prior_network_manifest_sha256,
            "prior_preflight_manifest_sha256": self.prior_preflight_manifest_sha256,
            "prior_prepared_manifest_sha256": self.prior_prepared_manifest_sha256,
            "prior_resolution_manifest_sha256": self.prior_resolution_manifest_sha256,
            "prior_resume_failure_manifest_sha256": (
                self.prior_resume_failure_manifest_sha256
            ),
            "prior_transfer_manifest_sha256": self.prior_transfer_manifest_sha256,
            "prior_v7_manifest_sha256": self.prior_v7_manifest_sha256,
            "reactivation_attempt_id": self.reactivation_attempt_id,
            "resolution_attempt_id": self.resolution_attempt_id,
            "resume_attempt_id": self.resume_attempt_id,
            "runtime_poll_attempts": self.runtime_poll_attempts,
            "settle_attempts": self.settle_attempts,
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
            "transport_id": launch_bindings.TRANSPORT_ID,
            "transport_timeout_seconds": self.transport_timeout_seconds,
        }


@dataclass(frozen=True)
class ReactivationResult:
    outcome: str
    exit_code: int
    evidence_root: Path
    manifest_sha256: str
    inventory_probe_invocations: int
    foreground_start_invocations: int
    guest_boot_hash_invocations: int


class CommandRunner(Protocol):
    def run(
        self, argv: tuple[str, ...], timeout_seconds: int
    ) -> start_control.CommandObservation: ...


Sleeper = Callable[[float], None]


def run_reactivation(
    request: ReactivationRequest,
    *,
    runner: CommandRunner | None = None,
    binding_validator=None,
    source_opener=None,
    source_revalidator=None,
    target_validator=None,
    sleeper: Sleeper = time.sleep,
) -> ReactivationResult:
    request.validate()
    writer = start_control.EvidenceWriter.create(request.output_root)
    writer.write_json("request.json", request.as_json())
    command_runner = runner or start_control.SubprocessCommandRunner()
    validate_bindings = binding_validator or validate_reactivation_bindings
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
    foreground_start_invocations = 0
    guest_boot_hash_invocations = 0
    settle_observation_count = 0
    runtime_poll_count = 0
    source_bundle: input_transfer.SourceBundle | None = None
    target_preflight: dict[str, object] | None = None
    registered_status: str | None = None
    inventory_error: str | None = None
    runtime_source = "not-observed"
    terminal_processes: tuple[dict[str, object], ...] = ()
    terminal_handles = "not-observed"
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
        if terminal_processes:
            raise ReactivationError("relevant-host-process-before-inventory-probe")

        stage = "target-handles-preflight"
        handles_observation, terminal_handles, handles = _observe_handles(
            command_runner, request
        )
        writer.write_json(
            "target-handles-preflight.json",
            _handles_evidence(handles_observation, terminal_handles, handles),
        )
        if terminal_handles != "absent":
            raise ReactivationError("target-handles-before-inventory-probe")

        stage = "utmctl-list-once"
        inventory_probe_invocations = 1
        list_observation = command_runner.run(
            ("utmctl", "list"), request.command_timeout_seconds
        )
        writer.write_json("utmctl-list-once.json", list_observation.as_json())
        try:
            inventory = start_control.parse_utmctl_list(list_observation)
            registered_status = _require_inventory(
                inventory, binding.baseline_inventory, request
            )
            writer.write_json(
                "inventory-classification.json",
                {
                    "format": EVIDENCE_FORMAT,
                    "other_registered_vm_count": len(inventory) - 1,
                    "other_registered_vms": "all-stopped",
                    "registered_vm_count": len(inventory),
                    "target_registered_status": registered_status,
                    "target_uuid": request.target_uuid,
                },
            )
        except (ReactivationError, start_control.StartControlError) as exc:
            inventory_error = str(exc)

        stage = "post-list-settle"
        all_settle_quiescent = True
        runtime_observed = False
        for attempt in range(1, request.settle_attempts + 1):
            settle_observation_count = attempt
            process_observation, terminal_processes = _observe_processes(
                command_runner, request
            )
            writer.write_json(
                f"host-process-settle-{attempt:03d}.json",
                _process_evidence(process_observation, terminal_processes),
            )
            handles_observation, terminal_handles, handles = _observe_handles(
                command_runner, request
            )
            writer.write_json(
                f"target-handles-settle-{attempt:03d}.json",
                _handles_evidence(
                    handles_observation, terminal_handles, handles
                ),
            )
            if terminal_processes or terminal_handles != "absent":
                all_settle_quiescent = False
            if _runtime_is_exact(terminal_processes, terminal_handles):
                runtime_observed = True
                runtime_source = "inventory-probe-reactivated"
                break
            if attempt < request.settle_attempts:
                sleeper(float(request.poll_interval_seconds))

        if inventory_error is not None:
            raise ReactivationError(f"inventory-unresolved:{inventory_error}")
        if registered_status == "started":
            if not runtime_observed:
                raise ReactivationError(
                    "registered-started-without-bounded-target-runtime"
                )
        elif registered_status == "stopped":
            if not all_settle_quiescent:
                raise ReactivationError(
                    "registered-stopped-with-nonquiescent-post-list-state"
                )
            stage = "target-files-ready"
            target_ready = validate_target(request)
            writer.write_json("target-files-ready.json", target_ready)
            if target_preflight != target_ready:
                raise ReactivationError("target-identity-changed-before-start")

            stage = "host-process-ready"
            process_observation, terminal_processes = _observe_processes(
                command_runner, request
            )
            writer.write_json(
                "host-process-ready.json",
                _process_evidence(process_observation, terminal_processes),
            )
            if terminal_processes:
                raise ReactivationError("relevant-host-process-before-start")

            stage = "target-handles-ready"
            handles_observation, terminal_handles, handles = _observe_handles(
                command_runner, request
            )
            writer.write_json(
                "target-handles-ready.json",
                _handles_evidence(
                    handles_observation, terminal_handles, handles
                ),
            )
            if terminal_handles != "absent":
                raise ReactivationError("target-handles-before-start")

            stage = "foreground-start-once"
            foreground_start_invocations = 1
            transport_observation = command_runner.run(
                launch_transport.transport_argv(request),
                request.transport_timeout_seconds,
            )
            writer.write_json(
                "foreground-start-once.json", transport_observation.as_json()
            )

            stage = "post-start-runtime"
            runtime_observed = False
            for attempt in range(1, request.runtime_poll_attempts + 1):
                runtime_poll_count = attempt
                process_observation, terminal_processes = _observe_processes(
                    command_runner, request
                )
                writer.write_json(
                    f"host-process-runtime-{attempt:03d}.json",
                    _process_evidence(
                        process_observation, terminal_processes
                    ),
                )
                handles_observation, terminal_handles, handles = _observe_handles(
                    command_runner, request
                )
                writer.write_json(
                    f"target-handles-runtime-{attempt:03d}.json",
                    _handles_evidence(
                        handles_observation, terminal_handles, handles
                    ),
                )
                if _runtime_is_exact(terminal_processes, terminal_handles):
                    runtime_observed = True
                    runtime_source = "foreground-start"
                    break
                if attempt < request.runtime_poll_attempts:
                    sleeper(float(request.poll_interval_seconds))
            if not runtime_observed:
                raise ReactivationError(
                    "foreground-start-without-bounded-target-runtime"
                )
        else:
            raise ReactivationError("target-registered-status-unresolved")

        stage = "guest-boot-id-hash-once"
        guest_boot_hash_invocations = 1
        boot_observation = command_runner.run(
            boot_id_hash_argv(request), request.command_timeout_seconds
        )
        observed_boot_hash = parse_boot_id_hash(boot_observation)
        if observed_boot_hash == binding.expected_boot_id_sha256:
            boot_classification = "original-boot-restored"
            outcome = "original-boot-restored"
            exit_code = EXIT_ORIGINAL_BOOT_RESTORED
            reason = "host-runtime-qualified-and-original-boot-hash-observed"
        else:
            boot_classification = "new-boot-started"
            outcome = "new-boot-started"
            exit_code = EXIT_NEW_BOOT_STARTED
            reason = "host-runtime-qualified-and-new-boot-hash-observed"
        writer.write_json(
            "guest-boot-id-hash-once.json",
            {
                "classification": boot_classification,
                "expected_boot_id_sha256": binding.expected_boot_id_sha256,
                "format": EVIDENCE_FORMAT,
                "observation": network_ready._observation_metadata(
                    boot_observation
                ),
                "observed_boot_id_sha256": observed_boot_hash,
            },
        )
    except (
        ReactivationError,
        input_transfer.CanonicalInputTransferError,
        network_ready.NetworkReadyError,
        start_control.StartControlError,
        launch_transport.LaunchTransportError,
        launch_bindings.BindingError,
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
                if inventory_probe_invocations == 1:
                    outcome = "state-indeterminate"
                    exit_code = EXIT_STATE_INDETERMINATE
                else:
                    outcome = "precondition-rejected"
                    exit_code = EXIT_PRECONDITION_REJECTED
            source_bundle.file_object.close()

    terminal = {
        "automatic_cleanup": "not-performed",
        "automatic_quit": "not-performed",
        "automatic_retry": "not-performed",
        "automatic_stop": "not-performed",
        "business_guest_action": "not-performed",
        "file_pull_invocations": 0,
        "file_push_invocations": 0,
        "foreground_start_invocations": foreground_start_invocations,
        "format": EVIDENCE_FORMAT,
        "guest_boot_hash_invocations": guest_boot_hash_invocations,
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
        "reactivation_attempt_id": request.reactivation_attempt_id,
        "reason": reason,
        "registered_status_from_inventory": registered_status,
        "runtime_observation_source": runtime_source,
        "runtime_poll_count": runtime_poll_count,
        "settle_observation_count": settle_observation_count,
        "target_handles_terminal": terminal_handles,
        "target_name": request.target_name,
        "target_uuid": request.target_uuid,
        "terminal_relevant_host_process_count": len(terminal_processes),
        "transaction": "artifacts-staged-preserved-no-resume",
    }
    writer.write_json("terminal.json", terminal)
    manifest_sha256 = writer.write_manifest()
    return ReactivationResult(
        outcome=outcome,
        exit_code=exit_code,
        evidence_root=request.output_root,
        manifest_sha256=manifest_sha256,
        inventory_probe_invocations=inventory_probe_invocations,
        foreground_start_invocations=foreground_start_invocations,
        guest_boot_hash_invocations=guest_boot_hash_invocations,
    )


def validate_reactivation_bindings(
    request: ReactivationRequest,
) -> bindings.ReactivationBinding:
    if Path(__file__).absolute() != request.repository_root / CONTROL_RELATIVE_PATH:
        raise ReactivationError("executed-control-path-mismatch")
    if Path(bindings.__file__).absolute() != (
        request.repository_root / BINDINGS_RELATIVE_PATH
    ):
        raise ReactivationError("executed-bindings-path-mismatch")
    try:
        return bindings.validate_reactivation_bindings(request)
    except ValueError as exc:
        raise ReactivationError(str(exc)) from exc


def boot_id_hash_argv(request: ReactivationRequest) -> tuple[str, ...]:
    return (
        "utmctl",
        "exec",
        request.target_uuid,
        "--cmd",
        "/usr/bin/python3",
        "-I",
        "-c",
        BOOT_ID_HASH_PROGRAM,
    )


def parse_boot_id_hash(
    observation: start_control.CommandObservation,
) -> str:
    try:
        start_control._require_successful_observation(
            observation, "guest-boot-id-hash"
        )
    except start_control.StartControlError as exc:
        raise ReactivationError(str(exc)) from exc
    payload = observation.stdout.prefix
    if len(payload) != 65 or not payload.endswith(b"\n"):
        raise ReactivationError("guest-boot-id-hash-output-invalid")
    try:
        digest = payload[:-1].decode("ascii")
    except UnicodeDecodeError as exc:
        raise ReactivationError("guest-boot-id-hash-not-ascii") from exc
    if not HEX_64.fullmatch(digest):
        raise ReactivationError("guest-boot-id-hash-invalid")
    return digest


def _observe_processes(
    runner: CommandRunner, request: ReactivationRequest
) -> tuple[
    start_control.CommandObservation, tuple[dict[str, object], ...]
]:
    observation = runner.run(PROCESS_COMMAND, request.command_timeout_seconds)
    return observation, launch_transport.parse_relevant_processes(observation)


def _observe_handles(
    runner: CommandRunner, request: ReactivationRequest
) -> tuple[start_control.CommandObservation, str, dict[str, object]]:
    observation = runner.run(
        network_ready._lsof_argv(request), request.command_timeout_seconds
    )
    try:
        state, handles = backend_control.classify_target_handles(
            observation, request
        )
    except backend_control.BackendResolutionError as exc:
        raise ReactivationError(str(exc)) from exc
    return observation, state, handles


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


def _require_inventory(
    registered: tuple[start_control.RegisteredVm, ...],
    baseline: tuple[start_control.RegisteredVm, ...],
    request: ReactivationRequest,
) -> str:
    for status in ("stopped", "started"):
        if launch_transport.inventory_matches_relative_to_v7(
            registered, baseline, request, target_status=status
        ):
            return status
    raise ReactivationError("inventory-not-v7-plus-exact-target-all-others-stopped")


def _runtime_is_exact(
    processes: tuple[dict[str, object], ...], handles_state: str
) -> bool:
    return (
        launch_transport._backend_process_count(processes) >= 1
        and launch_transport._role_count(processes, "utmctl") == 0
        and handles_state == "present"
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
            "Run one authorized global inventory probe, at most one "
            "foreground target start, and exactly one read-only boot hash "
            "observation before any business guest action."
        )
    )
    parser.add_argument("command", choices=("reactivate-once",))
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
        "source-bundle-sha256",
        "transfer-attempt-id",
        "resolution-attempt-id",
        "preflight-attempt-id",
        "checkpoint-attempt-id",
        "resume-attempt-id",
        "backend-resolution-attempt-id",
        "reactivation-attempt-id",
        "target-uuid",
        "target-name",
    ):
        parser.add_argument(f"--{name}", required=True)
    parser.add_argument("--source-bundle-size", type=int, required=True)
    parser.add_argument("--expected-vm-count", type=int, default=21)
    parser.add_argument("--settle-attempts", type=int, default=10)
    parser.add_argument("--runtime-poll-attempts", type=int, default=30)
    parser.add_argument("--poll-interval-seconds", type=int, default=1)
    parser.add_argument("--transport-timeout-seconds", type=int, default=60)
    parser.add_argument("--command-timeout-seconds", type=int, default=60)
    parser.add_argument(
        "--authorized-install-artifacts-staged-reactivation",
        action="store_true",
    )
    parser.add_argument(
        "--authorized-one-potential-backend-reactivation-list",
        action="store_true",
    )
    parser.add_argument(
        "--authorized-one-foreground-start-if-stopped-quiescent",
        action="store_true",
    )
    parser.add_argument(
        "--authorized-one-read-only-boot-id-hash", action="store_true"
    )
    parser.add_argument(
        "--authorized-no-business-guest-resume-retry-stop-or-quit",
        action="store_true",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    values = vars(args).copy()
    values.pop("command")
    request = ReactivationRequest(**values)
    try:
        result = run_reactivation(request)
    except ReactivationError as exc:
        print(
            f"reactivation rejected before evidence creation: {exc}",
            file=sys.stderr,
        )
        return EXIT_PRECONDITION_REJECTED
    print(f"outcome={result.outcome}")
    print(f"evidence_root={result.evidence_root}")
    print(f"manifest_sha256={result.manifest_sha256}")
    print(f"inventory_probe_invocations={result.inventory_probe_invocations}")
    print(f"foreground_start_invocations={result.foreground_start_invocations}")
    print(f"guest_boot_hash_invocations={result.guest_boot_hash_invocations}")
    return result.exit_code


if __name__ == "__main__":
    raise SystemExit(main())
