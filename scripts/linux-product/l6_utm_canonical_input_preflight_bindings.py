#!/usr/bin/env python3
from __future__ import annotations

import hashlib
from dataclasses import dataclass
from pathlib import Path
from types import SimpleNamespace
from typing import Protocol

import l6_utm_canonical_input_bindings as transfer_bindings
import l6_utm_canonical_input_resolution_bindings as resolution_bindings
import l6_utm_canonical_input_resolution_evidence as resolution_evidence
import l6_utm_guest_network_ready as network_ready
import l6_utm_start_once as start_control
import l6_v4_canonical_input_install as guest_installer
import l6_v4_canonical_input_preflight_probe as preflight_probe


EVIDENCE_FORMAT = "radishlex-linux-l6-utm-canonical-input-preflight-v1"
CONTROL_RELATIVE_PATH = Path(
    "scripts/linux-product/l6_utm_canonical_input_preflight.py"
)
BINDINGS_RELATIVE_PATH = Path(
    "scripts/linux-product/l6_utm_canonical_input_preflight_bindings.py"
)
PROBE_RELATIVE_PATH = Path(
    "scripts/linux-product/l6_v4_canonical_input_preflight_probe.py"
)
REQUIRED_PRIOR_RESOLUTION_MANIFEST_SHA256 = (
    "7bab8f2095769e7d5764d36330c678f693260ad576ce31437cea3552ddfd4e4a"
)
REQUIRED_PRIOR_RESOLUTION_REPOSITORY_HEAD = (
    "ca57dce50a9d15c1f217622f097ea8a021cf8c31"
)
REQUIRED_RESOLUTION_ATTEMPT_ID = (
    "d75818f-v4-input-resolution-20260823-v1"
)
REQUIRED_RESOLUTION_ENTRY_NAMES = (
    "request.json",
    "binding-preflight.json",
    "source-bundle-preflight.json",
    "target-files-preflight.json",
    "host-process-preflight.json",
    "target-handles-preflight.json",
    "network-evidence-live-readback-1.json",
    "network-evidence-live-readback-2.json",
    "existing-transfer-evidence-readback-1.json",
    "existing-transfer-evidence-readback-2.json",
    "existing-transfer-evidence.json",
    "guest-resolution-root-create.json",
    "guest-resolution-probe-push.json",
    "guest-resolution-probe-normalize.json",
    "guest-resolution-probe-readback.json",
    "guest-resolution-probe.json",
    "guest-resolution-evidence-readback-1.json",
    "guest-resolution-evidence-readback-2.json",
    "guest-resolution-evidence.json",
    "source-bundle-postflight.json",
    "terminal.json",
)


class PreflightBindingRequest(Protocol):
    repository_root: Path
    expected_repository_head: str
    prior_network_root: Path
    prior_network_manifest_sha256: str
    prior_transfer_root: Path
    prior_transfer_manifest_sha256: str
    prior_resolution_root: Path
    prior_resolution_manifest_sha256: str
    source_bundle_path: Path
    source_bundle_size: int
    source_bundle_sha256: str
    transfer_attempt_id: str
    resolution_attempt_id: str
    target_uuid: str
    target_name: str
    target_package_path: Path


@dataclass(frozen=True)
class PreflightBinding:
    evidence: dict[str, object]
    network: transfer_bindings.NetworkBinding
    probe_bytes: bytes
    resolution_evidence_bytes: bytes


