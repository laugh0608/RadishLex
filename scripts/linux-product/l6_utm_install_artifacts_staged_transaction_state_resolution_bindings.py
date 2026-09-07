#!/usr/bin/env python3
from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
from typing import Protocol

import l6_utm_guest_network_ready as network_ready
import l6_utm_install_artifacts_staged_fresh_boot_classification_bindings as classification_bindings
import l6_utm_install_artifacts_staged_fresh_boot_resume_result_bindings as prior_bindings
import l6_utm_start_once as start_control


EVIDENCE_FORMAT = (
    "radishlex-linux-l6-utm-install-artifacts-staged-"
    "transaction-state-resolution-binding-v1"
)
CONTROL_RELATIVE_PATH = Path(
    "scripts/linux-product/"
    "l6_utm_install_artifacts_staged_transaction_state_resolution.py"
)
BINDINGS_RELATIVE_PATH = Path(
    "scripts/linux-product/"
    "l6_utm_install_artifacts_staged_transaction_state_resolution_bindings.py"
)
PROBE_RELATIVE_PATH = Path(
    "scripts/linux-product/"
    "l6_v4_install_artifacts_staged_transaction_state_probe.py"
)
REQUIRED_PRIOR_MANIFEST_SHA256 = prior_bindings.REQUIRED_PRIOR_MANIFEST_SHA256
REQUIRED_PRIOR_ATTEMPT_ID = prior_bindings.REQUIRED_PRIOR_ATTEMPT_ID


class TransactionStateResolutionBindingRequest(
    prior_bindings.FreshBootResumeResultBindingRequest,
    Protocol,
):
    boot_start_attempt_id: str
    transaction_state_resolution_attempt_id: str


@dataclass(frozen=True)
class TransactionStateResolutionBinding:
    evidence: dict[str, object]
    upstream: prior_bindings.FreshBootResumeResultBinding
    baseline_inventory: tuple[start_control.RegisteredVm, ...]
    probe_bytes: bytes


def validate_transaction_state_resolution_bindings(
    request: TransactionStateResolutionBindingRequest,
) -> TransactionStateResolutionBinding:
    upstream = prior_bindings.validate_fresh_boot_resume_result_bindings(
        _PriorFreshBootResumeRequestView(request)
    )
    if (
        upstream.guest_resume_outcome != "state-indeterminate"
        or upstream.maintenance_resume_invocations != 1
        or upstream.postflight_invocations != 1
        or upstream.evidence.get("transaction") != "state-indeterminate"
        or upstream.evidence.get("dpkg_mutation_executed") != "unknown"
    ):
        raise ValueError("prior-fresh-boot-resume-result-not-indeterminate")

    identities: dict[str, object] = {}
    payloads: dict[str, bytes] = {}
    for relative, label in (
        (CONTROL_RELATIVE_PATH, "transaction_state_resolution_control"),
        (BINDINGS_RELATIVE_PATH, "transaction_state_resolution_bindings"),
        (PROBE_RELATIVE_PATH, "transaction_state_probe"),
    ):
        path = request.repository_root / relative
        network_ready._require_committed_regular(path, label)
        payload = path.read_bytes()
        if not payload:
            raise ValueError(f"{label}-empty")
        identities[f"{label}_sha256"] = network_ready._sha256_file(path)
        identities[f"{label}_size"] = len(payload)
        payloads[label] = payload

    baseline_inventory = _baseline_inventory(upstream)
    if len(baseline_inventory) != 20:
        raise ValueError("prior-baseline-inventory-count-invalid")

    return TransactionStateResolutionBinding(
        evidence={
            "format": EVIDENCE_FORMAT,
            **identities,
            "prior_fresh_boot_resume_attempt_id": (
                request.prior_fresh_boot_resume_attempt_id
            ),
            "prior_fresh_boot_resume_entries_verified": upstream.evidence[
                "prior_fresh_boot_resume_entries_verified"
            ],
            "prior_fresh_boot_resume_manifest_sha256": (
                request.prior_fresh_boot_resume_manifest_sha256
            ),
            "prior_fresh_boot_resume_outcome": upstream.guest_resume_outcome,
            "prior_transaction": upstream.evidence["transaction"],
            "repository_clean": True,
            "repository_head": request.expected_repository_head,
            "target_uuid": request.target_uuid,
            "transaction_state_resolution_attempt_id": (
                request.transaction_state_resolution_attempt_id
            ),
            "v7_baseline_vm_count": len(baseline_inventory),
        },
        upstream=upstream,
        baseline_inventory=baseline_inventory,
        probe_bytes=payloads["transaction_state_probe"],
    )


def _baseline_inventory(
    result: prior_bindings.FreshBootResumeResultBinding,
) -> tuple[start_control.RegisteredVm, ...]:
    fresh_resume = result.upstream
    recovery_resolution_result = fresh_resume.upstream
    recovery_resolution = recovery_resolution_result.upstream
    recovery_preflight_result = recovery_resolution.upstream
    recovery_preflight = recovery_preflight_result.upstream
    classification_result = recovery_preflight.upstream
    classification = classification_result.upstream
    boot_transport = classification.upstream
    runtime_resolution = boot_transport.upstream
    reactivation = runtime_resolution.upstream
    return reactivation.baseline_inventory


class _PriorFreshBootResumeRequestView:
    def __init__(self, request: TransactionStateResolutionBindingRequest) -> None:
        self._request = request

    @property
    def boot_start_attempt_id(self) -> str:
        return classification_bindings.REQUIRED_ATTEMPT_ID

    @property
    def guest_control_root(self) -> str:
        return (
            "/var/tmp/radishlex-l6-v4-boot-start-"
            + classification_bindings.REQUIRED_ATTEMPT_ID
        )

    @property
    def guest_probe_incoming(self) -> str:
        return f"{self.guest_recovery_root}/recovery-preflight.incoming.py"

    @property
    def guest_probe_path(self) -> str:
        return f"{self.guest_recovery_root}/recovery-preflight.py"

    @property
    def guest_marker_path(self) -> str:
        return f"{self.guest_recovery_root}/attempt.marker.json"

    @property
    def guest_result_path(self) -> str:
        return f"{self.guest_control_root}/boot-identity.evidence.json"

    def __getattr__(self, name: str) -> object:
        return getattr(self._request, name)
