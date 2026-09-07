#!/usr/bin/env python3
from __future__ import annotations

import hashlib
from dataclasses import dataclass
from pathlib import Path
from typing import Protocol

import l6_utm_canonical_input_bindings as transfer_bindings
import l6_utm_canonical_input_preflight as prior_preflight
import l6_utm_canonical_input_preflight_bindings as prior_bindings
import l6_utm_guest_network_ready as network_ready
import l6_utm_start_once as start_control
import l6_v4_canonical_input_preflight_probe as preflight_probe
import l6_v4_install_artifacts_staged_checkpoint_driver as checkpoint_driver


EVIDENCE_FORMAT = "radishlex-linux-l6-utm-install-artifacts-staged-checkpoint-v1"
CONTROL_RELATIVE_PATH = Path(
    "scripts/linux-product/l6_utm_install_artifacts_staged_checkpoint.py"
)
BINDINGS_RELATIVE_PATH = Path(
    "scripts/linux-product/l6_utm_install_artifacts_staged_checkpoint_bindings.py"
)
DRIVER_RELATIVE_PATH = Path(
    "scripts/linux-product/l6_v4_install_artifacts_staged_checkpoint_driver.py"
)
EVIDENCE_RELATIVE_PATH = Path(
    "scripts/linux-product/l6_utm_install_artifacts_staged_checkpoint_evidence.py"
)
REQUIRED_PRIOR_PREFLIGHT_MANIFEST_SHA256 = (
    "aba59811e62a2f16f149f8fffc71c9f57f823561afe053babd21dd903c360d7a"
)
REQUIRED_PRIOR_PREFLIGHT_REPOSITORY_HEAD = (
    "8bd9e65c18fd9d2edce5c5c0a78d148943aea8e4"
)
REQUIRED_PREFLIGHT_ATTEMPT_ID = (
    "d75818f-v4-negative-preflight-20260823-v1"
)
REQUIRED_CHECKPOINT_ATTEMPT_ID = (
    "d75818f-v4-install-artifacts-staged-checkpoint-20260823-v1"
)
REQUIRED_PREFLIGHT_ENTRY_NAMES = (
    "request.json",
    "binding-preflight.json",
    "source-bundle-preflight.json",
    "target-files-preflight.json",
    "host-process-preflight.json",
    "target-handles-preflight.json",
    "network-evidence-live-readback-1.json",
    "network-evidence-live-readback-2.json",
    "existing-resolution-evidence-readback-1.json",
    "existing-resolution-evidence-readback-2.json",
    "guest-preflight-root-create.json",
    "guest-preflight-probe-push.json",
    "guest-preflight-probe-chown.json",
    "guest-preflight-probe-chmod.json",
    "guest-preflight-probe-publish.json",
    "guest-preflight-probe-readback.json",
    "guest-negative-preflight.json",
    "guest-preflight-marker-readback.json",
    "guest-negative-preflight-evidence-readback-1.json",
    "guest-negative-preflight-evidence-readback-2.json",
    "guest-negative-preflight-evidence.json",
    "guest-preflight-phase-readback.json",
    "network-evidence-postflight-readback.json",
    "source-bundle-postflight.json",
    "terminal.json",
)


class CheckpointBindingRequest(Protocol):
    repository_root: Path
    expected_repository_head: str
    prior_network_root: Path
    prior_network_manifest_sha256: str
    prior_transfer_root: Path
    prior_transfer_manifest_sha256: str
    prior_resolution_root: Path
    prior_resolution_manifest_sha256: str
    prior_preflight_root: Path
    prior_preflight_manifest_sha256: str
    source_bundle_path: Path
    source_bundle_size: int
    source_bundle_sha256: str
    transfer_attempt_id: str
    resolution_attempt_id: str
    preflight_attempt_id: str
    checkpoint_attempt_id: str
    target_uuid: str
    target_name: str
    target_package_path: Path


@dataclass(frozen=True)
class CheckpointBinding:
    evidence: dict[str, object]
    network: transfer_bindings.NetworkBinding
    driver_bytes: bytes
    preflight_evidence_bytes: bytes
    preflight_evidence_path: str
    boot_id_sha256: str


