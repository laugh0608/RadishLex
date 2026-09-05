#!/usr/bin/env python3
from __future__ import annotations

import base64
import binascii
import hashlib
import json
import os
import plistlib
import stat
import uuid
from dataclasses import dataclass
from pathlib import Path
from typing import Protocol

import l6_upgrade_quiesced_case_contract as case_contract
import l6_upgrade_quiesced_registration_shell_bindings as shell_bindings
import l6_utm_clone_once as clone_control


EVIDENCE_FORMAT = "radishlex-linux-l6-upgrade-quiesced-clone-front-door-v3"
PREDECESSOR_EVIDENCE_FORMAT = (
    "radishlex-linux-l6-upgrade-quiesced-clone-front-door-v2"
)
SHELL_EVIDENCE_FORMAT = shell_bindings.SHELL_EVIDENCE_FORMAT
CONTROL_RELATIVE_PATH = Path(
    "scripts/linux-product/l6_upgrade_quiesced_clone_front_door.py"
)
ASSET_RETIREMENT_DELETE_FORMAT = (
    "radishlex-linux-l6-asset-retirement-delete-v1"
)
ASSET_RETIREMENT_DELETE_FILES = (
    "request.json",
    "binding-preflight.json",
    "evidence-anchors.json",
    "asset-identity-round-1.json",
    "predelete-utmctl-list.json",
    "handles-round-1.json",
    "handles-round-2.json",
    "asset-identity-round-2.json",
    "delete-01-command.json",
    "delete-01-post-list.json",
    "delete-01-package.json",
    "delete-02-command.json",
    "delete-02-post-list.json",
    "delete-02-package.json",
    "delete-03-command.json",
    "delete-03-post-list.json",
    "delete-03-package.json",
    "terminal.json",
)
PREDECESSOR_FAILURE_FILES = (
    "request.json",
    "binding-preflight.json",
    "source-snapshot-preflight.json",
    "registration-shell-preflight.json",
    "target-package-preclone.json",
    "source-and-shell-handles-preflight.json",
    "utmctl-list-preclone.json",
    "terminal.json",
)
DELETED_PACKAGE_LOCATIONS = (
    ("utm-documents", "RadishLex-Debian13-ARM64-L6-1ebbdab.utm"),
    ("utm-documents", "RadishLex-Debian13-ARM64-L6-823afca-repair.utm"),
    ("operator", "Debian13-ARM64-L6-512e8ab.utm"),
)
EXPECTED_DELETE_TERMINAL = {
    "asset_hash_rounds": 2,
    "automatic_retry": "not-performed",
    "automatic_rollback": "not-performed",
    "batch_id": "fifth-batch-v1",
    "clone": "not-performed",
    "delete_invocations": 3,
    "deleted_asset_ids": [
        "upgrade-rolled-back-1ebbdab",
        "repair-completed-823afca",
        "source-terminal-after-s2-512e8ab",
    ],
    "deleted_assets": 3,
    "format": ASSET_RETIREMENT_DELETE_FORMAT,
    "guest": "not-entered",
    "handle_rounds": 2,
    "inventory_queries": 4,
    "move": "not-performed",
    "outcome": "deleted",
    "reason": "fifth-batch-v1-deleted-and-verified",
    "start": "not-performed",
}
EXPECTED_PREDECESSOR_TERMINAL = {
    "automatic_delete": "not-performed",
    "automatic_retry": "not-performed",
    "automatic_rollback": "not-performed",
    "automatic_start": "not-performed",
    "clone_command_exit_code": None,
    "clone_command_timed_out": False,
    "clone_invocations": 0,
    "format": PREDECESSOR_EVIDENCE_FORMAT,
    "guest_exec": "not-performed",
    "input_transfer": "not-performed",
    "operation_id": "not-generated",
    "outcome": "precondition-rejected",
    "reason": "utmctl-list-preclone:preclone-vm-count-mismatch",
    "replacement_count": 0,
    "target_name": (
        "RadishLex-Debian13-ARM64-L6-d75818f-upgrade-quiesced"
    ),
    "target_package_state": "not-observed",
    "target_uuid": None,
    "transaction": "not-performed",
}


class CloneBindingError(ValueError):
    pass


