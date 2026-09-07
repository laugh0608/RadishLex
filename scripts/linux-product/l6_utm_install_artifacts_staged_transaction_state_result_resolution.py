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
import l6_utm_install_artifacts_staged_boot_start_resolution as boot_control
import l6_utm_install_artifacts_staged_fresh_boot_classification as classification_control
import l6_utm_install_artifacts_staged_runtime_resolution as runtime_control
import l6_utm_install_artifacts_staged_transaction_state_resolution as prior_control
import l6_utm_install_artifacts_staged_transaction_state_resolution_result_bindings as result_bindings
import l6_utm_start_once as start_control
import l6_v4_canonical_input_install as guest_installer


EVIDENCE_FORMAT = (
    "radishlex-linux-l6-utm-install-artifacts-staged-"
    "transaction-state-result-resolution-v1"
)
CONTROL_RELATIVE_PATH = Path(
    "scripts/linux-product/"
    "l6_utm_install_artifacts_staged_transaction_state_result_resolution.py"
)
RESULT_BINDINGS_RELATIVE_PATH = result_bindings.BINDINGS_RELATIVE_PATH
REQUIRED_ATTEMPT_ID = (
    "d75818f-v4-install-artifacts-staged-transaction-state-result-"
    "resolution-20260829-v1"
)
REQUIRED_PRIOR_ATTEMPT_ID = result_bindings.REQUIRED_PRIOR_ATTEMPT_ID
REQUIRED_PRIOR_MANIFEST_SHA256 = (
    result_bindings.REQUIRED_PRIOR_MANIFEST_SHA256
)
EXIT_TRANSACTION_COMPLETED = prior_control.EXIT_TRANSACTION_COMPLETED
EXIT_ARTIFACTS_STAGED = prior_control.EXIT_ARTIFACTS_STAGED
EXIT_PRECONDITION_REJECTED = prior_control.EXIT_PRECONDITION_REJECTED
EXIT_STATE_INDETERMINATE = prior_control.EXIT_STATE_INDETERMINATE


class TransactionStateResultResolutionError(ValueError):
    pass


