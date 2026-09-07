#!/usr/bin/env python3
from __future__ import annotations

import tarfile
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Protocol

import l6_utm_canonical_input_transfer as input_transfer
import l6_utm_guest_network_ready as network_ready
import l6_utm_install_artifacts_staged_backend_resolution as backend_control
import l6_utm_install_artifacts_staged_boot_start_resolution as boot_control
import l6_utm_install_artifacts_staged_reactivation as reactivation_control
import l6_utm_install_artifacts_staged_runtime_resolution as runtime_control
import l6_utm_install_artifacts_staged_transaction_state_resolution as transaction_control
import l6_utm_install_artifacts_staged_transaction_state_result_resolution_result_bindings as completed_bindings
import l6_utm_launch_transport_v2 as launch_transport
import l6_utm_start_once as start_control
import l6_v4_canonical_input_install as guest_installer


EVIDENCE_FORMAT = (
    "radishlex-linux-l6-utm-install-artifacts-staged-terminal-stop-v1"
)
CONTROL_RELATIVE_PATH = Path(
    "scripts/linux-product/"
    "l6_utm_install_artifacts_staged_terminal_stop.py"
)
REQUIRED_ATTEMPT_ID = (
    "d75818f-v4-install-artifacts-staged-terminal-stop-20260829-v1"
)
EXIT_STOPPED_VERIFIED = 0
EXIT_PRECONDITION_REJECTED = 11
EXIT_STATE_INDETERMINATE = 12


class TerminalStopError(ValueError):
    pass


class TerminalStopRequest(
    completed_bindings.TransactionStateResultResolutionResultBindingRequest,
    Protocol,
):
    output_root: Path
    terminal_stop_attempt_id: str
    stop_poll_attempts: int
    stop_poll_interval_seconds: int
    stop_timeout_seconds: int
    authorized_terminal_stop_control: bool
    authorized_bound_completed_transaction_result: bool
    authorized_one_preflight_and_one_terminal_inventory: bool
    authorized_at_most_one_graceful_stop_request: bool
    authorized_bounded_host_only_quiescence_observation: bool
    authorized_no_status_start_guest_file_resume_dpkg_retry_repair_cleanup_force_kill_or_quit: (
        bool
    )


@dataclass(frozen=True)
class TerminalStopBinding:
    evidence: dict[str, object]
    upstream: (
        completed_bindings.TransactionStateResultResolutionResultBinding
    )
    baseline_inventory: tuple[start_control.RegisteredVm, ...]
    prior_backend_pid: int
    prior_handle_sha256: str
    prior_handle_size: int


@dataclass(frozen=True)
class TerminalStopResult:
    outcome: str
    exit_code: int
    evidence_root: Path
    manifest_sha256: str
    preflight_inventory_invocations: int
    terminal_inventory_invocations: int
    graceful_stop_invocations: int
    quiescence_poll_count: int


class CommandRunner(Protocol):
    def run(
        self,
        argv: tuple[str, ...],
        timeout_seconds: int,
        *,
        stdin_file=None,
    ) -> start_control.CommandObservation: ...