def validate_preflight_bindings(
    request: PreflightBindingRequest,
) -> PreflightBinding:
    upstream = resolution_bindings.validate_resolution_bindings(request)
    identities: dict[str, object] = {}
    for relative_path, label in (
        (CONTROL_RELATIVE_PATH, "control"),
        (BINDINGS_RELATIVE_PATH, "bindings"),
        (PROBE_RELATIVE_PATH, "probe"),
    ):
        path = request.repository_root / relative_path
        network_ready._require_committed_regular(path, label)
        identities[f"{label}_sha256"] = network_ready._sha256_file(path)

    manifest = request.prior_resolution_root / "files.sha256"
    if (
        request.prior_resolution_manifest_sha256
        != REQUIRED_PRIOR_RESOLUTION_MANIFEST_SHA256
        or network_ready._sha256_file(manifest)
        != REQUIRED_PRIOR_RESOLUTION_MANIFEST_SHA256
    ):
        raise ValueError("prior-resolution-manifest-drift")
    entries = start_control._verify_sha256_manifest(
        request.prior_resolution_root, manifest
    )
    if entries != len(REQUIRED_RESOLUTION_ENTRY_NAMES):
        raise ValueError("prior-resolution-entry-count-invalid")
    if (
        resolution_bindings._manifest_names(manifest)
        != REQUIRED_RESOLUTION_ENTRY_NAMES
    ):
        raise ValueError("prior-resolution-entry-names-invalid")

    prior_request = network_ready._read_json(
        request.prior_resolution_root / "request.json"
    )
    expected_authorization = {
        "canonical_input_resolution": True,
        "create_new_read_only_probe": True,
        "existing_result_double_readback": True,
        "no_transfer_operation_transaction_stop_or_retry": True,
    }
    if (
        prior_request.get("format") != resolution_bindings.EVIDENCE_FORMAT
        or prior_request.get("expected_repository_head")
        != REQUIRED_PRIOR_RESOLUTION_REPOSITORY_HEAD
        or prior_request.get("authorization") != expected_authorization
        or prior_request.get("resolution_attempt_id")
        != request.resolution_attempt_id
        or prior_request.get("transfer_attempt_id")
        != request.transfer_attempt_id
        or prior_request.get("prior_network_manifest_sha256")
        != request.prior_network_manifest_sha256
        or prior_request.get("prior_transfer_manifest_sha256")
        != request.prior_transfer_manifest_sha256
        or prior_request.get("source_bundle_size")
        != request.source_bundle_size
        or prior_request.get("source_bundle_sha256")
        != request.source_bundle_sha256
        or prior_request.get("target_uuid") != request.target_uuid
        or prior_request.get("target_name") != request.target_name
        or prior_request.get("guest_final_input_root")
        != str(guest_installer.FINAL_INPUT_ROOT)
    ):
        raise ValueError("prior-resolution-request-semantics-invalid")
    expected_guest_resolution_root = (
        "/var/tmp/radishlex-l6-v4-input-resolution-"
        f"{request.resolution_attempt_id}"
    )
    if (
        prior_request.get("guest_resolution_root")
        != expected_guest_resolution_root
    ):
        raise ValueError("prior-resolution-guest-root-invalid")

    members = tuple(
        guest_installer.ArchiveMember(
            item.path, item.mode, item.size, item.sha256
        )
        for item in preflight_probe.INPUT_MEMBERS
    )
    transfer_payload = (
        request.prior_resolution_root / "existing-transfer-evidence.json"
    ).read_bytes()
    parsed_transfer = resolution_evidence.parse_transfer_evidence_for_resolution(
        transfer_payload, request, members
    )
    if (
        parsed_transfer.get("outcome") != "passed"
        or parsed_transfer.get("phase") != "final-readback"
        or parsed_transfer.get("final_switch") != "performed"
    ):
        raise ValueError("prior-resolution-transfer-outcome-invalid")

    resolution_readbacks = tuple(
        transfer_bindings._prior_readback_bytes(
            request.prior_resolution_root
            / f"guest-resolution-evidence-readback-{index}.json"
        )
        for index in (1, 2)
    )
    if resolution_readbacks[0] != resolution_readbacks[1]:
        raise ValueError("prior-resolution-evidence-readback-drift")
    parsed_resolution = resolution_evidence.parse_probe_evidence(
        resolution_readbacks[0], request
    )
    persisted_resolution = network_ready._read_json(
        request.prior_resolution_root / "guest-resolution-evidence.json"
    )
    if parsed_resolution != persisted_resolution or any(
        parsed_resolution.get(key) != expected
        for key, expected in {
            "active_installer_count": 0,
            "final_input_inventory_identity": "matched",
            "final_input_root_state": "private-directory",
            "outcome": "passed",
            "phase": "complete",
            "reason": "none",
            "staging_root_state": "absent",
            "suspicious_reference_count": 0,
        }.items()
    ):
        raise ValueError("prior-resolution-evidence-semantics-invalid")

    terminal = network_ready._read_json(
        request.prior_resolution_root / "terminal.json"
    )
    required_terminal = {
        "automatic_cleanup": "not-performed",
        "automatic_quit": "not-performed",
        "automatic_retry": "not-performed",
        "automatic_stop": "not-performed",
        "bundle_push_invocations": 0,
        "file_pull_invocations": 7,
        "file_push_invocations": 1,
        "guest_exec_invocations": 3,
        "guest_probe_outcome": "passed",
        "guest_transfer_outcome": "passed",
        "installer_invocations": 0,
        "operation_id": "not-generated",
        "outcome": "input-ready",
        "plain_utmctl_list": "not-performed",
        "plain_utmctl_start": "not-performed",
        "plain_utmctl_status": "not-performed",
        "probe_invocations": 1,
        "probe_result_readback_invocations": 2,
        "reason": "stable-transfer-result-and-read-only-probe-passed",
        "resolution_attempt_id": request.resolution_attempt_id,
        "source_bundle_sha256": request.source_bundle_sha256,
        "source_bundle_size": request.source_bundle_size,
        "target_name": request.target_name,
        "target_uuid": request.target_uuid,
        "transaction": "not-performed",
        "transfer_attempt_id": request.transfer_attempt_id,
        "transfer_result_readback_invocations": 2,
    }
    if terminal != {"format": resolution_bindings.EVIDENCE_FORMAT, **required_terminal}:
        raise ValueError("prior-resolution-terminal-semantics-invalid")

    probe_bytes = (request.repository_root / PROBE_RELATIVE_PATH).read_bytes()
    if not probe_bytes or len(probe_bytes) > preflight_probe.MAX_SMALL_FILE_BYTES:
        raise ValueError("negative-preflight-probe-size-invalid")
    boot_id = upstream.network.evidence.get("prior_network_boot_id")
    if not isinstance(boot_id, str):
        raise ValueError("prior-network-boot-id-invalid")
    return PreflightBinding(
        evidence={
            "format": EVIDENCE_FORMAT,
            **identities,
            "prior_network_boot_id_sha256": hashlib.sha256(
                boot_id.encode("ascii")
            ).hexdigest(),
            "prior_network_manifest_sha256": (
                request.prior_network_manifest_sha256
            ),
            "prior_resolution_entries_verified": entries,
            "prior_resolution_evidence_sha256": hashlib.sha256(
                resolution_readbacks[0]
            ).hexdigest(),
            "prior_resolution_evidence_size": len(resolution_readbacks[0]),
            "prior_resolution_manifest_sha256": (
                request.prior_resolution_manifest_sha256
            ),
            "prior_resolution_outcome": "input-ready",
            "prior_transfer_manifest_sha256": (
                request.prior_transfer_manifest_sha256
            ),
            "repository_clean": True,
            "repository_head": request.expected_repository_head,
            "resolution_attempt_id": request.resolution_attempt_id,
            "source_bundle_sha256": request.source_bundle_sha256,
            "source_bundle_size": request.source_bundle_size,
            "transfer_attempt_id": request.transfer_attempt_id,
        },
        network=upstream.network,
        probe_bytes=probe_bytes,
        resolution_evidence_bytes=resolution_readbacks[0],
    )
