#!/usr/bin/env python3
from __future__ import annotations

import argparse
import sys
import tarfile
from dataclasses import dataclass
from pathlib import Path
from typing import Protocol

import l6_utm_canonical_input_transfer as input_transfer
import l6_utm_guest_network_ready as network_ready
import l6_utm_install_artifacts_staged_boot_start_resolution as boot_start_control
import l6_utm_install_artifacts_staged_fresh_boot_classification as classification_control
import l6_utm_install_artifacts_staged_fresh_boot_recovery_preflight as prior_control
import l6_utm_install_artifacts_staged_fresh_boot_recovery_preflight_result_bindings as result_bindings
import l6_utm_install_artifacts_staged_new_boot_recovery_preflight as recovery_control
import l6_utm_install_artifacts_staged_resume_evidence as resume_evidence
import l6_utm_install_artifacts_staged_runtime_resolution as runtime_control
import l6_utm_start_once as start_control
import l6_v4_canonical_input_install as guest_installer
import l6_v4_install_artifacts_staged_new_boot_recovery_preflight as guest_probe


EVIDENCE_FORMAT = (
    "radishlex-linux-l6-utm-install-artifacts-staged-"
    "fresh-boot-recovery-result-resolution-v1"
)
CONTROL_RELATIVE_PATH = Path(
    "scripts/linux-product/"
    "l6_utm_install_artifacts_staged_fresh_boot_recovery_result_resolution.py"
)
RESULT_BINDINGS_RELATIVE_PATH = result_bindings.BINDINGS_RELATIVE_PATH
REQUIRED_ATTEMPT_ID = (
    "d75818f-v4-install-artifacts-staged-fresh-boot-recovery-"
    "result-resolution-20260827-v1"
)
REQUIRED_PRIOR_ATTEMPT_ID = result_bindings.REQUIRED_PRIOR_ATTEMPT_ID
REQUIRED_PRIOR_MANIFEST_SHA256 = (
    result_bindings.REQUIRED_PRIOR_MANIFEST_SHA256
)
EXIT_RECOVERY_QUALIFIED = recovery_control.EXIT_RECOVERY_QUALIFIED
EXIT_RECOVERY_REJECTED = recovery_control.EXIT_RECOVERY_REJECTED
EXIT_PRECONDITION_REJECTED = recovery_control.EXIT_PRECONDITION_REJECTED
EXIT_STATE_INDETERMINATE = recovery_control.EXIT_STATE_INDETERMINATE


class FreshBootRecoveryResultResolutionError(ValueError):
    pass


