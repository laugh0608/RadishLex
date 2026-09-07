#!/usr/bin/env python3
from __future__ import annotations

import argparse
import sys
from dataclasses import dataclass
from pathlib import Path

import l6_utm_install_artifacts_staged_boot_classification_bindings as bindings
import l6_utm_install_artifacts_staged_boot_start_resolution as boot_control
import l6_utm_install_artifacts_staged_guest_agent_bindings as guest_bindings
import l6_utm_install_artifacts_staged_guest_agent_result_bindings as result_bindings
import l6_utm_install_artifacts_staged_runtime_resolution as runtime_control
import l6_v4_boot_transport_probe as guest_probe


EVIDENCE_FORMAT = bindings.EVIDENCE_FORMAT
CONTROL_RELATIVE_PATH = bindings.CONTROL_RELATIVE_PATH
BINDINGS_RELATIVE_PATH = bindings.BINDINGS_RELATIVE_PATH
PROBE_RELATIVE_PATH = bindings.PROBE_RELATIVE_PATH
EXIT_ORIGINAL_BOOT_RESTORED = boot_control.EXIT_ORIGINAL_BOOT_RESTORED
EXIT_NEW_BOOT_STARTED = boot_control.EXIT_NEW_BOOT_STARTED
EXIT_PRECONDITION_REJECTED = boot_control.EXIT_PRECONDITION_REJECTED
EXIT_STATE_INDETERMINATE = boot_control.EXIT_STATE_INDETERMINATE


@dataclass(frozen=True)
class BootClassificationResolutionRequest(
    boot_control.BootStartResolutionRequest
):
    prior_boot_start_root: Path
    prior_boot_start_manifest_sha256: str
    prior_boot_start_attempt_id: str
    prior_guest_agent_root: Path
    prior_guest_agent_manifest_sha256: str
    guest_agent_attempt_id: str
    prior_guest_agent_attempt_id: str
    boot_classification_attempt_id: str
    agent_readiness_attempts: int
    authorized_install_artifacts_staged_boot_classification_resolution: bool
    authorized_bound_prior_guest_agent_result: bool
    authorized_bounded_read_only_guest_agent_readiness: bool

    def validate(self) -> None:
        super().validate(
            expected_boot_start_attempt_id=(
                bindings.REQUIRED_BOOT_CLASSIFICATION_ATTEMPT_ID
            )
        )
        for root, label in (
            (self.prior_boot_start_root, "prior-boot-start-root"),
            (self.prior_guest_agent_root, "prior-guest-agent-root"),
        ):
            if not root.is_absolute() or ".." in root.parts:
                raise boot_control.BootStartResolutionError(
                    f"{label}-must-be-absolute-normalized"
                )
            if runtime_control._paths_overlap(self.output_root, root):
                raise boot_control.BootStartResolutionError(
                    f"output-root-must-not-overlap-{label}"
                )
        if (
            self.prior_boot_start_manifest_sha256
            != guest_bindings.REQUIRED_PRIOR_BOOT_START_MANIFEST_SHA256
        ):
            raise boot_control.BootStartResolutionError(
                "required-prior-boot-start-manifest-mismatch"
            )
        if (
            self.prior_guest_agent_manifest_sha256
            != bindings.REQUIRED_PRIOR_GUEST_AGENT_MANIFEST_SHA256
        ):
            raise boot_control.BootStartResolutionError(
                "required-prior-guest-agent-manifest-mismatch"
            )
        for value, expected, label in (
            (
                self.prior_boot_start_attempt_id,
                guest_bindings.REQUIRED_PRIOR_BOOT_START_ATTEMPT_ID,
                "prior-boot-start",
            ),
            (
                self.guest_agent_attempt_id,
                bindings.REQUIRED_PRIOR_GUEST_AGENT_ATTEMPT_ID,
                "guest-agent",
            ),
            (
                self.prior_guest_agent_attempt_id,
                bindings.REQUIRED_PRIOR_GUEST_AGENT_ATTEMPT_ID,
                "prior-guest-agent",
            ),
            (
                self.boot_classification_attempt_id,
                bindings.REQUIRED_BOOT_CLASSIFICATION_ATTEMPT_ID,
                "boot-classification",
            ),
        ):
            if value != expected:
                raise boot_control.BootStartResolutionError(
                    f"required-{label}-attempt-id-mismatch"
                )
        if self.boot_start_attempt_id != self.boot_classification_attempt_id:
            raise boot_control.BootStartResolutionError(
                "boot-classification-boot-start-attempt-mismatch"
            )
        if self.agent_readiness_attempts != 60:
            raise boot_control.BootStartResolutionError(
                "agent-readiness-attempts-must-be-exactly-60"
            )
        for authorized, reason in (
            (
                self.authorized_install_artifacts_staged_boot_classification_resolution,
                "authorized-install-artifacts-staged-boot-classification-resolution-required",
            ),
            (
                self.authorized_bound_prior_guest_agent_result,
                "authorized-bound-prior-guest-agent-result-required",
            ),
            (
                self.authorized_bounded_read_only_guest_agent_readiness,
                "authorized-bounded-read-only-guest-agent-readiness-required",
            ),
        ):
            if not authorized:
                raise boot_control.BootStartResolutionError(reason)

    def as_json(self) -> dict[str, object]:
        value = super().as_json()
        authorization = value.get("authorization")
        if not isinstance(authorization, dict):
            raise boot_control.BootStartResolutionError(
                "boot-classification-authorization-invalid"
            )
        authorization.update(
            {
                "bound_prior_guest_agent_result": True,
                "bounded_read_only_guest_agent_readiness": True,
                "install_artifacts_staged_boot_classification_resolution": True,
            }
        )
        value.update(
            {
                "agent_readiness_attempts": self.agent_readiness_attempts,
                "boot_classification_attempt_id": (
                    self.boot_classification_attempt_id
                ),
                "format": EVIDENCE_FORMAT,
                "guest_agent_attempt_id": self.guest_agent_attempt_id,
                "prior_boot_start_attempt_id": (
                    self.prior_boot_start_attempt_id
                ),
                "prior_boot_start_manifest_sha256": (
                    self.prior_boot_start_manifest_sha256
                ),
                "prior_guest_agent_attempt_id": (
                    self.prior_guest_agent_attempt_id
                ),
                "prior_guest_agent_manifest_sha256": (
                    self.prior_guest_agent_manifest_sha256
                ),
            }
        )
        return value


