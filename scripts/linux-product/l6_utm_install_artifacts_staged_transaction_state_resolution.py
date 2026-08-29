#!/usr/bin/env python3
from __future__ import annotations

import argparse
import hashlib
import json
import sys
from dataclasses import dataclass
from pathlib import Path

import l6_utm_install_artifacts_staged_boot_start_resolution as boot_control
import l6_utm_install_artifacts_staged_fresh_boot_classification as classification_control
import l6_utm_install_artifacts_staged_fresh_boot_resume as resume_control
import l6_utm_install_artifacts_staged_fresh_boot_resume_result_bindings as prior_bindings
import l6_utm_install_artifacts_staged_runtime_resolution as runtime_control
import l6_utm_install_artifacts_staged_transaction_state_resolution_bindings as bindings
import l6_v4_install_artifacts_staged_transaction_state_probe as guest_probe


EVIDENCE_FORMAT = (
    "radishlex-linux-l6-utm-install-artifacts-staged-"
    "transaction-state-resolution-v1"
)
CONTROL_RELATIVE_PATH = bindings.CONTROL_RELATIVE_PATH
BINDINGS_RELATIVE_PATH = bindings.BINDINGS_RELATIVE_PATH
PROBE_RELATIVE_PATH = bindings.PROBE_RELATIVE_PATH
REQUIRED_ATTEMPT_ID = guest_probe.EXPECTED_ATTEMPT_ID
EXIT_TRANSACTION_COMPLETED = 0
EXIT_ARTIFACTS_STAGED = 10
EXIT_PRECONDITION_REJECTED = 11
EXIT_STATE_INDETERMINATE = 12


