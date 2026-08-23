#!/usr/bin/env python3
from __future__ import annotations

import base64
import hashlib
from dataclasses import dataclass
from pathlib import Path
from types import SimpleNamespace
from typing import Protocol

import l6_guest_case_contract as guest_case_contract
import l6_utm_guest_network_ready as network_ready
import l6_utm_start_once as start_control
import l6_v4_canonical_input_install as guest_installer


EVIDENCE_FORMAT = "radishlex-linux-l6-utm-canonical-input-transfer-v1"
CONTROL_RELATIVE_PATH = Path(
    "scripts/linux-product/l6_utm_canonical_input_transfer.py"
)
BINDINGS_RELATIVE_PATH = Path(
    "scripts/linux-product/l6_utm_canonical_input_bindings.py"
)
GUEST_INSTALLER_RELATIVE_PATH = Path(
    "scripts/linux-product/l6_v4_canonical_input_install.py"
)
GUEST_CASE_CONTRACT_RELATIVE_PATH = Path(
    "scripts/linux-product/l6_guest_case_contract.py"
)
REQUIRED_PRIOR_NETWORK_MANIFEST_SHA256 = (
    "40be3f3f3bfaab52021af20ff19f7ea66829c592d519414703cea3c634038383"
)
REQUIRED_PRIOR_NETWORK_REPOSITORY_HEAD = (
    "3286268a9ffdae47b4a571960ab07a2c1cd08068"
)


class BindingRequest(Protocol):
    repository_root: Path
    expected_repository_head: str
    prior_network_root: Path
    prior_network_manifest_sha256: str
    target_uuid: str
    target_name: str


@dataclass(frozen=True)
class NetworkBinding:
    evidence: dict[str, object]
    guest_evidence_path: str
    guest_evidence_bytes: bytes