def validate_terminal_stop_request(request: TerminalStopRequest) -> None:
    for path, label in (
        (request.repository_root, "repository-root"),
        (request.output_root, "output-root"),
        (
            request.prior_transaction_state_result_resolution_root,
            "prior-transaction-state-result-resolution-root",
        ),
        (request.target_package_path, "target-package-path"),
        (request.source_bundle_path, "source-bundle-path"),
    ):
        if not path.is_absolute() or ".." in path.parts:
            raise TerminalStopError(f"{label}-must-be-absolute-normalized")
    if runtime_control._paths_overlap(
        request.output_root, request.repository_root
    ):
        raise TerminalStopError("output-root-must-be-outside-repository")
    if runtime_control._paths_overlap(
        request.output_root,
        request.prior_transaction_state_result_resolution_root,
    ):
        raise TerminalStopError(
            "output-root-must-not-overlap-completed-result-root"
        )
    if request.output_root == request.target_package_path or (
        runtime_control._paths_overlap(
            request.output_root, request.target_package_path
        )
    ):
        raise TerminalStopError("output-root-must-not-overlap-target-package")
    if request.terminal_stop_attempt_id != REQUIRED_ATTEMPT_ID:
        raise TerminalStopError("required-terminal-stop-attempt-id-mismatch")
    if not start_control.HEX_40.fullmatch(request.expected_repository_head):
        raise TerminalStopError("expected-repository-head-invalid")
    if (
        request.prior_transaction_state_result_resolution_attempt_id
        != completed_bindings.REQUIRED_PRIOR_ATTEMPT_ID
        or request.prior_transaction_state_result_resolution_manifest_sha256
        != completed_bindings.REQUIRED_PRIOR_MANIFEST_SHA256
    ):
        raise TerminalStopError("required-completed-result-input-mismatch")
    try:
        launch_transport._validate_uuid(request.target_uuid, "target-uuid")
        launch_transport._validate_vm_name(request.target_name, "target-name")
    except launch_transport.LaunchTransportError as exc:
        raise TerminalStopError(str(exc)) from exc
    if request.expected_vm_count != 21:
        raise TerminalStopError("expected-vm-count-must-be-exactly-21")
    if request.identity_observations != 3:
        raise TerminalStopError("identity-observations-must-be-exactly-3")
    if request.stop_poll_attempts != 60:
        raise TerminalStopError("stop-poll-attempts-must-be-exactly-60")
    if request.stop_poll_interval_seconds != 1:
        raise TerminalStopError(
            "stop-poll-interval-seconds-must-be-exactly-1"
        )
    if request.stop_timeout_seconds != 120:
        raise TerminalStopError("stop-timeout-seconds-must-be-exactly-120")
    if request.command_timeout_seconds != 60:
        raise TerminalStopError("command-timeout-seconds-must-be-exactly-60")
    for authorized, reason in (
        (
            request.authorized_terminal_stop_control,
            "terminal-stop-control-authorization-required",
        ),
        (
            request.authorized_bound_completed_transaction_result,
            "bound-completed-transaction-result-authorization-required",
        ),
        (
            request.authorized_one_preflight_and_one_terminal_inventory,
            "two-inventory-boundary-authorization-required",
        ),
        (
            request.authorized_at_most_one_graceful_stop_request,
            "at-most-one-graceful-stop-authorization-required",
        ),
        (
            request.authorized_bounded_host_only_quiescence_observation,
            "bounded-host-quiescence-authorization-required",
        ),
        (
            request.authorized_no_status_start_guest_file_resume_dpkg_retry_repair_cleanup_force_kill_or_quit,
            "terminal-stop-forbidden-action-boundary-authorization-required",
        ),
    ):
        if not authorized:
            raise TerminalStopError(reason)


def terminal_stop_request_json(
    request: TerminalStopRequest,
) -> dict[str, object]:
    return {
        "authorization": {
            "at_most_one_graceful_stop_request": True,
            "bound_completed_transaction_result": True,
            "bounded_host_only_quiescence_observation": True,
            (
                "no_status_start_guest_file_resume_dpkg_retry_repair_"
                "cleanup_force_kill_or_quit"
            ): True,
            "one_preflight_and_one_terminal_inventory": True,
            "terminal_stop_control": True,
        },
        "command_timeout_seconds": request.command_timeout_seconds,
        "expected_repository_head": request.expected_repository_head,
        "expected_vm_count": request.expected_vm_count,
        "format": EVIDENCE_FORMAT,
        "identity_observations": request.identity_observations,
        "prior_transaction_state_result_resolution_attempt_id": (
            request.prior_transaction_state_result_resolution_attempt_id
        ),
        "prior_transaction_state_result_resolution_manifest_sha256": (
            request.prior_transaction_state_result_resolution_manifest_sha256
        ),
        "stop_poll_attempts": request.stop_poll_attempts,
        "stop_poll_interval_seconds": request.stop_poll_interval_seconds,
        "stop_timeout_seconds": request.stop_timeout_seconds,
        "target_name": request.target_name,
        "target_package_path_sha256": network_ready._sha256_text(
            str(request.target_package_path)
        ),
        "target_uuid": request.target_uuid,
        "terminal_stop_attempt_id": request.terminal_stop_attempt_id,
    }