@dataclass(frozen=True)
class TransactionStateResolutionRequest(resume_control.FreshBootResumeRequest):
    prior_fresh_boot_resume_root: Path
    prior_fresh_boot_resume_manifest_sha256: str
    prior_fresh_boot_resume_attempt_id: str
    transaction_state_resolution_attempt_id: str
    authorized_transaction_state_resolution: bool
    authorized_bound_prior_indeterminate_resume_result: bool
    authorized_one_all_stopped_inventory: bool
    authorized_one_foreground_start_for_read_only_observation: bool
    authorized_one_read_only_transaction_observation: bool
    authorized_terminal_stop_separate_and_not_performed: bool
    authorized_no_resume_dpkg_mutation_retry_repair_cleanup_or_automatic_stop: bool

    @property
    def probe_workflow_attempt_id(self) -> str:
        return self.transaction_state_resolution_attempt_id

    @property
    def guest_control_root(self) -> str:
        return str(guest_probe.EXPECTED_CONTROL_ROOT)

    @property
    def guest_probe_incoming(self) -> str:
        return f"{self.guest_control_root}/transaction-state-probe.incoming.py"

    @property
    def guest_probe_path(self) -> str:
        return f"{self.guest_control_root}/transaction-state-probe.py"

    @property
    def guest_marker_path(self) -> str:
        return f"{self.guest_control_root}/attempt.marker.json"

    @property
    def guest_result_path(self) -> str:
        return f"{self.guest_control_root}/transaction-state.evidence.json"

    def validate(self) -> None:
        resume_control.FreshBootResumeRequest.validate(self)
        if (
            not self.prior_fresh_boot_resume_root.is_absolute()
            or ".." in self.prior_fresh_boot_resume_root.parts
        ):
            raise boot_control.BootStartResolutionError(
                "prior-fresh-boot-resume-root-must-be-absolute-normalized"
            )
        if runtime_control._paths_overlap(
            self.output_root, self.prior_fresh_boot_resume_root
        ):
            raise boot_control.BootStartResolutionError(
                "output-root-must-not-overlap-prior-fresh-boot-resume-root"
            )
        if (
            self.prior_fresh_boot_resume_manifest_sha256
            != prior_bindings.REQUIRED_PRIOR_MANIFEST_SHA256
            or self.prior_fresh_boot_resume_attempt_id
            != prior_bindings.REQUIRED_PRIOR_ATTEMPT_ID
        ):
            raise boot_control.BootStartResolutionError(
                "required-prior-fresh-boot-resume-result-mismatch"
            )
        if self.transaction_state_resolution_attempt_id != REQUIRED_ATTEMPT_ID:
            raise boot_control.BootStartResolutionError(
                "required-transaction-state-resolution-attempt-id-mismatch"
            )
        if self.agent_readiness_attempts != 60:
            raise boot_control.BootStartResolutionError(
                "agent-readiness-attempts-must-be-exactly-60"
            )
        for authorized, reason in (
            (
                self.authorized_transaction_state_resolution,
                "transaction-state-resolution-authorization-required",
            ),
            (
                self.authorized_bound_prior_indeterminate_resume_result,
                "bound-prior-indeterminate-resume-result-authorization-required",
            ),
            (
                self.authorized_one_all_stopped_inventory,
                "one-all-stopped-inventory-authorization-required",
            ),
            (
                self.authorized_one_foreground_start_for_read_only_observation,
                "one-foreground-start-authorization-required",
            ),
            (
                self.authorized_one_read_only_transaction_observation,
                "one-read-only-transaction-observation-authorization-required",
            ),
            (
                self.authorized_terminal_stop_separate_and_not_performed,
                "terminal-stop-separate-boundary-authorization-required",
            ),
            (
                self.authorized_no_resume_dpkg_mutation_retry_repair_cleanup_or_automatic_stop,
                "forbidden-transaction-actions-boundary-authorization-required",
            ),
        ):
            if not authorized:
                raise boot_control.BootStartResolutionError(reason)

    def as_json(self) -> dict[str, object]:
        value = resume_control.FreshBootResumeRequest.as_json(self)
        value["authorization"] = {
            "bound_prior_indeterminate_resume_result": True,
            "no_resume_dpkg_mutation_retry_repair_cleanup_or_automatic_stop": True,
            "one_all_stopped_inventory": True,
            "one_foreground_start_for_read_only_observation": True,
            "one_read_only_transaction_observation": True,
            "terminal_stop_separate_and_not_performed": True,
            "transaction_state_resolution": True,
        }
        value.update(
            {
                "boot_start_attempt_id": (
                    self.transaction_state_resolution_attempt_id
                ),
                "format": EVIDENCE_FORMAT,
                "prior_boot_start_attempt_id": self.boot_start_attempt_id,
                "prior_fresh_boot_resume_attempt_id": (
                    self.prior_fresh_boot_resume_attempt_id
                ),
                "prior_fresh_boot_resume_manifest_sha256": (
                    self.prior_fresh_boot_resume_manifest_sha256
                ),
                "transaction_state_resolution_attempt_id": (
                    self.transaction_state_resolution_attempt_id
                ),
            }
        )
        return value


class TransactionStateProbeWorkflow:
    probe_relative_path = PROBE_RELATIVE_PATH
    probe_stage_name = "guest-transaction-state-probe-once"
    result_label = "guest-transaction-state-result"

    def baseline_inventory(
        self, binding: object
    ) -> tuple[boot_control.start_control.RegisteredVm, ...]:
        if not isinstance(binding, bindings.TransactionStateResolutionBinding):
            raise boot_control.BootStartResolutionError(
                "transaction-state-binding-type-invalid"
            )
        return binding.baseline_inventory

    def probe_argv(
        self,
        request: boot_control.BootStartResolutionRequest,
        binding: object,
    ) -> tuple[str, ...]:
        if not isinstance(
            request, TransactionStateResolutionRequest
        ) or not isinstance(binding, bindings.TransactionStateResolutionBinding):
            raise boot_control.BootStartResolutionError(
                "transaction-state-probe-context-invalid"
            )
        return transaction_state_probe_argv(request, binding)

    def marker_bytes(
        self,
        request: boot_control.BootStartResolutionRequest,
        binding: object,
        probe_sha256: str,
    ) -> bytes:
        del binding
        if not isinstance(request, TransactionStateResolutionRequest):
            raise boot_control.BootStartResolutionError(
                "transaction-state-marker-context-invalid"
            )
        return guest_probe.marker_bytes(
            request.transaction_state_resolution_attempt_id,
            probe_sha256,
        )

    def parse_result(
        self,
        payload: bytes,
        request: boot_control.BootStartResolutionRequest,
        binding: object,
        probe_sha256: str,
    ) -> boot_control.GuestProbeResolution:
        del binding
        if not isinstance(request, TransactionStateResolutionRequest):
            raise boot_control.BootStartResolutionError(
                "transaction-state-result-context-invalid"
            )
        return parse_transaction_state_result(
            payload, request, probe_sha256
        )