class FrontDoorRequest(Protocol):
    repository_root: Path
    expected_repository_head: str
    operator_asset_root: Path
    utm_documents_root: Path
    asset_retirement_delete_root: Path
    asset_retirement_delete_manifest_sha256: str
    predecessor_failure_root: Path
    predecessor_failure_manifest_sha256: str
    source_snapshot_root: Path
    registration_shell_evidence_root: Path
    registration_shell_manifest_sha256: str
    registration_shell_uuid: str
    registration_shell_name: str
    registration_shell_package_path: Path
    target_name: str
    target_package_path: Path
    expected_vm_count: int
    expected_preclone_inventory_sha256: str


@dataclass(frozen=True)
class BundleIdentity:
    config_path: Path
    efi_path: Path
    qcow2_path: Path
    config_sha256: str
    efi_sha256: str
    qcow2_sha256: str
    qcow2_name: str
    name: str
    uuid: str
    network: object

    def as_json(self) -> dict[str, object]:
        return {
            "config_sha256": self.config_sha256,
            "efi_sha256": self.efi_sha256,
            "name": self.name,
            "network": self.network,
            "qcow2_name": self.qcow2_name,
            "qcow2_sha256": self.qcow2_sha256,
            "uuid": self.uuid,
        }


def validate_clone_front_door_bindings(
    request: FrontDoorRequest,
) -> dict[str, object]:
    head = _run_git(
        request.repository_root, ("rev-parse", "HEAD")
    ).decode("ascii").strip()
    if head != request.expected_repository_head:
        raise CloneBindingError("repository-head-drift")
    if _run_git(request.repository_root, ("status", "--porcelain")):
        raise CloneBindingError("repository-not-clean")
    case_contract.validate_repository_contract(request.repository_root)
    control_sha256 = _validate_control_identity(request.repository_root)
    retirement = validate_post_retirement_baseline(request)
    predecessor = validate_predecessor_failure(request)

    manifest_path = request.registration_shell_evidence_root / "files.sha256"
    if sha256_file(manifest_path) != request.registration_shell_manifest_sha256:
        raise CloneBindingError("registration-shell-manifest-drift")
    entries = clone_control._verify_sha256_manifest(
        request.registration_shell_evidence_root, manifest_path
    )
    manifest_lines = manifest_path.read_text(encoding="ascii").splitlines()
    if entries != 1 or not manifest_lines[0].endswith(
        "  registration-shell.json"
    ):
        raise CloneBindingError("registration-shell-manifest-members-invalid")
    evidence = _read_json(
        request.registration_shell_evidence_root / "registration-shell.json"
    )
    shell = _read_bundle(
        request.registration_shell_package_path, root_mode=0o755
    )
    expected = shell_bindings.expected_shell_evidence(request, shell)
    if evidence != expected:
        raise CloneBindingError(
            "dedicated-registration-shell-evidence-invalid"
        )
    return {
        "case_contract": "validated",
        "control_sha256": control_sha256,
        "dedicated_registration_shell": True,
        "format": EVIDENCE_FORMAT,
        **retirement,
        **predecessor,
        "registration_shell_entries_verified": entries,
        "registration_shell_manifest_sha256": (
            request.registration_shell_manifest_sha256
        ),
        "repository_clean": True,
        "repository_head": head,
    }


