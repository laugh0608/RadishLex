#!/usr/bin/env python3
from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
from typing import Protocol

import l6_utm_guest_network_ready as network_ready
import l6_utm_install_artifacts_staged_boot_transport_bindings as transport_bindings
import l6_utm_install_artifacts_staged_guest_agent_result_bindings as result_bindings


EVIDENCE_FORMAT = (
    "radishlex-linux-l6-utm-install-artifacts-staged-"
    "boot-classification-resolution-v1"
)
CONTROL_RELATIVE_PATH = Path(
    "scripts/linux-product/"
    "l6_utm_install_artifacts_staged_boot_classification_resolution.py"
)
BINDINGS_RELATIVE_PATH = Path(
    "scripts/linux-product/"
    "l6_utm_install_artifacts_staged_boot_classification_bindings.py"
)
PROBE_RELATIVE_PATH = result_bindings.PROBE_RELATIVE_PATH
REQUIRED_PRIOR_GUEST_AGENT_MANIFEST_SHA256 = (
    result_bindings.REQUIRED_PRIOR_GUEST_AGENT_MANIFEST_SHA256
)
REQUIRED_PRIOR_GUEST_AGENT_ATTEMPT_ID = (
    result_bindings.REQUIRED_PRIOR_GUEST_AGENT_ATTEMPT_ID
)
REQUIRED_BOOT_CLASSIFICATION_ATTEMPT_ID = (
    "d75818f-v4-install-artifacts-staged-boot-classification-20260826-v1"
)


class BootClassificationBindingRequest(
    result_bindings.GuestAgentResultBindingRequest, Protocol
):
    boot_classification_attempt_id: str


@dataclass(frozen=True)
class BootClassificationBinding:
    evidence: dict[str, object]
    result: result_bindings.GuestAgentResultBinding
    upstream: transport_bindings.BootTransportBinding
    expected_boot_id_sha256: str
    probe_bytes: bytes
    prior_backend_pid: int


def validate_boot_classification_bindings(
    request: BootClassificationBindingRequest,
) -> BootClassificationBinding:
    if (
        request.boot_classification_attempt_id
        != REQUIRED_BOOT_CLASSIFICATION_ATTEMPT_ID
        or request.boot_start_attempt_id
        != REQUIRED_BOOT_CLASSIFICATION_ATTEMPT_ID
    ):
        raise ValueError("required-boot-classification-attempt-id-mismatch")
    if (
        request.prior_guest_agent_manifest_sha256
        != REQUIRED_PRIOR_GUEST_AGENT_MANIFEST_SHA256
        or request.prior_guest_agent_attempt_id
        != REQUIRED_PRIOR_GUEST_AGENT_ATTEMPT_ID
        or request.guest_agent_attempt_id
        != REQUIRED_PRIOR_GUEST_AGENT_ATTEMPT_ID
    ):
        raise ValueError("required-prior-guest-agent-result-mismatch")

    result = result_bindings.validate_guest_agent_result_bindings(request)
    identities: dict[str, object] = {}
    for relative_path, label in (
        (CONTROL_RELATIVE_PATH, "boot_classification_control"),
        (BINDINGS_RELATIVE_PATH, "boot_classification_bindings"),
        (PROBE_RELATIVE_PATH, "boot_transport_probe"),
    ):
        path = request.repository_root / relative_path
        network_ready._require_committed_regular(path, label)
        identities[f"{label}_sha256"] = network_ready._sha256_file(path)

    upstream = result.upstream.upstream.upstream
    return BootClassificationBinding(
        evidence={
            "format": EVIDENCE_FORMAT,
            **identities,
            "boot_classification_attempt_id": (
                request.boot_classification_attempt_id
            ),
            "expected_boot_id_sha256": result.expected_boot_id_sha256,
            "prior_guest_agent_attempt_id": (
                request.prior_guest_agent_attempt_id
            ),
            "prior_guest_agent_backend_pid": result.prior_backend_pid,
            "prior_guest_agent_entries_verified": (
                result.evidence["prior_guest_agent_entries_verified"]
            ),
            "prior_guest_agent_manifest_sha256": (
                request.prior_guest_agent_manifest_sha256
            ),
            "prior_guest_agent_marker": result.evidence[
                "prior_guest_agent_marker"
            ],
            "prior_guest_agent_outcome": result.evidence[
                "prior_guest_agent_outcome"
            ],
            "prior_guest_agent_result_binding_sha256": (
                result.evidence["guest_agent_result_bindings_sha256"]
            ),
            "repository_clean": True,
            "repository_head": request.expected_repository_head,
            "target_uuid": request.target_uuid,
        },
        result=result,
        upstream=upstream,
        expected_boot_id_sha256=result.expected_boot_id_sha256,
        probe_bytes=result.probe_bytes,
        prior_backend_pid=result.prior_backend_pid,
    )