def validate_transaction_state_resolution_bindings(
    request: TransactionStateResolutionRequest,
) -> bindings.TransactionStateResolutionBinding:
    for actual, relative, label in (
        (Path(__file__).absolute(), CONTROL_RELATIVE_PATH, "control"),
        (Path(bindings.__file__).absolute(), BINDINGS_RELATIVE_PATH, "bindings"),
        (Path(guest_probe.__file__).absolute(), PROBE_RELATIVE_PATH, "probe"),
    ):
        if actual != request.repository_root / relative:
            raise boot_control.BootStartResolutionError(
                f"executed-{label}-path-mismatch"
            )
    try:
        return bindings.validate_transaction_state_resolution_bindings(request)
    except ValueError as exc:
        raise boot_control.BootStartResolutionError(str(exc)) from exc


def transaction_state_probe_argv(
    request: TransactionStateResolutionRequest,
    binding: bindings.TransactionStateResolutionBinding,
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
        request.transaction_state_resolution_attempt_id,
        "--target-uuid",
        request.target_uuid,
        "--expected-operation-id-sha256",
        guest_probe.EXPECTED_OPERATION_ID_SHA256,
        "--expected-probe-sha256",
        hashlib.sha256(binding.probe_bytes).hexdigest(),
        "--control-root",
        request.guest_control_root,
    )