def validate_post_retirement_baseline(
    request: FrontDoorRequest,
) -> dict[str, object]:
    baseline = case_contract.EXPECTED_CLONE_FRONT_DOOR[
        "preclone_baseline"
    ]
    expected_root = request.operator_asset_root / str(
        baseline["delete_evidence_relative_path"]
    )
    if request.asset_retirement_delete_root != expected_root:
        raise CloneBindingError("asset-retirement-delete-root-drift")
    if request.asset_retirement_delete_manifest_sha256 != baseline[
        "delete_manifest_sha256"
    ]:
        raise CloneBindingError(
            "asset-retirement-delete-manifest-not-approved"
        )
    if request.expected_vm_count < baseline["registered_vm_count"]:
        raise CloneBindingError("live-inventory-smaller-than-managed-baseline")

    _validate_directory(
        request.operator_asset_root, 0o755, "operator-asset-root"
    )
    _validate_directory(
        request.utm_documents_root, 0o700, "utm-documents-root"
    )
    manifest = request.asset_retirement_delete_root / "files.sha256"
    if (
        sha256_file(manifest)
        != request.asset_retirement_delete_manifest_sha256
    ):
        raise CloneBindingError("asset-retirement-delete-manifest-drift")
    try:
        entries = clone_control._verify_sha256_manifest(
            request.asset_retirement_delete_root, manifest
        )
        actual_items = tuple(request.asset_retirement_delete_root.iterdir())
        actual_names = {item.name for item in actual_items}
    except (clone_control.CloneControlError, OSError) as exc:
        raise CloneBindingError(
            "asset-retirement-delete-evidence-invalid"
        ) from exc
    expected_names = set(ASSET_RETIREMENT_DELETE_FILES) | {"files.sha256"}
    if (
        entries != len(ASSET_RETIREMENT_DELETE_FILES)
        or actual_names != expected_names
    ):
        raise CloneBindingError("asset-retirement-delete-entry-set-drift")
    delete_stat = request.asset_retirement_delete_root.lstat()
    operator_stat = request.operator_asset_root.lstat()
    if (delete_stat.st_uid, delete_stat.st_gid) != (
        operator_stat.st_uid,
        operator_stat.st_gid,
    ):
        raise CloneBindingError("asset-retirement-delete-owner-drift")
    for item in actual_items:
        item_stat = item.lstat()
        if (item_stat.st_uid, item_stat.st_gid) != (
            delete_stat.st_uid,
            delete_stat.st_gid,
        ):
            raise CloneBindingError("asset-retirement-delete-entry-owner-drift")

    delete_request = _read_json(
        request.asset_retirement_delete_root / "request.json"
    )
    if (
        delete_request.get("format") != ASSET_RETIREMENT_DELETE_FORMAT
        or delete_request.get("batch_id") != baseline["delete_batch_id"]
        or delete_request.get("expected_repository_head")
        != baseline["delete_repository_head"]
        or delete_request.get("operator_asset_root_sha256")
        != sha256_text(str(request.operator_asset_root))
        or delete_request.get("utm_documents_root_sha256")
        != sha256_text(str(request.utm_documents_root))
    ):
        raise CloneBindingError("asset-retirement-delete-request-drift")
    terminal = _read_json(
        request.asset_retirement_delete_root / "terminal.json"
    )
    if terminal != EXPECTED_DELETE_TERMINAL:
        raise CloneBindingError("asset-retirement-delete-terminal-drift")
    for index in range(1, 4):
        package = _read_json(
            request.asset_retirement_delete_root
            / f"delete-{index:02d}-package.json"
        )
        if package != {
            "format": ASSET_RETIREMENT_DELETE_FORMAT,
            "state": "absent",
        }:
            raise CloneBindingError(
                "asset-retirement-delete-package-drift"
            )

    final_observation = _observation_from_json(
        request.asset_retirement_delete_root / "delete-03-post-list.json",
        ("utmctl", "list"),
    )
    try:
        registered = clone_control.parse_utmctl_list(final_observation)
    except clone_control.CloneControlError as exc:
        raise CloneBindingError(
            "asset-retirement-final-inventory-invalid"
        ) from exc
    if (
        len(registered) != baseline["registered_vm_count"]
        or any(item.status != "stopped" for item in registered)
        or clone_control.canonical_inventory_sha256(registered)
        != baseline["inventory_sha256"]
    ):
        raise CloneBindingError("asset-retirement-final-inventory-drift")
    managed, foreign = _classify_managed_inventory(registered)
    if len(managed) != baseline["registered_vm_count"] or foreign:
        raise CloneBindingError("asset-retirement-managed-inventory-drift")
    shell = [
        item
        for item in registered
        if item.uuid == request.registration_shell_uuid
        and item.name == request.registration_shell_name
    ]
    if len(shell) != 1:
        raise CloneBindingError("asset-retirement-final-shell-drift")

    for location, relative_path in DELETED_PACKAGE_LOCATIONS:
        root = (
            request.operator_asset_root
            if location == "operator"
            else request.utm_documents_root
        )
        _require_absent(root / relative_path)
    return {
        "asset_retirement_delete_entries_verified": entries,
        "asset_retirement_delete_manifest_sha256": (
            request.asset_retirement_delete_manifest_sha256
        ),
        "deleted_packages_absent": len(DELETED_PACKAGE_LOCATIONS),
        "post_retirement_inventory_sha256": baseline["inventory_sha256"],
        "post_retirement_vm_count": len(registered),
    }