@dataclass(frozen=True)
class FreshBootRecoveryResultResolutionRequest(
    prior_control.FreshBootRecoveryPreflightRequest
):
    prior_fresh_boot_recovery_preflight_root: Path
    prior_fresh_boot_recovery_preflight_manifest_sha256: str
    prior_fresh_boot_recovery_preflight_attempt_id: str
    result_resolution_attempt_id: str
    authorized_install_artifacts_staged_fresh_boot_recovery_result_resolution: bool
    authorized_bound_prior_fresh_boot_recovery_preflight_result: bool
    authorized_two_existing_recovery_result_readbacks: bool
    authorized_one_existing_recovery_phase_readback: bool
    authorized_no_inventory_start_status_probe_push_exec_resume_dpkg_retry_stop_or_quit: (
        bool
    )

    def validate(self) -> None:
        prior_control.FreshBootRecoveryPreflightRequest.validate(self)
        root = self.prior_fresh_boot_recovery_preflight_root
        if not root.is_absolute() or ".." in root.parts:
            raise FreshBootRecoveryResultResolutionError(
                "prior-fresh-boot-recovery-preflight-root-must-be-absolute-normalized"
            )
        if runtime_control._paths_overlap(self.output_root, root):
            raise FreshBootRecoveryResultResolutionError(
                "output-root-must-not-overlap-prior-fresh-boot-recovery-preflight-root"
            )
        if (
            self.prior_fresh_boot_recovery_preflight_manifest_sha256
            != REQUIRED_PRIOR_MANIFEST_SHA256
            or self.prior_fresh_boot_recovery_preflight_attempt_id
            != REQUIRED_PRIOR_ATTEMPT_ID
            or self.result_resolution_attempt_id != REQUIRED_ATTEMPT_ID
        ):
            raise FreshBootRecoveryResultResolutionError(
                "fixed-fresh-boot-recovery-result-resolution-input-mismatch"
            )
        for authorized, reason in (
            (
                self.authorized_install_artifacts_staged_fresh_boot_recovery_result_resolution,
                "fresh-boot-recovery-result-resolution-authorization-required",
            ),
            (
                self.authorized_bound_prior_fresh_boot_recovery_preflight_result,
                "prior-fresh-boot-recovery-result-binding-authorization-required",
            ),
            (
                self.authorized_two_existing_recovery_result_readbacks,
                "two-existing-recovery-result-readbacks-authorization-required",
            ),
            (
                self.authorized_one_existing_recovery_phase_readback,
                "one-existing-recovery-phase-readback-authorization-required",
            ),
            (
                self.authorized_no_inventory_start_status_probe_push_exec_resume_dpkg_retry_stop_or_quit,
                "fresh-boot-recovery-result-resolution-forbidden-action-boundary-authorization-required",
            ),
        ):
            if not authorized:
                raise FreshBootRecoveryResultResolutionError(reason)

    def as_json(self) -> dict[str, object]:
        return {
            "authorization": {
                "bound_prior_fresh_boot_recovery_preflight_result": True,
                "install_artifacts_staged_fresh_boot_recovery_result_resolution": True,
                (
                    "no_inventory_start_status_probe_push_exec_resume_dpkg_"
                    "retry_stop_or_quit"
                ): True,
                "one_existing_recovery_phase_readback": True,
                "two_existing_recovery_result_readbacks": True,
            },
            "command_timeout_seconds": self.command_timeout_seconds,
            "expected_repository_head": self.expected_repository_head,
            "format": EVIDENCE_FORMAT,
            "guest_recovery_phase_path": self.guest_phase_path,
            "guest_recovery_result_path": self.guest_terminal_path,
            "identity_observations": self.identity_observations,
            "prior_fresh_boot_classification_attempt_id": (
                self.prior_fresh_boot_classification_attempt_id
            ),
            "prior_fresh_boot_classification_manifest_sha256": (
                self.prior_fresh_boot_classification_manifest_sha256
            ),
            "prior_fresh_boot_recovery_preflight_attempt_id": (
                self.prior_fresh_boot_recovery_preflight_attempt_id
            ),
            "prior_fresh_boot_recovery_preflight_manifest_sha256": (
                self.prior_fresh_boot_recovery_preflight_manifest_sha256
            ),
            "result_resolution_attempt_id": self.result_resolution_attempt_id,
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
class FreshBootRecoveryResultResolutionBinding:
    evidence: dict[str, object]
    upstream: result_bindings.FreshBootRecoveryPreflightResultBinding
    current_boot_id_sha256: str
    prior_boot_id_sha256: str
    prior_backend_pid: int


@dataclass(frozen=True)
class FreshBootRecoveryResultResolutionResult:
    outcome: str
    exit_code: int
    evidence_root: Path
    manifest_sha256: str
    result_readback_invocations: int
    phase_readback_invocations: int


class CommandRunner(Protocol):
    def run(
        self,
        argv: tuple[str, ...],
        timeout_seconds: int,
        *,
        stdin_file=None,
    ) -> start_control.CommandObservation: ...


def validate_result_resolution_bindings(
    request: FreshBootRecoveryResultResolutionRequest,
) -> FreshBootRecoveryResultResolutionBinding:
    for actual, relative, label in (
        (Path(__file__).absolute(), CONTROL_RELATIVE_PATH, "control"),
        (
            Path(result_bindings.__file__).absolute(),
            RESULT_BINDINGS_RELATIVE_PATH,
            "prior-result-bindings",
        ),
    ):
        if actual != request.repository_root / relative:
            raise FreshBootRecoveryResultResolutionError(
                f"executed-{label}-path-mismatch"
            )
        network_ready._require_committed_regular(actual, label)
    try:
        upstream = result_bindings.validate_fresh_boot_recovery_preflight_result_bindings(
            request
        )
    except ValueError as exc:
        raise FreshBootRecoveryResultResolutionError(str(exc)) from exc
    identities: dict[str, object] = {}
    for relative, label in (
        (CONTROL_RELATIVE_PATH, "fresh_boot_recovery_result_resolution_control"),
        (
            RESULT_BINDINGS_RELATIVE_PATH,
            "fresh_boot_recovery_preflight_result_bindings",
        ),
    ):
        path = request.repository_root / relative
        identities[f"{label}_sha256"] = network_ready._sha256_file(path)
        identities[f"{label}_size"] = path.stat().st_size
    return FreshBootRecoveryResultResolutionBinding(
        evidence={
            "current_boot_id_sha256": upstream.current_boot_id_sha256,
            "format": EVIDENCE_FORMAT,
            **identities,
            "prior_boot_id_sha256": upstream.prior_boot_id_sha256,
            "prior_fresh_boot_recovery_preflight_attempt_id": (
                request.prior_fresh_boot_recovery_preflight_attempt_id
            ),
            "prior_fresh_boot_recovery_preflight_backend_pid": (
                upstream.prior_backend_pid
            ),
            "prior_fresh_boot_recovery_preflight_entries_verified": (
                upstream.evidence[
                    "prior_fresh_boot_recovery_preflight_entries_verified"
                ]
            ),
            "prior_fresh_boot_recovery_preflight_manifest_sha256": (
                request.prior_fresh_boot_recovery_preflight_manifest_sha256
            ),
            "prior_fresh_boot_recovery_preflight_outcome": (
                "state-indeterminate"
            ),
            "repository_clean": True,
            "repository_head": request.expected_repository_head,
            "result_resolution_attempt_id": request.result_resolution_attempt_id,
            "target_uuid": request.target_uuid,
        },
        upstream=upstream,
        current_boot_id_sha256=upstream.current_boot_id_sha256,
        prior_boot_id_sha256=upstream.prior_boot_id_sha256,
        prior_backend_pid=upstream.prior_backend_pid,
    )


def run_fresh_boot_recovery_result_resolution(
    request: FreshBootRecoveryResultResolutionRequest,
    *,
    runner: CommandRunner | None = None,
    binding_validator=None,
    source_opener=None,
    source_revalidator=None,
    target_validator=None,
) -> FreshBootRecoveryResultResolutionResult:
    request.validate()
    writer = start_control.EvidenceWriter.create(request.output_root)
    writer.write_json("request.json", request.as_json())
    command_runner = runner or input_transfer.SubprocessCommandRunner()
    validate_bindings = binding_validator or validate_result_resolution_bindings
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
    backend_pid: int | None = None
    terminal_processes: tuple[dict[str, object], ...] = ()
    identity_observation_count = 0
    result_readback_invocations = 0
    phase_readback_invocations = 0
    file_pull_invocations = 0
    resolution_started = False
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
                raise FreshBootRecoveryResultResolutionError(f"{name}-empty")
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
            runtime_control._process_evidence(
                process_observation, terminal_processes
            ),
        )
        runtime_control._require_no_utmctl(terminal_processes, "preflight")

        stage = "target-handle-pid-discovery"
        observation = command_runner.run(
            network_ready._lsof_argv(request), request.command_timeout_seconds
        )
        identity = runtime_control.parse_target_handle_process(
            observation,
            request,
            expected_argv=network_ready._lsof_argv(request),
        )
        backend_pid = int(identity["backend_pid"])
        if backend_pid != binding.prior_backend_pid:
            raise FreshBootRecoveryResultResolutionError(
                "prior-recovery-preflight-backend-pid-drift"
            )
        writer.write_json(
            stage + ".json",
            runtime_control._handle_identity_evidence(observation, identity),
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
                raise FreshBootRecoveryResultResolutionError(
                    "target-backend-pid-drift"
                )
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

        resolution_started = True
        payloads = []
        for index in (1, 2):
            result_readback_invocations = index
            payloads.append(
                pull(
                    request.guest_terminal_path,
                    f"existing-recovery-result-readback-{index}",
                )
            )
        payload = resume_evidence.require_equal_payloads(
            payloads, "existing-recovery-result"
        )
        guest_terminal = recovery_control.parse_guest_terminal(
            payload, request, binding
        )
        writer.write_json("existing-recovery-result.json", guest_terminal)

        phase_readback_invocations = 1
        pull(
            request.guest_phase_path,
            "existing-recovery-phase-readback",
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
            raise FreshBootRecoveryResultResolutionError(
                "target-backend-pid-terminal-drift"
            )

        stage = "host-process-terminal"
        process_observation, terminal_processes = runtime_control._observe_processes(
            command_runner, request
        )
        writer.write_json(
            stage + ".json",
            runtime_control._process_evidence(
                process_observation, terminal_processes
            ),
        )
        runtime_control._require_no_utmctl(terminal_processes, "terminal")

        stage = "target-files-postflight"
        target_after = validate_target(request)
        writer.write_json(stage + ".json", target_after)
        runtime_control._require_live_target_identity_stable(
            target_before, target_after
        )

        if guest_terminal["outcome"] == "recovery-qualified":
            outcome = "recovery-qualified"
            exit_code = EXIT_RECOVERY_QUALIFIED
            reason = "deferred-readback-recovery-qualified"
        else:
            outcome = "recovery-rejected"
            exit_code = EXIT_RECOVERY_REJECTED
            reason = f"deferred-readback-recovery-rejected:{guest_terminal['reason']}"
    except (
        FreshBootRecoveryResultResolutionError,
        boot_start_control.BootStartResolutionError,
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
        if resolution_started:
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
                if resolution_started:
                    outcome = "state-indeterminate"
                    exit_code = EXIT_STATE_INDETERMINATE
            source_bundle.file_object.close()

    writer.write_json(
        "terminal.json",
        {
            "automatic_cleanup": "not-performed",
            "automatic_quit": "not-performed",
            "automatic_retry": "not-performed",
            "automatic_stop": "not-performed",
            "backend_pid": backend_pid,
            "business_guest_action": "not-performed",
            "file_pull_invocations": file_pull_invocations,
            "file_push_invocations": 0,
            "format": EVIDENCE_FORMAT,
            "guest_exec_invocations": 0,
            "guest_probe_invocations": 0,
            "guest_recovery_outcome": (
                guest_terminal.get("outcome")
                if guest_terminal is not None
                else "not-observed"
            ),
            "identity_observation_count": identity_observation_count,
            "inventory_probe_invocations": 0,
            "maintenance_resume_invocations": 0,
            "operation_id": "existing-receipt-hash-only",
            "outcome": outcome,
            "phase_readback_invocations": phase_readback_invocations,
            "plain_utmctl_list": "not-performed",
            "plain_utmctl_start": "not-performed",
            "plain_utmctl_status": "not-performed",
            "prior_fresh_boot_recovery_preflight_attempt_id": (
                request.prior_fresh_boot_recovery_preflight_attempt_id
            ),
            "reason": reason,
            "result_readback_invocations": result_readback_invocations,
            "result_resolution_attempt_id": request.result_resolution_attempt_id,
            "target_name": request.target_name,
            "target_uuid": request.target_uuid,
            "terminal_relevant_host_process_count": len(terminal_processes),
            "transaction": "artifacts-staged-preserved-no-resume",
        },
    )
    runtime_control._require_no_raw_operation_id(request.output_root)
    manifest_sha256 = writer.write_manifest()
    return FreshBootRecoveryResultResolutionResult(
        outcome=outcome,
        exit_code=exit_code,
        evidence_root=request.output_root,
        manifest_sha256=manifest_sha256,
        result_readback_invocations=result_readback_invocations,
        phase_readback_invocations=phase_readback_invocations,
    )


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Consume one fully validated state-indeterminate fresh-boot recovery "
            "preflight and resolve only its already-created guest result through "
            "two readbacks plus one phase readback."
        )
    )
    parser.add_argument("command", choices=("resolve-existing-recovery-result",))
    classification_control._add_chain_arguments(parser)
    parser.add_argument(
        "--prior-fresh-boot-classification-root", type=Path, required=True
    )
    parser.add_argument(
        "--prior-fresh-boot-classification-manifest-sha256", required=True
    )
    parser.add_argument(
        "--prior-fresh-boot-classification-attempt-id", required=True
    )
    parser.add_argument(
        "--prior-fresh-boot-recovery-preflight-root", type=Path, required=True
    )
    parser.add_argument(
        "--prior-fresh-boot-recovery-preflight-manifest-sha256", required=True
    )
    parser.add_argument(
        "--prior-fresh-boot-recovery-preflight-attempt-id", required=True
    )
    parser.add_argument("--result-resolution-attempt-id", required=True)
    for name in (
        "authorized-install-artifacts-staged-fresh-boot-recovery-result-resolution",
        "authorized-bound-prior-fresh-boot-recovery-preflight-result",
        "authorized-two-existing-recovery-result-readbacks",
        "authorized-one-existing-recovery-phase-readback",
        "authorized-no-inventory-start-status-probe-push-exec-resume-dpkg-retry-stop-or-quit",
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
        }
    )
    request = FreshBootRecoveryResultResolutionRequest(**values)
    try:
        result = run_fresh_boot_recovery_result_resolution(request)
    except (
        FreshBootRecoveryResultResolutionError,
        recovery_control.RecoveryPreflightError,
        boot_start_control.BootStartResolutionError,
    ) as exc:
        print(
            "fresh boot recovery result resolution rejected before evidence "
            f"creation: {exc}",
            file=sys.stderr,
        )
        return EXIT_PRECONDITION_REJECTED
    print(f"outcome={result.outcome}")
    print(f"evidence_root={result.evidence_root}")
    print(f"manifest_sha256={result.manifest_sha256}")
    print(f"result_readback_invocations={result.result_readback_invocations}")
    print(f"phase_readback_invocations={result.phase_readback_invocations}")
    return result.exit_code


if __name__ == "__main__":
    raise SystemExit(main())