@dataclass(frozen=True)
class TransactionStateResultResolutionRequest(
    prior_control.TransactionStateResolutionRequest
):
    prior_transaction_state_resolution_root: Path
    prior_transaction_state_resolution_manifest_sha256: str
    prior_transaction_state_resolution_attempt_id: str
    transaction_state_result_resolution_attempt_id: str
    authorized_transaction_state_result_resolution: bool
    authorized_bound_prior_transaction_state_resolution_result: bool
    authorized_two_existing_transaction_state_result_readbacks: bool
    authorized_one_existing_transaction_state_phase_readback: bool
    authorized_no_inventory_list_status_start_push_exec_probe_resume_dpkg_retry_repair_cleanup_or_automatic_stop: (
        bool
    )

    @property
    def guest_transaction_phase_path(self) -> str:
        return f"{self.guest_control_root}/phase.json"

    def validate(self) -> None:
        prior_control.TransactionStateResolutionRequest.validate(self)
        root = self.prior_transaction_state_resolution_root
        if not root.is_absolute() or ".." in root.parts:
            raise TransactionStateResultResolutionError(
                "prior-transaction-state-resolution-root-must-be-absolute-normalized"
            )
        if runtime_control._paths_overlap(self.output_root, root):
            raise TransactionStateResultResolutionError(
                "output-root-must-not-overlap-prior-transaction-state-resolution-root"
            )
        if (
            self.prior_transaction_state_resolution_manifest_sha256
            != REQUIRED_PRIOR_MANIFEST_SHA256
            or self.prior_transaction_state_resolution_attempt_id
            != REQUIRED_PRIOR_ATTEMPT_ID
            or self.transaction_state_result_resolution_attempt_id
            != REQUIRED_ATTEMPT_ID
        ):
            raise TransactionStateResultResolutionError(
                "fixed-transaction-state-result-resolution-input-mismatch"
            )
        for authorized, reason in (
            (
                self.authorized_transaction_state_result_resolution,
                "transaction-state-result-resolution-authorization-required",
            ),
            (
                self.authorized_bound_prior_transaction_state_resolution_result,
                "prior-transaction-state-result-binding-authorization-required",
            ),
            (
                self.authorized_two_existing_transaction_state_result_readbacks,
                "two-existing-transaction-state-result-readbacks-authorization-required",
            ),
            (
                self.authorized_one_existing_transaction_state_phase_readback,
                "one-existing-transaction-state-phase-readback-authorization-required",
            ),
            (
                self.authorized_no_inventory_list_status_start_push_exec_probe_resume_dpkg_retry_repair_cleanup_or_automatic_stop,
                "transaction-state-result-resolution-forbidden-action-boundary-authorization-required",
            ),
        ):
            if not authorized:
                raise TransactionStateResultResolutionError(reason)

    def as_json(self) -> dict[str, object]:
        value = prior_control.TransactionStateResolutionRequest.as_json(self)
        value["authorization"] = {
            "bound_prior_transaction_state_resolution_result": True,
            (
                "no_inventory_list_status_start_push_exec_probe_resume_dpkg_"
                "retry_repair_cleanup_or_automatic_stop"
            ): True,
            "one_existing_transaction_state_phase_readback": True,
            "transaction_state_result_resolution": True,
            "two_existing_transaction_state_result_readbacks": True,
        }
        value.update(
            {
                "format": EVIDENCE_FORMAT,
                "guest_transaction_state_phase_path": (
                    self.guest_transaction_phase_path
                ),
                "guest_transaction_state_result_path": self.guest_result_path,
                "prior_transaction_state_resolution_attempt_id": (
                    self.prior_transaction_state_resolution_attempt_id
                ),
                "prior_transaction_state_resolution_manifest_sha256": (
                    self.prior_transaction_state_resolution_manifest_sha256
                ),
                "transaction_state_result_resolution_attempt_id": (
                    self.transaction_state_result_resolution_attempt_id
                ),
            }
        )
        return value


@dataclass(frozen=True)
class TransactionStateResultResolutionBinding:
    evidence: dict[str, object]
    upstream: result_bindings.TransactionStateResolutionResultBinding
    prior_backend_pid: int
    prior_handle_sha256: str
    prior_handle_size: int
    probe_sha256: str


@dataclass(frozen=True)
class TransactionStateResultResolutionResult:
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


def validate_transaction_state_result_resolution_bindings(
    request: TransactionStateResultResolutionRequest,
) -> TransactionStateResultResolutionBinding:
    for actual, relative, label in (
        (Path(__file__).absolute(), CONTROL_RELATIVE_PATH, "control"),
        (
            Path(result_bindings.__file__).absolute(),
            RESULT_BINDINGS_RELATIVE_PATH,
            "prior-result-bindings",
        ),
    ):
        if actual != request.repository_root / relative:
            raise TransactionStateResultResolutionError(
                f"executed-{label}-path-mismatch"
            )
        network_ready._require_committed_regular(actual, label)
    try:
        upstream = (
            result_bindings.validate_transaction_state_resolution_result_bindings(
                request
            )
        )
    except ValueError as exc:
        raise TransactionStateResultResolutionError(str(exc)) from exc
    identities: dict[str, object] = {}
    for relative, label in (
        (CONTROL_RELATIVE_PATH, "transaction_state_result_resolution_control"),
        (
            RESULT_BINDINGS_RELATIVE_PATH,
            "transaction_state_resolution_result_bindings",
        ),
    ):
        path = request.repository_root / relative
        identities[f"{label}_sha256"] = network_ready._sha256_file(path)
        identities[f"{label}_size"] = path.stat().st_size
    return TransactionStateResultResolutionBinding(
        evidence={
            "format": EVIDENCE_FORMAT,
            **identities,
            "prior_transaction_state_resolution_attempt_id": (
                request.prior_transaction_state_resolution_attempt_id
            ),
            "prior_transaction_state_resolution_backend_pid": (
                upstream.prior_backend_pid
            ),
            "prior_transaction_state_resolution_entries_verified": (
                upstream.evidence[
                    "prior_transaction_state_resolution_entries_verified"
                ]
            ),
            "prior_transaction_state_resolution_handle_sha256": (
                upstream.prior_handle_sha256
            ),
            "prior_transaction_state_resolution_handle_size": (
                upstream.prior_handle_size
            ),
            "prior_transaction_state_resolution_manifest_sha256": (
                request.prior_transaction_state_resolution_manifest_sha256
            ),
            "prior_transaction_state_resolution_outcome": "state-indeterminate",
            "repository_clean": True,
            "repository_head": request.expected_repository_head,
            "target_uuid": request.target_uuid,
            "transaction_state_probe_sha256": upstream.probe_sha256,
            "transaction_state_result_resolution_attempt_id": (
                request.transaction_state_result_resolution_attempt_id
            ),
        },
        upstream=upstream,
        prior_backend_pid=upstream.prior_backend_pid,
        prior_handle_sha256=upstream.prior_handle_sha256,
        prior_handle_size=upstream.prior_handle_size,
        probe_sha256=upstream.probe_sha256,
    )