def validate_predecessor_failure(
    request: FrontDoorRequest,
) -> dict[str, object]:
    expected = case_contract.EXPECTED_CLONE_FRONT_DOOR[
        "predecessor_failure"
    ]
    expected_root = request.operator_asset_root / str(
        expected["evidence_relative_path"]
    )
    if request.predecessor_failure_root != expected_root:
        raise CloneBindingError("predecessor-failure-root-drift")
    if request.predecessor_failure_manifest_sha256 != expected[
        "manifest_sha256"
    ]:
        raise CloneBindingError("predecessor-failure-manifest-not-approved")
    _validate_directory(
        request.predecessor_failure_root, 0o700, "predecessor-failure-root"
    )
    manifest = request.predecessor_failure_root / "files.sha256"
    if sha256_file(manifest) != request.predecessor_failure_manifest_sha256:
        raise CloneBindingError("predecessor-failure-manifest-drift")
    try:
        entries = clone_control._verify_sha256_manifest(
            request.predecessor_failure_root, manifest
        )
        actual_names = {
            item.name for item in request.predecessor_failure_root.iterdir()
        }
    except (clone_control.CloneControlError, OSError) as exc:
        raise CloneBindingError("predecessor-failure-evidence-invalid") from exc
    expected_names = set(PREDECESSOR_FAILURE_FILES) | {"files.sha256"}
    if (
        entries != len(PREDECESSOR_FAILURE_FILES)
        or actual_names != expected_names
    ):
        raise CloneBindingError("predecessor-failure-entry-set-drift")

    previous_request = _read_json(
        request.predecessor_failure_root / "request.json"
    )
    baseline = case_contract.EXPECTED_CLONE_FRONT_DOOR[
        "preclone_baseline"
    ]
    if (
        previous_request.get("format") != PREDECESSOR_EVIDENCE_FORMAT
        or previous_request.get("attempt_id") != expected["attempt_id"]
        or previous_request.get("expected_repository_head")
        != expected["repository_head"]
        or previous_request.get("expected_vm_count")
        != baseline["registered_vm_count"]
        or previous_request.get("expected_preclone_inventory_sha256")
        != baseline["inventory_sha256"]
        or previous_request.get("target_name") != request.target_name
        or previous_request.get("target_package_path_sha256")
        != sha256_text(str(request.target_package_path))
    ):
        raise CloneBindingError("predecessor-failure-request-drift")
    terminal = _read_json(
        request.predecessor_failure_root / "terminal.json"
    )
    if terminal != EXPECTED_PREDECESSOR_TERMINAL:
        raise CloneBindingError("predecessor-failure-terminal-drift")
    target = _read_json(
        request.predecessor_failure_root / "target-package-preclone.json"
    )
    if target != {
        "package_name": request.target_package_path.name,
        "package_path_sha256": sha256_text(str(request.target_package_path)),
        "state": "absent",
    }:
        raise CloneBindingError("predecessor-failure-target-drift")
    observation = _observation_from_json(
        request.predecessor_failure_root / "utmctl-list-preclone.json",
        ("utmctl", "list"),
        "predecessor-failure",
    )
    try:
        registered = clone_control.parse_utmctl_list(observation)
    except clone_control.CloneControlError as exc:
        raise CloneBindingError("predecessor-failure-inventory-invalid") from exc
    _, foreign = _classify_managed_inventory(registered)
    if (
        len(registered) != expected["registered_vm_count"]
        or clone_control.canonical_inventory_sha256(registered)
        != expected["inventory_sha256"]
        or len(foreign) != expected["foreign_vm_count"]
        or sum(item.status == "started" for item in foreign)
        != expected["started_foreign_vm_count"]
    ):
        raise CloneBindingError("predecessor-failure-inventory-drift")
    return {
        "predecessor_failure_entries_verified": entries,
        "predecessor_failure_foreign_vm_count": len(foreign),
        "predecessor_failure_manifest_sha256": (
            request.predecessor_failure_manifest_sha256
        ),
        "predecessor_failure_outcome": expected["outcome"],
    }


