#!/usr/bin/env python3
from __future__ import annotations

import re
from pathlib import PurePosixPath
from typing import Protocol


PROJECTION_FORMAT = (
    "radishlex-linux-l6-asset-retirement-fifth-batch-projections-v1"
)
ASSET_IDS = (
    "upgrade-rolled-back-1ebbdab",
    "repair-completed-823afca",
    "source-terminal-after-s2-512e8ab",
)
S2_PREDECESSOR = {
    "storage": "operator",
    "relative_path": (
        "RadishLex-L6-Snapshots/S2-source-data-512e8ab/"
        "local-snapshot.evidence.json"
    ),
    "sha256": "d164de0f5e6e88afbfa76ccd1c7d321d7064bbfd3ee1e275d3d4842d0dfa5c74",
    "identity": "S2-source-data-512e8ab-0c2cefd6",
    "config_sha256": (
        "5111741c54a49068dbbacfd131891b0990a531001769db21942eb88ac6767d2b"
    ),
    "efi_sha256": (
        "365b5a170dca95bdf07c0e5e940fafd4580a353e43c141abede71c95f91261bc"
    ),
    "qcow2_sha256": (
        "0c2cefd6b63420adf143e1f1d6e4e70f59841bf1ba56cb54e86d2e7a226f3eea"
    ),
    "relation": "retained-predecessor-not-current-bundle-copy",
}
POLICY = {
    "current_bundle_identity": "direct-full-sha256-required-at-prepare",
    "historical_evidence_owner": "501:20-required",
    "nonconforming_historical_evidence": (
        "reference-only-never-runtime-anchor"
    ),
    "projection": "committed-minimal-facts-without-history-rewrite",
    "retained_snapshot_relation": (
        "predecessor-only-never-current-bundle-copy"
    ),
}
TERMINALS = {
    "upgrade-rolled-back-terminal-projected": {
        "operation": "upgrade",
        "relationship": "target_newer",
        "outcome": "rolled_back",
        "maintenance_invocations": 1,
        "package_effect": "target-installed-then-source-restored",
        "source_recovery": "completed",
        "retry": "not-performed",
        "terminal_vm_state": "stopped",
        "qcow_open_handles": 0,
    },
    "repair-completed-terminal-projected": {
        "operation": "repair",
        "relationship": "same_release",
        "outcome": "completed",
        "maintenance_invocations": 1,
        "package_effect": "same-release-package-reapplied",
        "operation_chain_length": 3,
        "retry": "not-performed",
        "terminal_vm_state": "stopped",
        "qcow_open_handles": 0,
    },
    "source-terminal-post-s2-failed-closed": {
        "operation": "upgrade",
        "relationship": "target_newer",
        "outcome": "failed-closed-before-receipt",
        "maintenance_invocations": 1,
        "package_effect": "not-started",
        "source_receipt": "completed-preserved",
        "retry": "not-performed",
        "terminal_vm_state": "stopped",
        "qcow_open_handles": 0,
    },
}
HEX_64 = re.compile(r"[0-9a-f]{64}")
SAFE_ID = re.compile(r"[a-z0-9][a-z0-9-]{0,95}")


class ProjectionValidationError(ValueError):
    pass


class EvidenceAnchorLike(Protocol):
    storage: str
    relative_path: str
    sha256: str
    mode: int
    semantic: str


class RetirementAssetLike(Protocol):
    asset_id: str
    relative_path: str
    name: str
    uuid: str
    role: str
    config_sha256: str
    efi_sha256: str
    qcow2_sha256: str
    qcow2_name: str
    evidence_anchors: tuple[EvidenceAnchorLike, ...]