def validate_transfer_bindings(request: BindingRequest) -> NetworkBinding:
    if (
        Path(guest_case_contract.__file__).absolute()
        != request.repository_root / GUEST_CASE_CONTRACT_RELATIVE_PATH
    ):
        raise ValueError("executed-guest-case-contract-path-mismatch")
    repository_head = start_control._run_git(
        request.repository_root, ("rev-parse", "HEAD")
    ).decode("ascii").strip()
    if repository_head != request.expected_repository_head:
        raise ValueError("repository-head-drift")
    if start_control._run_git(
        request.repository_root, ("status", "--porcelain")
    ):
        raise ValueError("repository-not-clean")

    identities: dict[str, object] = {}
    for relative_path, label in (
        (CONTROL_RELATIVE_PATH, "control"),
        (BINDINGS_RELATIVE_PATH, "bindings"),
        (GUEST_INSTALLER_RELATIVE_PATH, "guest_installer"),
        (GUEST_CASE_CONTRACT_RELATIVE_PATH, "guest_case_contract"),
    ):
        path = request.repository_root / relative_path
        network_ready._require_committed_regular(path, label)
        identities[f"{label}_sha256"] = network_ready._sha256_file(path)
    expected_inventory = guest_case_contract.canonical_input_inventory(
        "radishlex_26.7.1+38-1_arm64.deb",
        "radishlex_26.7.1+38-2_arm64.deb",
    )
    if expected_inventory != guest_installer.CANONICAL_INVENTORY:
        raise ValueError("canonical-inventory-module-drift")

    manifest = request.prior_network_root / "files.sha256"
    if network_ready._sha256_file(manifest) != request.prior_network_manifest_sha256:
        raise ValueError("prior-network-manifest-drift")
    entries = start_control._verify_sha256_manifest(
        request.prior_network_root, manifest
    )
    if entries != 14:
        raise ValueError("prior-network-entry-count-invalid")
    prior_request = network_ready._read_json(
        request.prior_network_root / "request.json"
    )
    prior_terminal = network_ready._read_json(
        request.prior_network_root / "terminal.json"
    )
    required_terminal = {
        "automatic_quit": "not-performed",
        "automatic_retry": "not-performed",
        "automatic_stop": "not-performed",
        "business_input": "not-performed",
        "file_pull_invocations": 3,
        "file_push_invocations": 1,
        "guest_exec_invocations": 3,
        "network_evidence_outcome": "passed",
        "network_script_command_exit_code": 0,
        "network_script_command_timed_out": False,
        "network_script_invocations": 1,
        "operation_id": "not-generated",
        "outcome": "network-ready",
        "plain_utmctl_list": "not-performed",
        "plain_utmctl_start": "not-performed",
        "plain_utmctl_status": "not-performed",
        "reason": "double-readback-loopback-only-main-routes-empty",
        "target_name": request.target_name,
        "target_uuid": request.target_uuid,
        "transaction": "not-performed",
    }
    if any(
        prior_terminal.get(key) != value
        for key, value in required_terminal.items()
    ):
        raise ValueError("prior-network-terminal-semantics-invalid")
    if (
        prior_request.get("format") != network_ready.EVIDENCE_FORMAT
        or prior_request.get("expected_repository_head")
        != REQUIRED_PRIOR_NETWORK_REPOSITORY_HEAD
        or prior_request.get("target_uuid") != request.target_uuid
        or prior_request.get("target_name") != request.target_name
        or prior_request.get("prior_network_failure_manifest_sha256")
        != network_ready.REQUIRED_PRIOR_NETWORK_FAILURE_MANIFEST_SHA256
        or prior_request.get("guest_root")
        != (
            "/var/tmp/radishlex-l6-v4-network-ready-"
            f"{prior_request.get('attempt_id', '')}"
        )
    ):
        raise ValueError("prior-network-request-semantics-invalid")
    guest_root = prior_request.get("guest_root")
    attempt_id = prior_request.get("attempt_id")
    if not isinstance(guest_root, str) or not isinstance(attempt_id, str):
        raise ValueError("prior-network-guest-root-invalid")

    first = _prior_readback_bytes(
        request.prior_network_root / "guest-network-evidence-readback-1.json"
    )
    second = _prior_readback_bytes(
        request.prior_network_root / "guest-network-evidence-readback-2.json"
    )
    if first != second:
        raise ValueError("prior-network-evidence-readback-drift")
    parsed = network_ready.parse_guest_network_evidence(
        first, SimpleNamespace(attempt_id=attempt_id)  # type: ignore[arg-type]
    )
    persisted = network_ready._read_json(
        request.prior_network_root / "guest-network-evidence.json"
    )
    if parsed != persisted or any(
        parsed.get(key) != value
        for key, value in (
            ("outcome", "passed"),
            ("reason", "none"),
            ("active_interface_count", 1),
            ("active_interfaces", "lo"),
            ("non_loopback_up_count", 0),
            ("ipv4_main_route_count", 0),
            ("ipv6_main_route_count", 0),
        )
    ):
        raise ValueError("prior-network-evidence-semantics-invalid")
    evidence = {
        "format": EVIDENCE_FORMAT,
        **identities,
        "canonical_inventory": list(expected_inventory),
        "prior_network_boot_id": parsed["boot_id"],
        "prior_network_entries_verified": entries,
        "prior_network_evidence_sha256": hashlib.sha256(first).hexdigest(),
        "prior_network_evidence_size": len(first),
        "prior_network_manifest_sha256": request.prior_network_manifest_sha256,
        "repository_clean": True,
        "repository_head": repository_head,
    }
    return NetworkBinding(
        evidence=evidence,
        guest_evidence_path=f"{guest_root}/network.evidence",
        guest_evidence_bytes=first,
    )


def _prior_readback_bytes(path: Path) -> bytes:
    value = network_ready._read_json(path)
    stdout = value.get("stdout")
    stderr = value.get("stderr")
    if not isinstance(stdout, dict) or not isinstance(stderr, dict):
        raise ValueError("prior-network-readback-observation-invalid")
    try:
        payload = base64.b64decode(
            str(stdout["prefix_base64"]), validate=True
        )
    except (KeyError, ValueError) as exc:
        raise ValueError("prior-network-readback-base64-invalid") from exc
    if (
        value.get("exit_code") != 0
        or value.get("timed_out") is not False
        or stdout.get("truncated") is not False
        or stdout.get("total_bytes") != len(payload)
        or stdout.get("sha256") != hashlib.sha256(payload).hexdigest()
        or stderr.get("total_bytes") != 0
        or stderr.get("truncated") is not False
    ):
        raise ValueError("prior-network-readback-metadata-invalid")
    return payload