def validate_live_preclone_inventory(
    registered: tuple[clone_control.RegisteredVm, ...],
    request: FrontDoorRequest,
) -> dict[str, object]:
    policy = case_contract.EXPECTED_CLONE_FRONT_DOOR[
        "live_inventory_policy"
    ]
    if len(registered) != request.expected_vm_count:
        raise CloneBindingError("preclone-vm-count-mismatch")
    if any(item.status != "stopped" for item in registered):
        raise CloneBindingError("preclone-vm-not-all-stopped")
    if clone_control.canonical_inventory_sha256(registered) != (
        request.expected_preclone_inventory_sha256
    ):
        raise CloneBindingError("preclone-inventory-sha256-mismatch")
    managed, foreign = _classify_managed_inventory(registered)
    if foreign and not policy["foreign_overlay_allowed"]:
        raise CloneBindingError("preclone-foreign-overlay-not-allowed")
    if any(item.name == request.target_name for item in registered):
        raise CloneBindingError("target-name-already-registered")
    return {
        "all_registered_vms_stopped": True,
        "foreign_overlay_allowed": bool(policy["foreign_overlay_allowed"]),
        "foreign_vm_count": len(foreign),
        "inventory_sha256": request.expected_preclone_inventory_sha256,
        "managed_vm_count": len(managed),
        "registered_vm_count": len(registered),
    }


def _classify_managed_inventory(
    registered: tuple[clone_control.RegisteredVm, ...],
) -> tuple[
    tuple[clone_control.RegisteredVm, ...],
    tuple[clone_control.RegisteredVm, ...],
]:
    baseline = case_contract.EXPECTED_CLONE_FRONT_DOOR[
        "preclone_baseline"
    ]
    expected_members = tuple(
        clone_control.RegisteredVm(
            str(item["uuid"]), "stopped", str(item["name"])
        )
        for item in baseline["managed_members"]
    )
    actual_by_uuid = {item.uuid: item for item in registered}
    for expected in expected_members:
        if actual_by_uuid.get(expected.uuid) != expected:
            raise CloneBindingError("managed-preclone-member-drift")
        if sum(item.name == expected.name for item in registered) != 1:
            raise CloneBindingError("managed-preclone-name-not-unique")
    managed_uuids = {item.uuid for item in expected_members}
    foreign = tuple(
        item for item in registered if item.uuid not in managed_uuids
    )
    return expected_members, foreign


def validate_source_snapshot(request: FrontDoorRequest) -> BundleIdentity:
    identity = _read_bundle(request.source_snapshot_root, root_mode=0o700)
    expected = case_contract.EXPECTED_START
    if (
        identity.config_sha256 != expected["config_sha256"]
        or identity.efi_sha256 != expected["efi_sha256"]
        or identity.qcow2_sha256 != expected["qcow2_sha256"]
    ):
        raise CloneBindingError("source-snapshot-disk-identity-drift")
    evidence_path = (
        request.source_snapshot_root / "local-snapshot.evidence.json"
    )
    _validate_regular_file(
        evidence_path,
        0o600,
        str(expected["local_evidence_sha256"]),
        "source-local-evidence",
    )
    return identity


def validate_registration_shell(request: FrontDoorRequest) -> BundleIdentity:
    identity = _read_bundle(
        request.registration_shell_package_path, root_mode=0o755
    )
    if (
        identity.uuid != request.registration_shell_uuid
        or identity.name != request.registration_shell_name
        or identity.network != []
    ):
        raise CloneBindingError("registration-shell-runtime-identity-drift")
    return identity