def run_transaction_state_result_resolution(
    request: TransactionStateResultResolutionRequest,
    *,
    runner: CommandRunner | None = None,
    binding_validator=None,
    source_opener=None,
    source_revalidator=None,
    target_validator=None,
) -> TransactionStateResultResolutionResult:
    request.validate()
    writer = start_control.EvidenceWriter.create(request.output_root)
    writer.write_json("request.json", request.as_json())
    command_runner = runner or input_transfer.SubprocessCommandRunner()
    validate_bindings = (
        binding_validator or validate_transaction_state_result_resolution_bindings
    )
    open_source = source_opener or input_transfer.open_source_bundle
    revalidate_source = (
        source_revalidator or input_transfer.revalidate_open_source_bundle
    )
    validate_target = target_validator or network_ready.validate_target_files
    workflow = prior_control.TransactionStateProbeWorkflow()

    stage = "binding-preflight"
    reason = "not-run"
    outcome = "precondition-rejected"
    exit_code = EXIT_PRECONDITION_REJECTED
    transaction = "state-indeterminate"
    source_bundle: input_transfer.SourceBundle | None = None
    target_before: dict[str, object] | None = None
    backend_pid: int | None = None
    terminal_processes: tuple[dict[str, object], ...] = ()
    terminal_excluded_processes: tuple[dict[str, object], ...] = ()
    identity_observation_count = 0
    result_readback_invocations = 0
    phase_readback_invocations = 0
    file_pull_invocations = 0
    resolution_started = False
    guest_resolution: boot_control.GuestProbeResolution | None = None

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
                raise TransactionStateResultResolutionError(f"{name}-empty")
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
        (
            process_observation,
            terminal_processes,
            terminal_excluded_processes,
        ) = boot_control._observe_processes(
            command_runner, request, workflow
        )
        writer.write_json(
            stage + ".json",
            boot_control._process_evidence(
                process_observation,
                terminal_processes,
                terminal_excluded_processes,
                workflow,
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
        _require_prior_handle_identity(observation, binding, backend_pid)
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
            _require_prior_handle_identity(observation, binding, int(identity["backend_pid"]))
            writer.write_json(
                stage + ".json",
                runtime_control._handle_identity_evidence(observation, identity),
            )
            (
                process_observation,
                terminal_processes,
                terminal_excluded_processes,
            ) = boot_control._observe_processes(
                command_runner, request, workflow
            )
            writer.write_json(
                f"host-process-confirmation-{index:03d}.json",
                boot_control._process_evidence(
                    process_observation,
                    terminal_processes,
                    terminal_excluded_processes,
                    workflow,
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
                    request.guest_result_path,
                    f"existing-transaction-state-result-readback-{index}",
                )
            )
        if payloads[0] != payloads[1]:
            raise TransactionStateResultResolutionError(
                "existing-transaction-state-result-readback-drift"
            )
        guest_resolution = prior_control.parse_transaction_state_result(
            payloads[0], request, binding.probe_sha256
        )
        writer.write_json(
            "existing-transaction-state-result.json",
            {
                "evidence": guest_resolution.evidence,
                "format": EVIDENCE_FORMAT,
                "outcome": guest_resolution.outcome,
                "reason": guest_resolution.reason,
                "transaction": guest_resolution.transaction,
            },
        )

        phase_readback_invocations = 1
        pull(
            request.guest_transaction_phase_path,
            "existing-transaction-state-phase-readback",
            exact=prior_control.guest_probe.canonical_json(
                {
                    "format": prior_control.guest_probe.PHASE_FORMAT,
                    "phase": guest_resolution.transaction,
                }
            ),
        )

        stage = "target-handle-pid-terminal"
        argv = runtime_control.targeted_lsof_argv(request, backend_pid)
        observation = command_runner.run(argv, request.command_timeout_seconds)
        identity = runtime_control.parse_target_handle_process(
            observation, request, expected_argv=argv
        )
        _require_prior_handle_identity(observation, binding, int(identity["backend_pid"]))
        writer.write_json(
            stage + ".json",
            runtime_control._handle_identity_evidence(observation, identity),
        )

        stage = "host-process-terminal"
        (
            process_observation,
            terminal_processes,
            terminal_excluded_processes,
        ) = boot_control._observe_processes(
            command_runner, request, workflow
        )
        writer.write_json(
            stage + ".json",
            boot_control._process_evidence(
                process_observation,
                terminal_processes,
                terminal_excluded_processes,
                workflow,
            ),
        )
        runtime_control._require_no_utmctl(terminal_processes, "terminal")

        stage = "target-files-postflight"
        target_after = validate_target(request)
        writer.write_json(stage + ".json", target_after)
        runtime_control._require_live_target_identity_stable(
            target_before, target_after
        )

        outcome = guest_resolution.outcome
        exit_code = guest_resolution.exit_code
        transaction = guest_resolution.transaction
        reason = f"deferred-readback-{guest_resolution.outcome}"
    except (
        TransactionStateResultResolutionError,
        boot_control.BootStartResolutionError,
        input_transfer.CanonicalInputTransferError,
        network_ready.NetworkReadyError,
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
            "business_guest_action": (
                "existing-result-readback-only"
                if guest_resolution is not None
                else "not-performed"
            ),
            "file_pull_invocations": file_pull_invocations,
            "file_push_invocations": 0,
            "format": EVIDENCE_FORMAT,
            "guest_exec_invocations": 0,
            "guest_probe_invocations": 0,
            "identity_observation_count": identity_observation_count,
            "inventory_probe_invocations": 0,
            "maintenance_resume_invocations": 0,
            "operation_id": (
                "hash-only"
                if guest_resolution is not None
                else "not-read-or-generated"
            ),
            "outcome": outcome,
            "phase_readback_invocations": phase_readback_invocations,
            "plain_utmctl_list": "not-performed",
            "plain_utmctl_start": "not-performed",
            "plain_utmctl_status": "not-performed",
            "prior_transaction_state_resolution_attempt_id": (
                request.prior_transaction_state_resolution_attempt_id
            ),
            "reason": reason,
            "result_readback_invocations": result_readback_invocations,
            "target_name": request.target_name,
            "target_uuid": request.target_uuid,
            "terminal_excluded_generic_qemu_process_count": len(
                terminal_excluded_processes
            ),
            "terminal_relevant_host_process_count": len(terminal_processes),
            "transaction": transaction,
            "transaction_state_result_resolution_attempt_id": (
                request.transaction_state_result_resolution_attempt_id
            ),
        },
    )
    runtime_control._require_no_raw_operation_id(request.output_root)
    manifest_sha256 = writer.write_manifest()
    return TransactionStateResultResolutionResult(
        outcome=outcome,
        exit_code=exit_code,
        evidence_root=request.output_root,
        manifest_sha256=manifest_sha256,
        result_readback_invocations=result_readback_invocations,
        phase_readback_invocations=phase_readback_invocations,
    )


def _require_prior_handle_identity(
    observation: start_control.CommandObservation,
    binding: TransactionStateResultResolutionBinding,
    backend_pid: int,
) -> None:
    if (
        backend_pid != binding.prior_backend_pid
        or observation.stdout.sha256 != binding.prior_handle_sha256
        or observation.stdout.total_bytes != binding.prior_handle_size
    ):
        raise TransactionStateResultResolutionError(
            "prior-transaction-state-live-handle-drift"
        )


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Resolve only the already-created transaction-state result through "
            "two stable readbacks plus one phase readback, without inventory, "
            "start, push, exec, probe, transaction mutation, or automatic stop."
        )
    )
    parser.add_argument(
        "command", choices=("resolve-existing-transaction-state-result",)
    )
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
    parser.add_argument(
        "--prior-fresh-boot-recovery-result-resolution-root",
        type=Path,
        required=True,
    )
    parser.add_argument(
        "--prior-fresh-boot-recovery-result-resolution-manifest-sha256",
        required=True,
    )
    parser.add_argument(
        "--prior-fresh-boot-recovery-result-resolution-attempt-id",
        required=True,
    )
    parser.add_argument("--fresh-boot-resume-attempt-id", required=True)
    parser.add_argument("--resume-timeout-seconds", type=int, default=1500)
    parser.add_argument("--evidence-settle-seconds", type=int, default=10)
    parser.add_argument(
        "--prior-fresh-boot-resume-root", type=Path, required=True
    )
    parser.add_argument(
        "--prior-fresh-boot-resume-manifest-sha256", required=True
    )
    parser.add_argument(
        "--prior-fresh-boot-resume-attempt-id", required=True
    )
    parser.add_argument(
        "--transaction-state-resolution-attempt-id", required=True
    )
    parser.add_argument(
        "--prior-transaction-state-resolution-root", type=Path, required=True
    )
    parser.add_argument(
        "--prior-transaction-state-resolution-manifest-sha256", required=True
    )
    parser.add_argument(
        "--prior-transaction-state-resolution-attempt-id", required=True
    )
    parser.add_argument(
        "--transaction-state-result-resolution-attempt-id", required=True
    )
    for name in (
        "authorized-transaction-state-result-resolution",
        "authorized-bound-prior-transaction-state-resolution-result",
        "authorized-two-existing-transaction-state-result-readbacks",
        "authorized-one-existing-transaction-state-phase-readback",
        "authorized-no-inventory-list-status-start-push-exec-probe-resume-dpkg-retry-repair-cleanup-or-automatic-stop",
    ):
        parser.add_argument(f"--{name}", action="store_true")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    values = vars(args).copy()
    values.pop("command")
    values.update(prior_control._prior_chain_authorizations())
    values.update(
        {
            "authorized_transaction_state_resolution": True,
            "authorized_bound_prior_indeterminate_resume_result": True,
            "authorized_one_all_stopped_inventory": True,
            "authorized_one_foreground_start_for_read_only_observation": True,
            "authorized_one_read_only_transaction_observation": True,
            "authorized_terminal_stop_separate_and_not_performed": True,
            "authorized_no_resume_dpkg_mutation_retry_repair_cleanup_or_automatic_stop": True,
        }
    )
    request = TransactionStateResultResolutionRequest(**values)
    try:
        result = run_transaction_state_result_resolution(request)
    except (
        TransactionStateResultResolutionError,
        boot_control.BootStartResolutionError,
        ValueError,
    ) as exc:
        print(
            "transaction state result resolution rejected before evidence "
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