def validate_terminal_stop_bindings(
    request: TerminalStopRequest,
) -> TerminalStopBinding:
    control_path = request.repository_root / CONTROL_RELATIVE_PATH
    if Path(__file__).absolute() != control_path:
        raise TerminalStopError("executed-control-path-mismatch")
    try:
        network_ready._require_committed_regular(
            control_path, "terminal_stop_control"
        )
        upstream = completed_bindings.validate_transaction_state_result_resolution_result_bindings(
            request
        )
    except ValueError as exc:
        raise TerminalStopError(str(exc)) from exc
    if (
        upstream.evidence.get("transaction") != "completed"
        or upstream.evidence.get("package_profile") != "installed-verified"
        or upstream.evidence.get("startup_profile") != "allowed"
    ):
        raise TerminalStopError("completed-transaction-result-required")
    baseline = upstream.upstream.upstream.upstream.baseline_inventory
    if (
        len(baseline) != 20
        or any(item.status != "stopped" for item in baseline)
        or any(item.uuid == request.target_uuid for item in baseline)
    ):
        raise TerminalStopError("completed-result-baseline-inventory-invalid")
    return TerminalStopBinding(
        evidence={
            "completed_result_boot_id_sha256": upstream.boot_id_sha256,
            "completed_result_entries_verified": upstream.evidence[
                "prior_transaction_state_result_resolution_entries_verified"
            ],
            "completed_result_manifest_sha256": (
                request.prior_transaction_state_result_resolution_manifest_sha256
            ),
            "completed_result_receipt_sha256": upstream.receipt_sha256,
            "completed_result_transaction": "completed",
            "format": EVIDENCE_FORMAT,
            "prior_backend_pid": upstream.prior_backend_pid,
            "prior_handle_sha256": upstream.prior_handle_sha256,
            "prior_handle_size": upstream.prior_handle_size,
            "repository_clean": True,
            "repository_head": request.expected_repository_head,
            "target_uuid": request.target_uuid,
            "terminal_stop_attempt_id": request.terminal_stop_attempt_id,
            "terminal_stop_control_sha256": network_ready._sha256_file(
                control_path
            ),
            "terminal_stop_control_size": control_path.stat().st_size,
            "v7_baseline_vm_count": len(baseline),
        },
        upstream=upstream,
        baseline_inventory=baseline,
        prior_backend_pid=upstream.prior_backend_pid,
        prior_handle_sha256=upstream.prior_handle_sha256,
        prior_handle_size=upstream.prior_handle_size,
    )


