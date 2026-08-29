#!/usr/bin/env python3
from __future__ import annotations

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
import l6_utm_clone_once as clone_control


EVIDENCE_FORMAT = "radishlex-linux-l6-upgrade-quiesced-clone-front-door-v1"
SHELL_EVIDENCE_FORMAT = (
    "radishlex-linux-l6-upgrade-quiesced-dedicated-registration-shell-v1"
)
CONTROL_RELATIVE_PATH = Path(
    "scripts/linux-product/l6_upgrade_quiesced_clone_front_door.py"
)


class CloneBindingError(ValueError):
    pass


class FrontDoorRequest(Protocol):
    repository_root: Path
    expected_repository_head: str
    source_snapshot_root: Path
    registration_shell_evidence_root: Path
    registration_shell_manifest_sha256: str
    registration_shell_uuid: str
    registration_shell_name: str
    registration_shell_package_path: Path
    target_name: str
    target_package_path: Path


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
    expected = {
        "case_profile": "debian13-arm64-upgrade-quiesced-crash-v1",
        "config_sha256": shell.config_sha256,
        "dedicated_to_case": True,
        "efi_sha256": shell.efi_sha256,
        "format": SHELL_EVIDENCE_FORMAT,
        "guest_state_reuse": False,
        "network": [],
        "package_path_sha256": sha256_text(
            str(request.registration_shell_package_path)
        ),
        "qcow2_name": shell.qcow2_name,
        "qcow2_sha256": shell.qcow2_sha256,
        "source_terminal_reuse": False,
        "vm_name": request.registration_shell_name,
        "vm_uuid": request.registration_shell_uuid,
    }
    if evidence != expected:
        raise CloneBindingError(
            "dedicated-registration-shell-evidence-invalid"
        )
    return {
        "case_contract": "validated",
        "control_sha256": control_sha256,
        "dedicated_registration_shell": True,
        "format": EVIDENCE_FORMAT,
        "registration_shell_entries_verified": entries,
        "registration_shell_manifest_sha256": (
            request.registration_shell_manifest_sha256
        ),
        "repository_clean": True,
        "repository_head": head,
    }


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


def _read_json(path: Path) -> dict[str, object]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as exc:
        raise CloneBindingError(f"json-invalid:{path.name}") from exc
    if not isinstance(value, dict):
        raise CloneBindingError(f"json-root-invalid:{path.name}")
    return value