def validate_boot_classification_bindings(
    request: BootClassificationResolutionRequest,
) -> bindings.BootClassificationBinding:
    expected_paths = (
        (Path(__file__).absolute(), CONTROL_RELATIVE_PATH, "control"),
        (Path(bindings.__file__).absolute(), BINDINGS_RELATIVE_PATH, "bindings"),
        (Path(guest_probe.__file__).absolute(), PROBE_RELATIVE_PATH, "probe"),
        (
            Path(result_bindings.__file__).absolute(),
            result_bindings.BINDINGS_RELATIVE_PATH,
            "guest-agent-result-bindings",
        ),
    )
    for actual, relative, label in expected_paths:
        if actual != request.repository_root / relative:
            raise boot_control.BootStartResolutionError(
                f"executed-{label}-path-mismatch"
            )
    try:
        return bindings.validate_boot_classification_bindings(request)
    except ValueError as exc:
        raise boot_control.BootStartResolutionError(str(exc)) from exc


def run_boot_classification_resolution(
    request: BootClassificationResolutionRequest,
    *,
    binding_validator=None,
    **kwargs: object,
) -> boot_control.BootStartResolutionResult:
    return boot_control.run_boot_start_resolution(
        request,
        binding_validator=(
            binding_validator or validate_boot_classification_bindings
        ),
        evidence_format=EVIDENCE_FORMAT,
        **kwargs,
    )


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "From a fully stopped inventory, start one frozen artifacts_staged "
            "target once, wait for bounded guest-agent readiness, and classify "
            "boot identity through the fixed-scope private probe."
        )
    )
    parser.add_argument("command", choices=("classify-boot-after-start-once",))
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
        "authorized-install-artifacts-staged-boot-start-resolution",
        "authorized-one-potential-backend-reactivation-list",
        "authorized-one-foreground-start-from-stopped",
        "authorized-bounded-target-runtime-observation",
        "authorized-one-private-guest-probe-delivery-and-execution",
        "authorized-two-independent-result-readbacks",
        "authorized-no-status-resume-business-guest-retry-stop-or-quit",
        "authorized-install-artifacts-staged-boot-classification-resolution",
        "authorized-bound-prior-guest-agent-result",
        "authorized-bounded-read-only-guest-agent-readiness",
    ):
        parser.add_argument(f"--{name}", action="store_true")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    values = vars(args).copy()
    values.pop("command")
    request = BootClassificationResolutionRequest(**values)
    try:
        result = run_boot_classification_resolution(request)
    except boot_control.BootStartResolutionError as exc:
        print(
            "boot classification rejected before evidence creation: "
            f"{exc}",
            file=sys.stderr,
        )
        return EXIT_PRECONDITION_REJECTED
    print(f"outcome={result.outcome}")
    print(f"evidence_root={result.evidence_root}")
    print(f"manifest_sha256={result.manifest_sha256}")
    print(f"inventory_probe_invocations={result.inventory_probe_invocations}")
    print(f"foreground_start_invocations={result.foreground_start_invocations}")
    print(f"guest_probe_invocations={result.guest_probe_invocations}")
    print(f"result_readback_invocations={result.result_readback_invocations}")
    return result.exit_code


if __name__ == "__main__":
    raise SystemExit(main())