def validate_target_bundle(
    request: FrontDoorRequest,
    registration: BundleIdentity,
    target_uuid: str,
    *,
    materialized: bool,
    source: BundleIdentity | None = None,
) -> BundleIdentity:
    identity = _read_bundle(request.target_package_path, root_mode=0o755)
    if identity.name != request.target_name or identity.uuid != target_uuid:
        raise CloneBindingError("target-name-or-uuid-drift")
    if identity.network != []:
        raise CloneBindingError("target-network-must-be-empty")
    if _normalized_plist(registration.config_path) != _normalized_plist(
        identity.config_path
    ):
        raise CloneBindingError(
            "target-config-differs-beyond-name-and-uuid"
        )
    expected_efi = (
        source.efi_sha256
        if materialized and source is not None
        else registration.efi_sha256
    )
    expected_qcow2 = (
        source.qcow2_sha256
        if materialized and source is not None
        else registration.qcow2_sha256
    )
    if (
        identity.efi_sha256 != expected_efi
        or identity.qcow2_sha256 != expected_qcow2
    ):
        raise CloneBindingError("target-materialization-state-mismatch")
    return identity


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    try:
        with path.open("rb") as source:
            while block := source.read(1024 * 1024):
                digest.update(block)
    except OSError as exc:
        raise CloneBindingError(f"sha256-unreadable:{path.name}") from exc
    return digest.hexdigest()


def sha256_text(value: str) -> str:
    return hashlib.sha256(value.encode("utf-8")).hexdigest()


def _read_bundle(root: Path, *, root_mode: int) -> BundleIdentity:
    _validate_directory(root, root_mode, "bundle-root")
    data = root / "Data"
    _validate_directory(data, root_mode, "bundle-data")
    config = root / "config.plist"
    _validate_regular_file(config, 0o644, None, "config")
    try:
        value = plistlib.loads(config.read_bytes())
    except (OSError, plistlib.InvalidFileException) as exc:
        raise CloneBindingError("config-plist-invalid") from exc
    if not isinstance(value, dict):
        raise CloneBindingError("config-plist-root-invalid")
    information = value.get("Information")
    drives = value.get("Drive")
    if not isinstance(information, dict) or not isinstance(drives, list):
        raise CloneBindingError("config-required-fields-invalid")
    disks = [
        item
        for item in drives
        if isinstance(item, dict) and item.get("ImageType") == "Disk"
    ]
    if len(disks) != 1 or disks[0].get("Interface") != "VirtIO":
        raise CloneBindingError("config-disk-contract-invalid")
    qcow2_name = disks[0].get("ImageName")
    if (
        not isinstance(qcow2_name, str)
        or Path(qcow2_name).name != qcow2_name
        or not qcow2_name.endswith(".qcow2")
    ):
        raise CloneBindingError("config-qcow2-name-invalid")
    name = information.get("Name")
    vm_uuid = information.get("UUID")
    if not isinstance(name, str) or not isinstance(vm_uuid, str):
        raise CloneBindingError("config-information-invalid")
    try:
        canonical_uuid = str(uuid.UUID(vm_uuid)).upper()
    except ValueError as exc:
        raise CloneBindingError("config-uuid-invalid") from exc
    if canonical_uuid != vm_uuid:
        raise CloneBindingError("config-uuid-not-canonical")
    efi = data / "efi_vars.fd"
    qcow2 = data / qcow2_name
    _validate_regular_file(efi, 0o644, None, "efi")
    _validate_regular_file(qcow2, 0o644, None, "qcow2")
    return BundleIdentity(
        config_path=config,
        efi_path=efi,
        qcow2_path=qcow2,
        config_sha256=sha256_file(config),
        efi_sha256=sha256_file(efi),
        qcow2_sha256=sha256_file(qcow2),
        qcow2_name=qcow2_name,
        name=name,
        uuid=vm_uuid,
        network=value.get("Network"),
    )


def _normalized_plist(path: Path) -> object:
    try:
        value = plistlib.loads(path.read_bytes())
    except (OSError, plistlib.InvalidFileException) as exc:
        raise CloneBindingError("config-plist-invalid") from exc
    if (
        not isinstance(value, dict)
        or not isinstance(value.get("Information"), dict)
    ):
        raise CloneBindingError("config-information-invalid")
    result = dict(value)
    information = dict(result["Information"])
    information.pop("Name", None)
    information.pop("UUID", None)
    result["Information"] = information
    return result


def _validate_directory(path: Path, mode: int, label: str) -> None:
    try:
        metadata = path.lstat()
    except OSError as exc:
        raise CloneBindingError(f"{label}-unavailable") from exc
    if (
        not stat.S_ISDIR(metadata.st_mode)
        or stat.S_ISLNK(metadata.st_mode)
        or stat.S_IMODE(metadata.st_mode) != mode
        or metadata.st_uid != os.getuid()
        or metadata.st_gid != os.getgid()
    ):
        raise CloneBindingError(f"{label}-identity-invalid")