def validate_terminal_projection(
    value: dict[str, object], asset: RetirementAssetLike
) -> None:
    _require_keys(
        value,
        {"format", "batch_id", "policy", "assets"},
        "fifth-batch-projection",
    )
    if (
        value["format"] != PROJECTION_FORMAT
        or value["batch_id"] != "fifth-batch-v1"
        or value["policy"] != POLICY
        or not isinstance(value["assets"], list)
        or len(value["assets"]) != len(ASSET_IDS)
    ):
        raise ProjectionValidationError("fifth-batch-projection-header-invalid")

    entries: dict[str, dict[str, object]] = {}
    for item in value["assets"]:
        _require_keys(
            item,
            {
                "asset_id",
                "name",
                "uuid",
                "role",
                "current_bundle_identity",
                "terminal",
                "historical_sources",
                "retained_predecessor",
            },
            "fifth-batch-projection-asset",
        )
        item_id = _require_safe_id(item["asset_id"], "projection-asset-id")
        if item_id in entries:
            raise ProjectionValidationError(
                "fifth-batch-projection-asset-duplicate"
            )
        entries[item_id] = item
    if tuple(entries) != ASSET_IDS:
        raise ProjectionValidationError("fifth-batch-projection-assets-invalid")

    entry = entries.get(asset.asset_id)
    expected_terminal = TERMINALS.get(asset.role)
    if (
        entry is None
        or expected_terminal is None
        or entry["name"] != asset.name
        or entry["uuid"] != asset.uuid
        or entry["role"] != asset.role
        or entry["terminal"] != expected_terminal
    ):
        raise ProjectionValidationError("fifth-batch-projection-terminal-invalid")

    identity = entry["current_bundle_identity"]
    _require_keys(
        identity,
        {"config_sha256", "efi_sha256", "qcow2_sha256"},
        "fifth-batch-projection-current-identity",
    )
    expected_identity = {
        "config_sha256": asset.config_sha256,
        "efi_sha256": asset.efi_sha256,
        "qcow2_sha256": asset.qcow2_sha256,
    }
    if identity != expected_identity:
        raise ProjectionValidationError(
            "fifth-batch-projection-identity-mismatch"
        )

    historical_sources = entry["historical_sources"]
    expected_source_count = 0 if asset.asset_id == ASSET_IDS[2] else 2
    if (
        not isinstance(historical_sources, list)
        or len(historical_sources) != expected_source_count
    ):
        raise ProjectionValidationError("fifth-batch-historical-sources-invalid")
    historical_paths: set[str] = set()
    for source in historical_sources:
        _require_keys(
            source,
            {
                "storage",
                "relative_path",
                "sha256",
                "observed_owner",
                "status",
            },
            "fifth-batch-historical-source",
        )
        relative_path = _require_relative(
            source["relative_path"], "fifth-batch-historical-source-path"
        )
        if (
            source["storage"] != "operator"
            or source["observed_owner"] != "501:0"
            or source["status"] != "reference-only-never-runtime-anchor"
            or relative_path in historical_paths
        ):
            raise ProjectionValidationError("fifth-batch-historical-source-invalid")
        _require_hash(source["sha256"], "fifth-batch-historical-source")
        historical_paths.add(relative_path)
        if any(
            anchor.storage == "operator"
            and anchor.relative_path == relative_path
            for anchor in asset.evidence_anchors
        ):
            raise ProjectionValidationError(
                "fifth-batch-nonconforming-source-used-as-runtime-anchor"
            )

    predecessor = entry["retained_predecessor"]
    if asset.asset_id != ASSET_IDS[2]:
        if predecessor is not None:
            raise ProjectionValidationError(
                "fifth-batch-unexpected-retained-predecessor"
            )
        return

    _require_keys(
        predecessor,
        set(S2_PREDECESSOR),
        "fifth-batch-retained-predecessor",
    )
    if (
        predecessor != S2_PREDECESSOR
        or predecessor["config_sha256"] != asset.config_sha256
        or predecessor["efi_sha256"] == asset.efi_sha256
        or predecessor["qcow2_sha256"] == asset.qcow2_sha256
    ):
        raise ProjectionValidationError("fifth-batch-predecessor-relation-invalid")
    matches = [
        anchor
        for anchor in asset.evidence_anchors
        if anchor.storage == "operator"
        and anchor.relative_path == predecessor["relative_path"]
        and anchor.sha256 == predecessor["sha256"]
        and anchor.mode == 0o600
        and anchor.semantic == "retained-predecessor-snapshot"
    ]
    if len(matches) != 1:
        raise ProjectionValidationError("fifth-batch-predecessor-anchor-invalid")


def validate_retained_predecessor_snapshot(
    value: dict[str, object], asset: RetirementAssetLike
) -> None:
    source = value.get("source_vm")
    snapshot = value.get("snapshot")
    if (
        asset.asset_id != ASSET_IDS[2]
        or asset.role != "source-terminal-post-s2-failed-closed"
        or value.get("format") != "radishlex-linux-l6-local-snapshot-evidence-v1"
        or not isinstance(source, dict)
        or not isinstance(snapshot, dict)
        or source.get("relative_path") != asset.relative_path
        or source.get("name") != asset.name
        or source.get("uuid") != asset.uuid
        or source.get("source_vm_stopped_after_verification") is not True
        or source.get("all_registered_vms_stopped_after_verification") is not True
        or source.get("registered_running_vm_count_after_verification") != 0
        or source.get("qcow_open_handles_after_verification") != 0
        or snapshot.get("id") != S2_PREDECESSOR["identity"]
        or snapshot.get("registered_with_utm") is not False
    ):
        raise ProjectionValidationError("retained-predecessor-snapshot-invalid")
    files = snapshot.get("restorable_files")
    if not isinstance(files, list):
        raise ProjectionValidationError(
            "retained-predecessor-snapshot-files-invalid"
        )
    actual = {
        item.get("path"): item.get("sha256")
        for item in files
        if isinstance(item, dict)
    }
    expected = {
        "config.plist": S2_PREDECESSOR["config_sha256"],
        "Data/efi_vars.fd": S2_PREDECESSOR["efi_sha256"],
        f"Data/{asset.qcow2_name}": S2_PREDECESSOR["qcow2_sha256"],
    }
    if (
        actual != expected
        or actual["config.plist"] != asset.config_sha256
        or actual["Data/efi_vars.fd"] == asset.efi_sha256
        or actual[f"Data/{asset.qcow2_name}"] == asset.qcow2_sha256
    ):
        raise ProjectionValidationError(
            "retained-predecessor-snapshot-current-identity-confused"
        )


def _require_keys(value: object, expected: set[str], label: str) -> None:
    if not isinstance(value, dict) or set(value) != expected:
        raise ProjectionValidationError(f"{label}-keys-invalid")


def _require_safe_id(value: object, label: str) -> str:
    if not isinstance(value, str) or not SAFE_ID.fullmatch(value):
        raise ProjectionValidationError(f"{label}-invalid")
    return value


def _require_relative(value: object, label: str) -> str:
    if not isinstance(value, str) or not value or "\\" in value or "\x00" in value:
        raise ProjectionValidationError(f"{label}-invalid")
    path = PurePosixPath(value)
    if path.is_absolute() or any(part in ("", ".", "..") for part in path.parts):
        raise ProjectionValidationError(f"{label}-invalid")
    return value


def _require_hash(value: object, label: str) -> str:
    if not isinstance(value, str) or not HEX_64.fullmatch(value):
        raise ProjectionValidationError(f"{label}-sha256-invalid")
    return value
