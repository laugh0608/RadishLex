#!/usr/bin/env python3
from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
from typing import Protocol

import l6_utm_guest_network_ready as network_ready
import l6_utm_install_artifacts_staged_fresh_boot_classification_result_bindings as classification_result_bindings
import l6_v4_install_artifacts_staged_new_boot_recovery_preflight as guest_probe


EVIDENCE_FORMAT = (
    "radishlex-linux-l6-utm-install-artifacts-staged-"
    "fresh-boot-recovery-preflight-binding-v1"
)
CONTROL_RELATIVE_PATH = Path(
    "scripts/linux-product/"
    "l6_utm_install_artifacts_staged_fresh_boot_recovery_preflight.py"
)
BINDINGS_RELATIVE_PATH = Path(
    "scripts/linux-product/"
    "l6_utm_install_artifacts_staged_fresh_boot_recovery_preflight_bindings.py"
)
PROBE_RELATIVE_PATH = Path(
    "scripts/linux-product/"
    "l6_v4_install_artifacts_staged_new_boot_recovery_preflight.py"
)
RESUME_DRIVER_RELATIVE_PATH = Path(
    "scripts/linux-product/l6_v4_install_artifacts_staged_resume_driver.py"
)
REQUIRED_ATTEMPT_ID = guest_probe.FRESH_BOOT_RECOVERY_ATTEMPT_ID
REQUIRED_PRIOR_CLASSIFICATION_ATTEMPT_ID = (
    classification_result_bindings.bindings.REQUIRED_ATTEMPT_ID
)
REQUIRED_PRIOR_BOOT_ID_SHA256 = guest_probe.EXPECTED_PRIOR_BOOT_ID_SHA256
REQUIRED_ENDED_BOOT_ID_SHA256 = guest_probe.EXPECTED_CURRENT_BOOT_ID_SHA256
REQUIRED_RESUME_DRIVER_SHA256 = guest_probe.EXPECTED_RESUME_DRIVER_SHA256


class FreshBootRecoveryPreflightBindingRequest(
    classification_result_bindings.FreshBootClassificationResultBindingRequest,
    Protocol,
):
    recovery_preflight_attempt_id: str


@dataclass(frozen=True)
class FreshBootRecoveryPreflightBinding:
    evidence: dict[str, object]
    upstream: classification_result_bindings.FreshBootClassificationResultBinding
    probe_bytes: bytes
    resume_driver_bytes: bytes
    prior_boot_id_sha256: str
    current_boot_id_sha256: str
    prior_backend_pid: int


def validate_fresh_boot_recovery_preflight_bindings(
    request: FreshBootRecoveryPreflightBindingRequest,
) -> FreshBootRecoveryPreflightBinding:
    if request.recovery_preflight_attempt_id != REQUIRED_ATTEMPT_ID:
        raise ValueError("required-fresh-boot-recovery-attempt-id-mismatch")
    if (
        request.prior_fresh_boot_classification_attempt_id
        != REQUIRED_PRIOR_CLASSIFICATION_ATTEMPT_ID
    ):
        raise ValueError("required-prior-fresh-boot-classification-mismatch")

    upstream = (
        classification_result_bindings.validate_fresh_boot_classification_result_bindings(
            request
        )
    )
    if (
        upstream.expected_boot_id_sha256 != REQUIRED_ENDED_BOOT_ID_SHA256
        or upstream.observed_boot_id_sha256 == REQUIRED_ENDED_BOOT_ID_SHA256
        or upstream.observed_boot_id_sha256 == REQUIRED_PRIOR_BOOT_ID_SHA256
    ):
        raise ValueError("fresh-boot-classification-identity-invalid")

    identities: dict[str, object] = {}
    payloads: dict[str, bytes] = {}
    for relative_path, label in (
        (CONTROL_RELATIVE_PATH, "fresh_boot_recovery_control"),
        (BINDINGS_RELATIVE_PATH, "fresh_boot_recovery_bindings"),
        (PROBE_RELATIVE_PATH, "fresh_boot_recovery_probe"),
        (RESUME_DRIVER_RELATIVE_PATH, "frozen_resume_driver"),
    ):
        path = request.repository_root / relative_path
        network_ready._require_committed_regular(path, label)
        payload = path.read_bytes()
        if not payload:
            raise ValueError(f"{label}-empty")
        identities[f"{label}_sha256"] = network_ready._sha256_file(path)
        identities[f"{label}_size"] = len(payload)
        payloads[label] = payload
    if identities["frozen_resume_driver_sha256"] != REQUIRED_RESUME_DRIVER_SHA256:
        raise ValueError("frozen-resume-driver-identity-invalid")

    return FreshBootRecoveryPreflightBinding(
        evidence={
            "format": EVIDENCE_FORMAT,
            **identities,
            "current_boot_id_sha256": upstream.observed_boot_id_sha256,
            "fresh_boot_recovery_preflight_attempt_id": (
                request.recovery_preflight_attempt_id
            ),
            "prior_boot_id_sha256": REQUIRED_PRIOR_BOOT_ID_SHA256,
            "prior_fresh_boot_classification_attempt_id": (
                request.prior_fresh_boot_classification_attempt_id
            ),
            "prior_fresh_boot_classification_backend_pid": (
                upstream.prior_backend_pid
            ),
            "prior_fresh_boot_classification_entries_verified": (
                upstream.evidence[
                    "prior_fresh_boot_classification_entries_verified"
                ]
            ),
            "prior_fresh_boot_classification_manifest_sha256": (
                request.prior_fresh_boot_classification_manifest_sha256
            ),
            "prior_fresh_boot_classification_outcome": "new-boot-started",
            "prior_fresh_boot_classification_repository_head": (
                upstream.evidence[
                    "prior_fresh_boot_classification_repository_head"
                ]
            ),
            "repository_clean": True,
            "repository_head": request.expected_repository_head,
            "target_uuid": request.target_uuid,
        },
        upstream=upstream,
        probe_bytes=payloads["fresh_boot_recovery_probe"],
        resume_driver_bytes=payloads["frozen_resume_driver"],
        prior_boot_id_sha256=REQUIRED_PRIOR_BOOT_ID_SHA256,
        current_boot_id_sha256=upstream.observed_boot_id_sha256,
        prior_backend_pid=upstream.prior_backend_pid,
    )