def _validate_regular_file(
    path: Path,
    mode: int,
    expected_sha256: str | None,
    label: str,
) -> None:
    try:
        metadata = path.lstat()
    except OSError as exc:
        raise CloneBindingError(f"{label}-unavailable") from exc
    if (
        not stat.S_ISREG(metadata.st_mode)
        or stat.S_ISLNK(metadata.st_mode)
        or stat.S_IMODE(metadata.st_mode) != mode
        or metadata.st_uid != os.getuid()
        or metadata.st_gid != os.getgid()
        or metadata.st_nlink != 1
    ):
        raise CloneBindingError(f"{label}-identity-invalid")
    if expected_sha256 is not None and sha256_file(path) != expected_sha256:
        raise CloneBindingError(f"{label}-sha256-mismatch")


def _validate_control_identity(repository_root: Path) -> str:
    expected = repository_root / CONTROL_RELATIVE_PATH
    if Path(__file__).with_name(
        "l6_upgrade_quiesced_clone_front_door.py"
    ).absolute() != expected:
        raise CloneBindingError("executed-control-path-mismatch")
    try:
        metadata = expected.lstat()
    except OSError as exc:
        raise CloneBindingError("executed-control-unavailable") from exc
    if (
        not stat.S_ISREG(metadata.st_mode)
        or stat.S_ISLNK(metadata.st_mode)
        or metadata.st_nlink != 1
        or stat.S_IMODE(metadata.st_mode) & 0o022
    ):
        raise CloneBindingError("executed-control-identity-invalid")
    return sha256_file(expected)


def _run_git(repository_root: Path, arguments: tuple[str, ...]) -> bytes:
    observation = clone_control.SubprocessCommandRunner().run(
        ("/usr/bin/git", "-C", str(repository_root), *arguments), 30
    )
    try:
        clone_control._require_successful_observation(observation, "git")
    except clone_control.CloneControlError as exc:
        raise CloneBindingError(str(exc)) from exc
    return observation.stdout.prefix


def _observation_from_json(
    path: Path,
    expected_argv: tuple[str, ...],
    evidence_label: str = "asset-retirement",
) -> clone_control.CommandObservation:
    value = _read_json(path)
    if set(value) != {"argv", "exit_code", "timed_out", "stdout", "stderr"}:
        raise CloneBindingError(f"{evidence_label}-observation-keys-invalid")
    if value["argv"] != list(expected_argv):
        raise CloneBindingError(f"{evidence_label}-observation-argv-drift")
    stdout = _captured_bytes(value["stdout"], "stdout", evidence_label)
    stderr = _captured_bytes(value["stderr"], "stderr", evidence_label)
    observation = clone_control.CommandObservation.from_bytes(
        expected_argv,
        exit_code=value["exit_code"],
        timed_out=value["timed_out"],
        stdout=stdout,
        stderr=stderr,
    )
    if observation.as_json() != value:
        raise CloneBindingError(f"{evidence_label}-observation-content-drift")
    return observation


def _captured_bytes(
    value: object, label: str, evidence_label: str
) -> bytes:
    if not isinstance(value, dict):
        raise CloneBindingError(f"{evidence_label}-observation-{label}-invalid")
    encoded = value.get("prefix_base64")
    if not isinstance(encoded, str):
        raise CloneBindingError(
            f"{evidence_label}-observation-{label}-base64-invalid"
        )
    try:
        return base64.b64decode(encoded, validate=True)
    except (ValueError, binascii.Error) as exc:
        raise CloneBindingError(
            f"{evidence_label}-observation-{label}-base64-invalid"
        ) from exc


def _require_absent(path: Path) -> None:
    try:
        path.lstat()
    except FileNotFoundError:
        return
    except OSError as exc:
        raise CloneBindingError("retired-package-observation-failed") from exc
    raise CloneBindingError("retired-package-reappeared")


def _read_json(path: Path) -> dict[str, object]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as exc:
        raise CloneBindingError(f"json-invalid:{path.name}") from exc
    if not isinstance(value, dict):
        raise CloneBindingError(f"json-root-invalid:{path.name}")
    return value