def run_terminal_stop(
    request: TerminalStopRequest,
    *,
    runner: CommandRunner | None = None,
    binding_validator=None,
    source_opener=None,
    source_revalidator=None,
    target_validator=None,
    sleeper=time.sleep,
) -> TerminalStopResult:
    validate_terminal_stop_request(request)
    writer = start_control.EvidenceWriter.create(request.output_root)
    writer.write_json("request.json", terminal_stop_request_json(request))
    command_runner = runner or input_transfer.SubprocessCommandRunner()
    validate_bindings = binding_validator or validate_terminal_stop_bindings
    open_source = source_opener or input_transfer.open_source_bundle
    revalidate_source = (
        source_revalidator or input_transfer.revalidate_open_source_bundle
    )
    validate_target = target_validator or network_ready.validate_target_files
    workflow = transaction_control.TransactionStateProbeWorkflow()

    stage = "binding-preflight"
    reason = "not-run"
    outcome = "precondition-rejected"
    exit_code = EXIT_PRECONDITION_REJECTED
    binding: TerminalStopBinding | None = None
    source_bundle: input_transfer.SourceBundle | None = None
    target_before: dict[str, object] | None = None
    current_backend_pid: int | None = None
    current_handle_sha256: str | None = None
    current_handle_size: int | None = None
    registered_status = "not-observed"
    preflight_inventory_invocations = 0
    terminal_inventory_invocations = 0
    graceful_stop_invocations = 0
    identity_observation_count = 0
    quiescence_poll_count = 0
    target_handles_preflight = "not-observed"
    target_handles_terminal = "not-observed"
    terminal_relevant_processes: tuple[dict[str, object], ...] = ()
    terminal_excluded_processes: tuple[dict[str, object], ...] = ()
    stop_command_error: str | None = None

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
        process_observation, processes, excluded = _observe_processes(
            command_runner, request, workflow
        )
        terminal_relevant_processes = processes
        terminal_excluded_processes = excluded
        writer.write_json(
            stage + ".json",
            _process_evidence(process_observation, processes, excluded),
        )
        runtime_control._require_no_utmctl(processes, "preflight")

        stage = "target-handles-preflight"
        handles_observation, handles_state, handles = _observe_handles(
            command_runner, request
        )
        target_handles_preflight = handles_state
        writer.write_json(
            stage + ".json",
            _handles_evidence(handles_observation, handles_state, handles),
        )

        stage = "utmctl-list-preflight"
        preflight_inventory_invocations = 1
        list_observation = command_runner.run(
            ("utmctl", "list"), request.command_timeout_seconds
        )
        writer.write_json(stage + ".json", list_observation.as_json())
        inventory = start_control.parse_utmctl_list(list_observation)
        registered_status = reactivation_control._require_inventory(
            inventory, binding.baseline_inventory, request
        )
        writer.write_json(
            "inventory-preflight-classification.json",
            _inventory_evidence(request, inventory, registered_status),
        )

        if registered_status == "started":
            stage = "target-handle-pid-discovery"
            discovery = command_runner.run(
                network_ready._lsof_argv(request),
                request.command_timeout_seconds,
            )
            identity = runtime_control.parse_target_handle_process(
                discovery,
                request,
                expected_argv=network_ready._lsof_argv(request),
            )
            current_backend_pid = int(identity["backend_pid"])
            current_handle_sha256 = discovery.stdout.sha256
            current_handle_size = discovery.stdout.total_bytes
            writer.write_json(
                stage + ".json",
                runtime_control._handle_identity_evidence(
                    discovery, identity
                ),
            )

            for index in range(1, request.identity_observations + 1):
                stage = f"target-handle-pid-confirmation-{index:03d}"
                argv = runtime_control.targeted_lsof_argv(
                    request, current_backend_pid
                )
                observation = command_runner.run(
                    argv, request.command_timeout_seconds
                )
                identity = runtime_control.parse_target_handle_process(
                    observation, request, expected_argv=argv
                )
                identity_observation_count = index
                if (
                    int(identity["backend_pid"]) != current_backend_pid
                    or observation.stdout.sha256 != current_handle_sha256
                    or observation.stdout.total_bytes != current_handle_size
                ):
                    raise TerminalStopError("live-target-handle-identity-drift")
                writer.write_json(
                    stage + ".json",
                    runtime_control._handle_identity_evidence(
                        observation, identity
                    ),
                )
                process_observation, processes, excluded = _observe_processes(
                    command_runner, request, workflow
                )
                writer.write_json(
                    f"host-process-confirmation-{index:03d}.json",
                    _process_evidence(
                        process_observation, processes, excluded
                    ),
                )
                runtime_control._require_no_utmctl(
                    processes, f"confirmation-{index:03d}"
                )

            stage = "target-files-ready"
            target_ready = validate_target(request)
            writer.write_json(stage + ".json", target_ready)
            runtime_control._require_live_target_identity_stable(
                target_before, target_ready
            )

            stage = "utmctl-stop-request-once"
            graceful_stop_invocations = 1
            stop_observation = command_runner.run(
                ("utmctl", "stop", request.target_uuid, "--request"),
                request.stop_timeout_seconds,
            )
            writer.write_json(stage + ".json", stop_observation.as_json())
            try:
                start_control._require_successful_observation(
                    stop_observation, "utmctl-stop-request"
                )
            except start_control.StartControlError as exc:
                stop_command_error = str(exc)
        elif registered_status == "stopped":
            if handles_state != "absent":
                raise TerminalStopError(
                    "registered-stopped-with-target-handles-present"
                )
        else:
            raise TerminalStopError("target-registered-status-unresolved")

        stage = "target-quiescence-poll"
        target_handles_terminal = "not-observed"
        quiescence_observed = False
        for attempt in range(1, request.stop_poll_attempts + 1):
            quiescence_poll_count = attempt
            process_observation, processes, excluded = _observe_processes(
                command_runner, request, workflow
            )
            terminal_relevant_processes = processes
            terminal_excluded_processes = excluded
            writer.write_json(
                f"host-process-quiescence-{attempt:03d}.json",
                _process_evidence(process_observation, processes, excluded),
            )
            runtime_control._require_no_utmctl(
                processes, f"quiescence-{attempt:03d}"
            )
            handles_observation, handles_state, handles = _observe_handles(
                command_runner, request
            )
            target_handles_terminal = handles_state
            writer.write_json(
                f"target-handles-quiescence-{attempt:03d}.json",
                _handles_evidence(
                    handles_observation, handles_state, handles
                ),
            )
            if (
                handles_state == "absent"
                and launch_transport._backend_process_count(processes) == 0
            ):
                quiescence_observed = True
                break
            if attempt < request.stop_poll_attempts:
                sleeper(float(request.stop_poll_interval_seconds))
        if not quiescence_observed:
            raise TerminalStopError("target-runtime-not-quiescent")

        stage = "utmctl-list-terminal"
        terminal_inventory_invocations = 1
        terminal_list_observation = command_runner.run(
            ("utmctl", "list"), request.command_timeout_seconds
        )
        writer.write_json(
            stage + ".json", terminal_list_observation.as_json()
        )
        terminal_inventory = start_control.parse_utmctl_list(
            terminal_list_observation
        )
        terminal_status = reactivation_control._require_inventory(
            terminal_inventory, binding.baseline_inventory, request
        )
        writer.write_json(
            "inventory-terminal-classification.json",
            _inventory_evidence(request, terminal_inventory, terminal_status),
        )
        if terminal_status != "stopped":
            raise TerminalStopError("target-not-registered-stopped-terminal")

        stage = "host-process-terminal"
        process_observation, processes, excluded = _observe_processes(
            command_runner, request, workflow
        )
        terminal_relevant_processes = processes
        terminal_excluded_processes = excluded
        writer.write_json(
            stage + ".json",
            _process_evidence(process_observation, processes, excluded),
        )
        runtime_control._require_no_utmctl(processes, "terminal")
        if launch_transport._backend_process_count(processes) != 0:
            raise TerminalStopError("terminal-utm-backend-process-present")

        stage = "target-handles-terminal"
        handles_observation, handles_state, handles = _observe_handles(
            command_runner, request
        )
        target_handles_terminal = handles_state
        writer.write_json(
            stage + ".json",
            _handles_evidence(handles_observation, handles_state, handles),
        )
        if handles_state != "absent":
            raise TerminalStopError("target-handles-reappeared-terminal")

        stage = "target-files-postflight"
        target_after = validate_target(request)
        writer.write_json(stage + ".json", target_after)
        runtime_control._require_live_target_identity_stable(
            target_before, target_after
        )
        if stop_command_error is not None:
            raise TerminalStopError(
                f"graceful-stop-command-untrusted:{stop_command_error}"
            )

        if graceful_stop_invocations == 1:
            outcome = "stopped-verified"
            reason = "single-graceful-request-and-all-stopped-cross-check"
        else:
            outcome = "already-stopped-verified"
            reason = "all-stopped-without-stop-request"
        exit_code = EXIT_STOPPED_VERIFIED
    except (
        TerminalStopError,
        backend_control.BackendResolutionError,
        boot_control.BootStartResolutionError,
        input_transfer.CanonicalInputTransferError,
        launch_transport.LaunchTransportError,
        network_ready.NetworkReadyError,
        reactivation_control.ReactivationError,
        runtime_control.RuntimeResolutionError,
        start_control.StartControlError,
        OSError,
        tarfile.TarError,
        ValueError,
    ) as exc:
        reason = f"{stage}:{exc}"
        if preflight_inventory_invocations == 1:
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
                if preflight_inventory_invocations == 1:
                    outcome = "state-indeterminate"
                    exit_code = EXIT_STATE_INDETERMINATE
                else:
                    outcome = "precondition-rejected"
                    exit_code = EXIT_PRECONDITION_REJECTED
            source_bundle.file_object.close()

    writer.write_json(
        "terminal.json",
        {
            "automatic_cleanup": "not-performed",
            "automatic_force": "not-performed",
            "automatic_kill": "not-performed",
            "automatic_quit": "not-performed",
            "automatic_repair": "not-performed",
            "automatic_retry": "not-performed",
            "backend_pid_current": current_backend_pid,
            "backend_pid_prior": (
                binding.prior_backend_pid if binding is not None else None
            ),
            "business_guest_action": "not-performed",
            "current_handle_sha256": current_handle_sha256,
            "current_handle_size": current_handle_size,
            "file_pull_invocations": 0,
            "file_push_invocations": 0,
            "format": EVIDENCE_FORMAT,
            "graceful_stop_invocations": graceful_stop_invocations,
            "guest_exec_invocations": 0,
            "identity_observation_count": identity_observation_count,
            "maintenance_resume_invocations": 0,
            "operation_id": "not-read-or-generated",
            "outcome": outcome,
            "plain_utmctl_list_invocations": (
                preflight_inventory_invocations
                + terminal_inventory_invocations
            ),
            "plain_utmctl_start": "not-performed",
            "plain_utmctl_status": "not-performed",
            "preflight_inventory_invocations": (
                preflight_inventory_invocations
            ),
            "quiescence_poll_count": quiescence_poll_count,
            "reason": reason,
            "registered_status_preflight": registered_status,
            "stop_command_error": stop_command_error,
            "target_handles_preflight": target_handles_preflight,
            "target_handles_terminal": target_handles_terminal,
            "target_name": request.target_name,
            "target_uuid": request.target_uuid,
            "terminal_excluded_generic_qemu_process_count": len(
                terminal_excluded_processes
            ),
            "terminal_inventory_invocations": (
                terminal_inventory_invocations
            ),
            "terminal_relevant_host_process_count": len(
                terminal_relevant_processes
            ),
            "terminal_stop_attempt_id": request.terminal_stop_attempt_id,
            "transaction": "completed-frozen-before-stop",
        },
    )
    runtime_control._require_no_raw_operation_id(request.output_root)
    manifest_sha256 = writer.write_manifest()
    return TerminalStopResult(
        outcome=outcome,
        exit_code=exit_code,
        evidence_root=request.output_root,
        manifest_sha256=manifest_sha256,
        preflight_inventory_invocations=preflight_inventory_invocations,
        terminal_inventory_invocations=terminal_inventory_invocations,
        graceful_stop_invocations=graceful_stop_invocations,
        quiescence_poll_count=quiescence_poll_count,
    )