def validate_checkpoint_bindings(
    request: CheckpointBindingRequest,
) -> CheckpointBinding:
    upstream = prior_bindings.validate_preflight_bindings(request)
    identities: dict[str, object] = {}
    for relative_path, label in (
        (CONTROL_RELATIVE_PATH, "control"),
        (BINDINGS_RELATIVE_PATH, "bindings"),
        (EVIDENCE_RELATIVE_PATH, "evidence"),
        (DRIVER_RELATIVE_PATH, "driver"),
    ):
        path = request.repository_root / relative_path
        network_ready._require_committed_regular(path, label)
        identities[f"{label}_sha256"] = network_ready._sha256_file(path)

    manifest = request.prior_preflight_root / "files.sha256"
    if (
        request.prior_preflight_manifest_sha256
        != REQUIRED_PRIOR_PREFLIGHT_MANIFEST_SHA256
        or network_ready._sha256_file(manifest)
        != REQUIRED_PRIOR_PREFLIGHT_MANIFEST_SHA256
    ):
        raise ValueError("prior-preflight-manifest-drift")
    entries = start_control._verify_sha256_manifest(
        request.prior_preflight_root, manifest
    )
    if entries != len(REQUIRED_PREFLIGHT_ENTRY_NAMES):
        raise ValueError("prior-preflight-entry-count-invalid")
    if _manifest_names(manifest) != REQUIRED_PREFLIGHT_ENTRY_NAMES:
        raise ValueError("prior-preflight-entry-names-invalid")

    prior_request = network_ready._read_json(
        request.prior_preflight_root / "request.json"
    )
    expected_authorization = {
        "canonical_input_negative_preflight": True,
        "create_new_guest_preflight_root": True,
        "existing_resolution_double_readback": True,
        "read_only_startup_without_case_maintenance_acceptance_or_dpkg": True,
    }
    if (
        prior_request.get("format") != prior_preflight.EVIDENCE_FORMAT
        or prior_request.get("expected_repository_head")
        != REQUIRED_PRIOR_PREFLIGHT_REPOSITORY_HEAD
        or prior_request.get("authorization") != expected_authorization
        or prior_request.get("preflight_attempt_id")
        != request.preflight_attempt_id
        or prior_request.get("resolution_attempt_id")
        != request.resolution_attempt_id
        or prior_request.get("transfer_attempt_id")
        != request.transfer_attempt_id
        or prior_request.get("prior_network_manifest_sha256")
        != request.prior_network_manifest_sha256
        or prior_request.get("prior_resolution_manifest_sha256")
        != request.prior_resolution_manifest_sha256
        or prior_request.get("prior_transfer_manifest_sha256")
        != request.prior_transfer_manifest_sha256
        or prior_request.get("source_bundle_size")
        != request.source_bundle_size
        or prior_request.get("source_bundle_sha256")
        != request.source_bundle_sha256
        or prior_request.get("target_uuid") != request.target_uuid
        or prior_request.get("target_name") != request.target_name
    ):
        raise ValueError("prior-preflight-request-semantics-invalid")
    expected_guest_root = (
        "/var/tmp/radishlex-l6-v4-negative-preflight-"
        f"{request.preflight_attempt_id}"
    )
    if prior_request.get("guest_preflight_root") != expected_guest_root:
        raise ValueError("prior-preflight-guest-root-invalid")

    evidence_readbacks = tuple(
        transfer_bindings._prior_readback_bytes(
            request.prior_preflight_root
            / f"guest-negative-preflight-evidence-readback-{index}.json"
        )
        for index in (1, 2)
    )
    if evidence_readbacks[0] != evidence_readbacks[1]:
        raise ValueError("prior-preflight-evidence-readback-drift")
    boot_id_sha256 = upstream.evidence.get("prior_network_boot_id_sha256")
    if not isinstance(boot_id_sha256, str):
        raise ValueError("prior-preflight-boot-id-invalid")
    parsed = prior_preflight.parse_preflight_evidence(
        evidence_readbacks[0], request, boot_id_sha256
    )
    persisted = network_ready._read_json(
        request.prior_preflight_root / "guest-negative-preflight-evidence.json"
    )
    required_guest = {
        "checkpoint_evidence": "absent",
        "dependency_count": 20,
        "dependency_identity": "matched",
        "guard": "absent",
        "input_inventory_count": len(preflight_probe.INPUT_MEMBERS),
        "input_inventory_identity": "matched",
        "input_root_identity": "private-directory",
        "network": "loopback-only-main-routes-empty",
        "operation_id": "not-generated",
        "outcome": "passed",
        "package": "not-installed",
        "phase": "complete",
        "product_processes": "absent",
        "reason": "none",
        "receipt_terminal": "absent",
        "release_pair_identity": "matched",
        "startup_reason": "ReceiptMissing",
        "state_root": "absent",
        "transaction": "not-performed",
        "user_xdg": "absent",
    }
    if parsed != persisted or any(
        parsed.get(key) != expected for key, expected in required_guest.items()
    ):
        raise ValueError("prior-preflight-evidence-semantics-invalid")

    marker = transfer_bindings._prior_readback_bytes(
        request.prior_preflight_root / "guest-preflight-marker-readback.json"
    )
    expected_marker = preflight_probe.expected_marker_bytes(
        request.preflight_attempt_id,
        hashlib.sha256(upstream.probe_bytes).hexdigest(),
    )
    if marker != expected_marker:
        raise ValueError("prior-preflight-marker-invalid")
    phase = transfer_bindings._prior_readback_bytes(
        request.prior_preflight_root / "guest-preflight-phase-readback.json"
    )
    if phase != preflight_probe.canonical_json(
        {"format": preflight_probe.EVIDENCE_FORMAT, "phase": "complete"}
    ):
        raise ValueError("prior-preflight-phase-invalid")

    terminal = network_ready._read_json(
        request.prior_preflight_root / "terminal.json"
    )
    required_terminal = {
        "acceptance_invocations": 0,
        "automatic_cleanup": "not-performed",
        "automatic_quit": "not-performed",
        "automatic_retry": "not-performed",
        "automatic_stop": "not-performed",
        "case_invocations": 0,
        "dpkg_invocations": 0,
        "file_pull_invocations": 10,
        "file_push_invocations": 1,
        "guest_exec_invocations": 5,
        "guest_preflight_outcome": "passed",
        "installer_invocations": 0,
        "maintenance_invocations": 0,
        "operation_id": "not-generated",
        "outcome": "preflight-ready",
        "plain_utmctl_list": "not-performed",
        "plain_utmctl_start": "not-performed",
        "plain_utmctl_status": "not-performed",
        "preflight_attempt_id": request.preflight_attempt_id,
        "preflight_invocations": 1,
        "reason": "one-shot-negative-preflight-passed",
        "resolution_attempt_id": request.resolution_attempt_id,
        "source_bundle_sha256": request.source_bundle_sha256,
        "source_bundle_size": request.source_bundle_size,
        "target_name": request.target_name,
        "target_uuid": request.target_uuid,
        "transaction": "not-performed",
        "transfer_attempt_id": request.transfer_attempt_id,
    }
    if terminal != {"format": prior_preflight.EVIDENCE_FORMAT, **required_terminal}:
        raise ValueError("prior-preflight-terminal-semantics-invalid")

    driver_bytes = (request.repository_root / DRIVER_RELATIVE_PATH).read_bytes()
    if not driver_bytes or len(driver_bytes) > checkpoint_driver.MAX_DRIVER_BYTES:
        raise ValueError("checkpoint-driver-size-invalid")
    return CheckpointBinding(
        evidence={
            "format": EVIDENCE_FORMAT,
            **identities,
            "checkpoint_attempt_id": request.checkpoint_attempt_id,
            "preflight_attempt_id": request.preflight_attempt_id,
            "prior_network_boot_id_sha256": boot_id_sha256,
            "prior_preflight_entries_verified": entries,
            "prior_preflight_evidence_sha256": hashlib.sha256(
                evidence_readbacks[0]
            ).hexdigest(),
            "prior_preflight_evidence_size": len(evidence_readbacks[0]),
            "prior_preflight_manifest_sha256": (
                request.prior_preflight_manifest_sha256
            ),
            "prior_preflight_outcome": "preflight-ready",
            "repository_clean": True,
            "repository_head": request.expected_repository_head,
            "source_bundle_sha256": request.source_bundle_sha256,
            "source_bundle_size": request.source_bundle_size,
            "target_uuid": request.target_uuid,
        },
        network=upstream.network,
        driver_bytes=driver_bytes,
        preflight_evidence_bytes=evidence_readbacks[0],
        preflight_evidence_path=f"{expected_guest_root}/negative-preflight.evidence.json",
        boot_id_sha256=boot_id_sha256,
    )


def _manifest_names(path: Path) -> tuple[str, ...]:
    names: list[str] = []
    for line in path.read_text(encoding="ascii").splitlines():
        parts = line.split("  ", 1)
        if len(parts) != 2:
            raise ValueError("prior-preflight-manifest-line-invalid")
        names.append(parts[1])
    return tuple(names)
