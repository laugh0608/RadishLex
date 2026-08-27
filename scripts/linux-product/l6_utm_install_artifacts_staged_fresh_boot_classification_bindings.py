#!/usr/bin/env python3
from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
from typing import Protocol

import l6_utm_guest_network_ready as network_ready
import l6_utm_install_artifacts_staged_boot_classification_result_bindings as classification_result_bindings
import l6_utm_install_artifacts_staged_boot_transport_bindings as transport_bindings
import l6_utm_install_artifacts_staged_new_boot_recovery_preflight_result_bindings as recovery_result_bindings


EVIDENCE_FORMAT = (
    "radishlex-linux-l6-utm-install-artifacts-staged-"
    "fresh-boot-classification-binding-v1"
)
CONTROL_RELATIVE_PATH = Path(
    "scripts/linux-product/"
    "l6_utm_install_artifacts_staged_fresh_boot_classification.py"
)
BINDINGS_RELATIVE_PATH = Path(
    "scripts/linux-product/"
    "l6_utm_install_artifacts_staged_fresh_boot_classification_bindings.py"
)
PROBE_RELATIVE_PATH = classification_result_bindings.PROBE_RELATIVE_PATH
REQUIRED_ATTEMPT_ID = (
    "d75818f-v4-install-artifacts-staged-fresh-boot-classification-"
    "20260827-v1"
)
REQUIRED_PRIOR_RECOVERY_ATTEMPT_ID = (
    recovery_result_bindings.REQUIRED_PRIOR_ATTEMPT_ID
)
REQUIRED_PRIOR_RECOVERY_MANIFEST_SHA256 = (
    recovery_result_bindings.REQUIRED_PRIOR_MANIFEST_SHA256
)


class FreshBootClassificationBindingRequest(
    recovery_result_bindings.RecoveryPreflightResultBindingRequest,
    Protocol,
):
    prior_recovery_preflight_root: Path
    prior_recovery_preflight_manifest_sha256: str
    prior_recovery_preflight_attempt_id: str


@dataclass(frozen=True)
class FreshBootClassificationBinding:
    evidence: dict[str, object]
    result: recovery_result_bindings.RecoveryPreflightResultBinding
    upstream: transport_bindings.BootTransportBinding
    expected_boot_id_sha256: str
    probe_bytes: bytes
    prior_backend_pid: int


def validate_fresh_boot_classification_bindings(
    request: FreshBootClassificationBindingRequest,
) -> FreshBootClassificationBinding:
    if request.boot_start_attempt_id != REQUIRED_ATTEMPT_ID:
        raise ValueError("required-fresh-boot-classification-attempt-id-mismatch")
    if (
        request.prior_recovery_preflight_attempt_id
        != REQUIRED_PRIOR_RECOVERY_ATTEMPT_ID
        or request.prior_recovery_preflight_manifest_sha256
        != REQUIRED_PRIOR_RECOVERY_MANIFEST_SHA256
    ):
        raise ValueError("required-prior-recovery-preflight-result-mismatch")

    result = recovery_result_bindings.validate_recovery_preflight_result_bindings(
        request
    )
    prior_classification = result.upstream
    expected_boot_id_sha256 = prior_classification.observed_boot_id_sha256
    if (
        result.guest_recovery_outcome != "recovery-rejected"
        or result.guest_probe_transport_exit_code != 0
        or expected_boot_id_sha256
        != recovery_result_bindings.recovery_bindings.REQUIRED_CURRENT_BOOT_ID_SHA256
    ):
        raise ValueError("prior-recovery-preflight-terminal-mismatch")

    identities: dict[str, object] = {}
    payloads: dict[str, bytes] = {}
    for relative_path, label in (
        (CONTROL_RELATIVE_PATH, "fresh_boot_classification_control"),
        (BINDINGS_RELATIVE_PATH, "fresh_boot_classification_bindings"),
        (PROBE_RELATIVE_PATH, "boot_transport_probe"),
    ):
        path = request.repository_root / relative_path
        network_ready._require_committed_regular(path, label)
        payload = path.read_bytes()
        if not payload:
            raise ValueError(f"{label}-empty")
        identities[f"{label}_sha256"] = network_ready._sha256_file(path)
        identities[f"{label}_size"] = len(payload)
        payloads[label] = payload

    boot_transport = prior_classification.upstream.upstream
    return FreshBootClassificationBinding(
        evidence={
            "format": EVIDENCE_FORMAT,
            **identities,
            "ended_boot_id_sha256": expected_boot_id_sha256,
            "fresh_boot_classification_attempt_id": request.boot_start_attempt_id,
            "prior_recovery_preflight_attempt_id": (
                request.prior_recovery_preflight_attempt_id
            ),
            "prior_recovery_preflight_backend_pid": result.prior_backend_pid,
            "prior_recovery_preflight_entries_verified": result.evidence[
                "prior_recovery_preflight_entries_verified"
            ],
            "prior_recovery_preflight_guest_outcome": (
                result.guest_recovery_outcome
            ),
            "prior_recovery_preflight_host_outcome": result.evidence[
                "host_recorded_outcome"
            ],
            "prior_recovery_preflight_manifest_sha256": (
                request.prior_recovery_preflight_manifest_sha256
            ),
            "prior_recovery_preflight_result_authority": result.evidence[
                "result_authority"
            ],
            "repository_clean": True,
            "repository_head": request.expected_repository_head,
            "target_uuid": request.target_uuid,
        },
        result=result,
        upstream=boot_transport,
        expected_boot_id_sha256=expected_boot_id_sha256,
        probe_bytes=payloads["boot_transport_probe"],
        prior_backend_pid=result.prior_backend_pid,
    )
