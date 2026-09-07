#!/usr/bin/env python3
from __future__ import annotations

import argparse
import sys
from dataclasses import dataclass
from pathlib import Path

import l6_utm_install_artifacts_staged_boot_start_resolution as boot_start_control
import l6_utm_install_artifacts_staged_fresh_boot_classification as classification_control
import l6_utm_install_artifacts_staged_fresh_boot_recovery_preflight_bindings as bindings
import l6_utm_install_artifacts_staged_new_boot_recovery_preflight as recovery_control
import l6_utm_install_artifacts_staged_runtime_resolution as runtime_control
import l6_v4_install_artifacts_staged_new_boot_recovery_preflight as guest_probe


EVIDENCE_FORMAT = recovery_control.EVIDENCE_FORMAT
CONTROL_RELATIVE_PATH = bindings.CONTROL_RELATIVE_PATH
BINDINGS_RELATIVE_PATH = bindings.BINDINGS_RELATIVE_PATH
PROBE_RELATIVE_PATH = bindings.PROBE_RELATIVE_PATH
EXIT_RECOVERY_QUALIFIED = recovery_control.EXIT_RECOVERY_QUALIFIED
EXIT_RECOVERY_REJECTED = recovery_control.EXIT_RECOVERY_REJECTED
EXIT_PRECONDITION_REJECTED = recovery_control.EXIT_PRECONDITION_REJECTED
EXIT_STATE_INDETERMINATE = recovery_control.EXIT_STATE_INDETERMINATE


@dataclass(frozen=True)
class FreshBootRecoveryPreflightRequest(
    classification_control.FreshBootClassificationRequest
):
    prior_fresh_boot_classification_root: Path
    prior_fresh_boot_classification_manifest_sha256: str
    prior_fresh_boot_classification_attempt_id: str
    authorized_install_artifacts_staged_fresh_boot_recovery_preflight: bool
    authorized_bound_prior_fresh_boot_classification_result: bool
    authorized_bounded_read_only_fresh_boot_recovery_probe: bool
    authorized_one_private_fresh_boot_recovery_probe_delivery_and_execution: bool
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
        classification_control.FreshBootClassificationRequest.validate(self)
        root = self.prior_fresh_boot_classification_root
        if not root.is_absolute() or ".." in root.parts:
            raise recovery_control.RecoveryPreflightError(
                "prior-fresh-boot-classification-root-must-be-absolute-normalized"
            )
        if runtime_control._paths_overlap(self.output_root, root):
            raise recovery_control.RecoveryPreflightError(
                "output-root-must-not-overlap-prior-fresh-boot-classification-root"
            )
        if (
            not boot_start_control.HEX_64.fullmatch(
                self.prior_fresh_boot_classification_manifest_sha256
            )
            or self.prior_fresh_boot_classification_attempt_id
            != bindings.REQUIRED_PRIOR_CLASSIFICATION_ATTEMPT_ID
            or self.recovery_preflight_attempt_id != bindings.REQUIRED_ATTEMPT_ID
        ):
            raise recovery_control.RecoveryPreflightError(
                "fixed-fresh-boot-recovery-input-mismatch"
            )
        for authorized, reason in (
            (
                self.authorized_install_artifacts_staged_fresh_boot_recovery_preflight,
                "fresh-boot-recovery-preflight-authorization-required",
            ),
            (
                self.authorized_bound_prior_fresh_boot_classification_result,
                "prior-fresh-boot-classification-binding-authorization-required",
            ),
            (
                self.authorized_bounded_read_only_fresh_boot_recovery_probe,
                "bounded-readonly-fresh-boot-recovery-probe-authorization-required",
            ),
            (
                self.authorized_one_private_fresh_boot_recovery_probe_delivery_and_execution,
                "one-private-fresh-boot-recovery-probe-authorization-required",
            ),
            (
                self.authorized_no_inventory_start_status_resume_dpkg_mutation_retry_stop_or_quit,
                "fresh-boot-recovery-forbidden-action-boundary-authorization-required",
            ),
        ):
            if not authorized:
                raise recovery_control.RecoveryPreflightError(reason)

    def as_json(self) -> dict[str, object]:
        value = classification_control.FreshBootClassificationRequest.as_json(self)
        value.update(
            {
                "authorization": {
                    "bound_prior_fresh_boot_classification_result": True,
                    "bounded_read_only_guest_agent_readiness": True,
                    "bounded_read_only_fresh_boot_recovery_probe": True,
                    "install_artifacts_staged_fresh_boot_recovery_preflight": True,
                    (
                        "no_inventory_start_status_resume_dpkg_mutation_retry_"
                        "stop_or_quit"
                    ): True,
                    "one_private_fresh_boot_recovery_probe_delivery_and_execution": True,
                    "two_independent_result_readbacks": True,
                },
                "format": EVIDENCE_FORMAT,
                "prior_fresh_boot_classification_attempt_id": (
                    self.prior_fresh_boot_classification_attempt_id
                ),
                "prior_fresh_boot_classification_manifest_sha256": (
                    self.prior_fresh_boot_classification_manifest_sha256
                ),
                "recovery_preflight_attempt_id": (
                    self.recovery_preflight_attempt_id
                ),
            }
        )
        return value


def validate_fresh_boot_recovery_preflight_bindings(
    request: FreshBootRecoveryPreflightRequest,
) -> bindings.FreshBootRecoveryPreflightBinding:
    expected_paths = (
        (Path(__file__).absolute(), CONTROL_RELATIVE_PATH, "control"),
        (Path(bindings.__file__).absolute(), BINDINGS_RELATIVE_PATH, "bindings"),
        (Path(guest_probe.__file__).absolute(), PROBE_RELATIVE_PATH, "probe"),
    )
    for actual, relative, label in expected_paths:
        if actual != request.repository_root / relative:
            raise recovery_control.RecoveryPreflightError(
                f"executed-{label}-path-mismatch"
            )
    try:
        return bindings.validate_fresh_boot_recovery_preflight_bindings(request)
    except ValueError as exc:
        raise recovery_control.RecoveryPreflightError(str(exc)) from exc


def run_fresh_boot_recovery_preflight(
    request: FreshBootRecoveryPreflightRequest,
    *,
    binding_validator=None,
    **kwargs: object,
) -> recovery_control.RecoveryPreflightResult:
    return recovery_control.run_recovery_preflight(
        request,
        binding_validator=(
            binding_validator
            or validate_fresh_boot_recovery_preflight_bindings
        ),
        **kwargs,
    )


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Consume one fully validated fresh-boot classification result and "
            "run one read-only recovery qualification probe without inventory, "
            "start, resume, or package mutation."
        )
    )
    parser.add_argument("command", choices=("qualify-fresh-boot-recovery",))
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
    for name in (
        "authorized-install-artifacts-staged-fresh-boot-recovery-preflight",
        "authorized-bound-prior-fresh-boot-classification-result",
        "authorized-bounded-read-only-fresh-boot-recovery-probe",
        "authorized-one-private-fresh-boot-recovery-probe-delivery-and-execution",
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
        }
    )
    request = FreshBootRecoveryPreflightRequest(**values)
    try:
        result = run_fresh_boot_recovery_preflight(request)
    except (
        recovery_control.RecoveryPreflightError,
        boot_start_control.BootStartResolutionError,
    ) as exc:
        print(
            f"fresh boot recovery preflight rejected before evidence creation: {exc}",
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