def _observe_processes(
    runner: CommandRunner,
    request: TerminalStopRequest,
    workflow: transaction_control.TransactionStateProbeWorkflow,
) -> tuple[
    start_control.CommandObservation,
    tuple[dict[str, object], ...],
    tuple[dict[str, object], ...],
]:
    return boot_control._observe_processes(runner, request, workflow)


def _observe_handles(
    runner: CommandRunner,
    request: TerminalStopRequest,
) -> tuple[start_control.CommandObservation, str, dict[str, object]]:
    observation = runner.run(
        network_ready._lsof_argv(request), request.command_timeout_seconds
    )
    try:
        state, handles = backend_control.classify_target_handles(
            observation, request
        )
    except backend_control.BackendResolutionError as exc:
        raise TerminalStopError(str(exc)) from exc
    return observation, state, handles


def _process_evidence(
    observation: start_control.CommandObservation,
    processes: tuple[dict[str, object], ...],
    excluded: tuple[dict[str, object], ...],
) -> dict[str, object]:
    return {
        "excluded_generic_qemu_process_count": len(excluded),
        "excluded_generic_qemu_processes": list(excluded),
        "format": EVIDENCE_FORMAT,
        "observation": network_ready._observation_metadata(observation),
        "process_scope": "utm-specific-accounting-v1",
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


def _inventory_evidence(
    request: TerminalStopRequest,
    inventory: tuple[start_control.RegisteredVm, ...],
    target_status: str,
) -> dict[str, object]:
    return {
        "format": EVIDENCE_FORMAT,
        "other_registered_vm_count": len(inventory) - 1,
        "other_registered_vms": "all-stopped",
        "registered_vm_count": len(inventory),
        "target_registered_status": target_status,
        "target_uuid": request.target_uuid,
    }
