#!/usr/bin/env python3
from __future__ import annotations

import json
from typing import Protocol

import l6_v4_canonical_input_install as guest_installer
import l6_v4_canonical_input_resolution_probe as resolution_probe


class ResolutionEvidenceRequest(Protocol):
    transfer_attempt_id: str
    resolution_attempt_id: str
    source_bundle_size: int
    source_bundle_sha256: str


def parse_transfer_evidence_for_resolution(
    payload: bytes,
    request: ResolutionEvidenceRequest,
    expected_members: tuple[guest_installer.ArchiveMember, ...],
) -> dict[str, object]:
    try:
        text = payload.decode("utf-8")
        value = json.loads(text)
    except (UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise ValueError("transfer-evidence-json-invalid") from exc
    if (
        not text.endswith("\n")
        or "\r" in text
        or "\x00" in text
        or not isinstance(value, dict)
    ):
        raise ValueError("transfer-evidence-shape-invalid")
    expected_keys = {
        "attempt_id",
        "automatic_cleanup",
        "automatic_retry",
        "automatic_stop",
        "bundle_sha256",
        "bundle_size",
        "final_input_root",
        "final_switch",
        "format",
        "inventory",
        "inventory_count",
        "operation_id",
        "outcome",
        "phase",
        "reason",
        "transaction",
    }
    fixed = {
        "attempt_id": request.transfer_attempt_id,
        "automatic_cleanup": "not-performed",
        "automatic_retry": "not-performed",
        "automatic_stop": "not-performed",
        "bundle_sha256": request.source_bundle_sha256,
        "bundle_size": request.source_bundle_size,
        "final_input_root": str(guest_installer.FINAL_INPUT_ROOT),
        "format": guest_installer.EVIDENCE_FORMAT,
        "operation_id": "not-generated",
        "transaction": "not-performed",
    }
    if set(value) != expected_keys or any(
        value.get(key) != expected for key, expected in fixed.items()
    ):
        raise ValueError("transfer-evidence-identity-invalid")
    exact_inventory = [member.as_json() for member in expected_members]
    outcome = value.get("outcome")
    phase = value.get("phase")
    final_switch = value.get("final_switch")
    reason = value.get("reason")
    inventory = value.get("inventory")
    inventory_count = value.get("inventory_count")
    if outcome == "passed":
        if (
            phase != "final-readback"
            or final_switch != "performed"
            or reason != "none"
            or inventory != exact_inventory
            or inventory_count != len(exact_inventory)
        ):
            raise ValueError("transfer-evidence-passed-semantics-invalid")
    elif outcome == "failed":
        if (
            phase
            not in {
                "root-preflight",
                "attempt-marker",
                "bundle-normalize",
                "archive-validate",
                "private-staging",
                "atomic-switch",
                "final-readback",
            }
            or final_switch not in {"not-performed", "performed"}
            or not isinstance(reason, str)
            or reason in {"", "none"}
        ):
            raise ValueError("transfer-evidence-failed-semantics-invalid")
        pre_inventory_phases = {
            "root-preflight",
            "attempt-marker",
            "bundle-normalize",
            "archive-validate",
        }
        expected_inventory = [] if phase in pre_inventory_phases else exact_inventory
        if inventory != expected_inventory or inventory_count != len(
            expected_inventory
        ):
            raise ValueError("transfer-evidence-failed-inventory-invalid")
        if final_switch == "performed" and phase != "final-readback":
            raise ValueError(
                "transfer-evidence-failed-switch-semantics-invalid"
            )
    else:
        raise ValueError("transfer-evidence-outcome-invalid")
    return value


def parse_probe_evidence(
    payload: bytes, request: ResolutionEvidenceRequest
) -> dict[str, object]:
    try:
        text = payload.decode("utf-8")
        value = json.loads(text)
    except (UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise ValueError("probe-evidence-json-invalid") from exc
    if (
        not text.endswith("\n")
        or "\r" in text
        or "\x00" in text
        or not isinstance(value, dict)
    ):
        raise ValueError("probe-evidence-shape-invalid")
    expected_keys = {
        "active_installer_count",
        "automatic_cleanup",
        "automatic_retry",
        "automatic_stop",
        "bundle_identity",
        "final_input_root",
        "final_input_inventory_identity",
        "final_input_root_state",
        "format",
        "installer_identity",
        "operation_id",
        "outcome",
        "phase",
        "reason",
        "resolution_attempt_id",
        "staging_root_state",
        "suspicious_reference_count",
        "transaction",
        "transfer_attempt_id",
        "transfer_evidence_identity",
        "transfer_marker_identity",
        "transfer_root_state",
    }
    fixed = {
        "automatic_cleanup": "not-performed",
        "automatic_retry": "not-performed",
        "automatic_stop": "not-performed",
        "final_input_root": str(guest_installer.FINAL_INPUT_ROOT),
        "format": resolution_probe.EVIDENCE_FORMAT,
        "operation_id": "not-generated",
        "resolution_attempt_id": request.resolution_attempt_id,
        "transaction": "not-performed",
        "transfer_attempt_id": request.transfer_attempt_id,
    }
    if set(value) != expected_keys or any(
        value.get(key) != expected for key, expected in fixed.items()
    ):
        raise ValueError("probe-evidence-identity-invalid")
    for key in ("active_installer_count", "suspicious_reference_count"):
        count = value.get(key)
        if not isinstance(count, int) or isinstance(count, bool) or count < 0:
            raise ValueError("probe-evidence-process-count-invalid")
    if value.get("final_input_inventory_identity") not in {
        "matched",
        "not-observed",
    }:
        raise ValueError("probe-evidence-final-inventory-identity-invalid")
    if value.get("outcome") == "passed":
        if any(
            value.get(key) != expected
            for key, expected in {
                "active_installer_count": 0,
                "bundle_identity": "matched",
                "final_input_inventory_identity": (
                    "matched"
                    if value.get("final_input_root_state")
                    == "private-directory"
                    else "not-observed"
                ),
                "installer_identity": "matched",
                "phase": "complete",
                "reason": "none",
                "suspicious_reference_count": 0,
                "transfer_evidence_identity": "matched",
                "transfer_marker_identity": "matched",
                "transfer_root_state": "private-directory",
            }.items()
        ) or value.get("staging_root_state") not in {
            "absent",
            "private-directory",
        } or value.get("final_input_root_state") not in {
            "absent",
            "private-directory",
        }:
            raise ValueError("probe-evidence-passed-semantics-invalid")
    elif value.get("outcome") == "indeterminate":
        if not isinstance(value.get("reason"), str) or value.get("reason") in {
            "",
            "none",
        }:
            raise ValueError(
                "probe-evidence-indeterminate-semantics-invalid"
            )
    else:
        raise ValueError("probe-evidence-outcome-invalid")
    return value