def parse_transaction_state_result(
    payload: bytes,
    request: TransactionStateResolutionRequest,
    probe_sha256: str,
) -> boot_control.GuestProbeResolution:
    try:
        value = json.loads(payload.decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise boot_control.BootStartResolutionError(
            "transaction-state-result-json-invalid"
        ) from exc
    required_fields = {
        "attempt_id",
        "automatic_cleanup",
        "automatic_retry",
        "boot_id_sha256",
        "dpkg_command_profile",
        "dpkg_log_sha256",
        "dpkg_log_size",
        "dpkg_mutation",
        "dpkg_status_sha256",
        "format",
        "guard_profile",
        "maintenance_invocations",
        "operation_id",
        "operation_id_sha256",
        "outcome",
        "package_profile",
        "probe_sha256",
        "product_state_write",
        "reason",
        "receipt_sha256",
        "receipt_size",
        "resume_invocations",
        "startup_profile",
        "target_uuid",
        "transaction",
    }
    if not isinstance(value, dict) or set(value) != required_fields:
        raise boot_control.BootStartResolutionError(
            "transaction-state-result-fields-invalid"
        )
    if (
        value.get("format") != guest_probe.EVIDENCE_FORMAT
        or value.get("attempt_id")
        != request.transaction_state_resolution_attempt_id
        or value.get("target_uuid") != request.target_uuid
        or value.get("probe_sha256") != probe_sha256
        or value.get("operation_id") != "hash-only"
        or value.get("operation_id_sha256")
        != guest_probe.EXPECTED_OPERATION_ID_SHA256
        or value.get("automatic_cleanup") != "not-performed"
        or value.get("automatic_retry") != "not-performed"
        or value.get("dpkg_mutation") != "not-performed"
        or value.get("product_state_write") != "not-performed"
        or value.get("maintenance_invocations") != 0
        or value.get("resume_invocations") != 0
        or value.get("outcome") != value.get("transaction")
        or value.get("transaction")
        not in {"completed", "artifacts-staged", "state-indeterminate"}
    ):
        raise boot_control.BootStartResolutionError(
            "transaction-state-result-semantics-invalid"
        )

    transaction = str(value["transaction"])
    if transaction == "completed":
        _validate_completed_result(value)
        outcome = "transaction-completed"
        exit_code = EXIT_TRANSACTION_COMPLETED
    elif transaction == "artifacts-staged":
        _validate_artifacts_staged_result(value)
        outcome = "transaction-artifacts-staged"
        exit_code = EXIT_ARTIFACTS_STAGED
    else:
        _validate_indeterminate_result(value)
        outcome = "state-indeterminate"
        exit_code = EXIT_STATE_INDETERMINATE

    evidence = {
        "boot_id_sha256": value["boot_id_sha256"],
        "dpkg_log_sha256": value["dpkg_log_sha256"],
        "dpkg_log_size": value["dpkg_log_size"],
        "dpkg_status_sha256": value["dpkg_status_sha256"],
        "format": EVIDENCE_FORMAT,
        "guest_result_sha256": hashlib.sha256(payload).hexdigest(),
        "guest_result_size": len(payload),
        "operation_id": "hash-only",
        "operation_id_sha256": guest_probe.EXPECTED_OPERATION_ID_SHA256,
        "package_profile": value["package_profile"],
        "receipt_sha256": value["receipt_sha256"],
        "receipt_size": value["receipt_size"],
        "transaction": transaction,
    }
    return boot_control.GuestProbeResolution(
        outcome=outcome,
        exit_code=exit_code,
        transaction=transaction,
        reason=(
            "single-start-read-only-transaction-observation-and-"
            "terminal-target-stable"
        ),
        observed_boot_id_sha256=(
            str(value["boot_id_sha256"])
            if value["boot_id_sha256"] is not None
            else None
        ),
        evidence_name="transaction-state-classification.json",
        evidence=evidence,
        business_guest_action="one-read-only-transaction-observation",
        operation_id_disposition="hash-only",
    )


def _require_hash_and_positive_size(
    value: dict[str, object], digest_key: str, size_key: str
) -> None:
    digest = value.get(digest_key)
    size = value.get(size_key)
    if (
        not isinstance(digest, str)
        or not boot_control.HEX_64.fullmatch(digest)
        or not isinstance(size, int)
        or size <= 0
    ):
        raise boot_control.BootStartResolutionError(
            f"transaction-state-{digest_key}-invalid"
        )


def _validate_completed_result(value: dict[str, object]) -> None:
    for digest_key, size_key in (
        ("receipt_sha256", "receipt_size"),
        ("dpkg_log_sha256", "dpkg_log_size"),
    ):
        _require_hash_and_positive_size(value, digest_key, size_key)
    if (
        value.get("boot_id_sha256") is None
        or not boot_control.HEX_64.fullmatch(str(value["boot_id_sha256"]))
        or value.get("dpkg_status_sha256")
        != guest_probe.EXPECTED_INSTALLED_DPKG_STATUS_SHA256
        or value.get("dpkg_command_profile")
        != "audit-plus-verify-read-only"
        or value.get("guard_profile") != "absent-after-reboot"
        or value.get("package_profile") != "installed-verified"
        or value.get("startup_profile") != "allowed"
    ):
        raise boot_control.BootStartResolutionError(
            "transaction-state-completed-semantics-invalid"
        )


def _validate_artifacts_staged_result(value: dict[str, object]) -> None:
    if (
        value.get("boot_id_sha256") is None
        or not boot_control.HEX_64.fullmatch(str(value["boot_id_sha256"]))
        or value.get("receipt_sha256") != guest_probe.EXPECTED_RECEIPT_SHA256
        or value.get("receipt_size") != guest_probe.EXPECTED_RECEIPT_SIZE
        or value.get("dpkg_status_sha256")
        != guest_probe.EXPECTED_INITIAL_DPKG_STATUS_SHA256
        or value.get("dpkg_log_sha256")
        != guest_probe.EXPECTED_INITIAL_DPKG_LOG_SHA256
        or value.get("dpkg_log_size")
        != guest_probe.EXPECTED_INITIAL_DPKG_LOG_SIZE
        or value.get("dpkg_command_profile") != "audit-read-only"
        or value.get("guard_profile") != "absent-after-reboot"
        or value.get("package_profile")
        != "not-installed-staging-preserved"
        or value.get("startup_profile") != "operation-in-progress"
    ):
        raise boot_control.BootStartResolutionError(
            "transaction-state-artifacts-staged-semantics-invalid"
        )


def _validate_indeterminate_result(value: dict[str, object]) -> None:
    if (
        any(
            value.get(key) is not None
            for key in (
                "boot_id_sha256",
                "dpkg_log_sha256",
                "dpkg_log_size",
                "dpkg_status_sha256",
                "receipt_sha256",
                "receipt_size",
            )
        )
        or value.get("dpkg_command_profile") != "not-classified"
        or value.get("guard_profile") != "not-classified"
        or value.get("package_profile") != "not-classified"
        or value.get("startup_profile") != "not-classified"
        or not isinstance(value.get("reason"), str)
    ):
        raise boot_control.BootStartResolutionError(
            "transaction-state-indeterminate-semantics-invalid"
        )


def run_transaction_state_resolution(
    request: TransactionStateResolutionRequest,
    **kwargs: object,
) -> boot_control.BootStartResolutionResult:
    binding_validator = kwargs.pop(
        "binding_validator",
        validate_transaction_state_resolution_bindings,
    )
    return boot_control.run_boot_start_resolution(
        request,
        binding_validator=binding_validator,
        evidence_format=EVIDENCE_FORMAT,
        guest_probe_workflow=TransactionStateProbeWorkflow(),
        **kwargs,
    )


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "From an all-stopped inventory, start the frozen target once and "
            "observe completed, artifacts_staged, or indeterminate without "
            "resume, dpkg mutation, retry, cleanup, or automatic stop."
        )
    )
    parser.add_argument(
        "command", choices=("resolve-transaction-state-read-only",)
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
    for name in (
        "authorized-transaction-state-resolution",
        "authorized-bound-prior-indeterminate-resume-result",
        "authorized-one-all-stopped-inventory",
        "authorized-one-foreground-start-for-read-only-observation",
        "authorized-one-read-only-transaction-observation",
        "authorized-terminal-stop-separate-and-not-performed",
        "authorized-no-resume-dpkg-mutation-retry-repair-cleanup-or-automatic-stop",
    ):
        parser.add_argument(f"--{name}", action="store_true")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    values = vars(args).copy()
    values.pop("command")
    values.update(_prior_chain_authorizations())
    request = TransactionStateResolutionRequest(**values)
    try:
        result = run_transaction_state_resolution(request)
    except (boot_control.BootStartResolutionError, ValueError) as exc:
        print(f"transaction_state_resolution_error={exc}", file=sys.stderr)
        return EXIT_PRECONDITION_REJECTED
    print(f"transaction_state_resolution_outcome={result.outcome}")
    print(f"evidence_root={result.evidence_root}")
    print(f"files_sha256={result.manifest_sha256}")
    print(f"inventory_probe_invocations={result.inventory_probe_invocations}")
    print(f"foreground_start_invocations={result.foreground_start_invocations}")
    print(f"guest_probe_invocations={result.guest_probe_invocations}")
    print(f"result_readback_invocations={result.result_readback_invocations}")
    return result.exit_code


def _prior_chain_authorizations() -> dict[str, bool]:
    return {
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
        "authorized_install_artifacts_staged_fresh_boot_resume": True,
        "authorized_bound_qualified_fresh_boot_recovery_result": True,
        "authorized_one_private_fresh_boot_resume_delivery_and_execution": True,
        "authorized_one_maintenance_resume_and_postflight": True,
        "authorized_no_inventory_start_status_probe_retry_cleanup_stop_or_quit": True,
    }


if __name__ == "__main__":
    raise SystemExit(main())
