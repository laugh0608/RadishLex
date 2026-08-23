#!/usr/bin/env python3
from __future__ import annotations

import hashlib
import os
import plistlib
import pwd
import re
import stat
from pathlib import Path
from typing import Protocol

import l6_utm_start_once as start_control


EVIDENCE_FORMAT = "radishlex-linux-l6-utm-launch-transport-v2"
PREPARED_EVIDENCE_FORMAT = (
    "radishlex-linux-l6-dependency-frozen-clone-prepared-v1"
)
PREPARED_PREFLIGHT_FORMAT = (
    "radishlex-linux-l6-dependency-frozen-materialization-preflight-v1"
)
REQUIRED_PREPARED_MANIFEST_SHA256 = (
    "c40c55d9fa2c62d5ceabd6894612a31d4ea9eb18666d808602d515d5f867144b"
)
REQUIRED_CLONE_MANIFEST_SHA256 = (
    "f76d1943e4af0f21f2fa9f93481b4d7e77112fb9f14bb64400f71f9457cff6e2"
)
REQUIRED_PREPARED_REPOSITORY_HEAD = (
    "e4ca1bd0fbad38d1813643a31a606ffd1b3c0fd9"
)
REQUIRED_TARGET_UUID = "50B75F88-493D-42C0-A1DC-054DEC478038"
REQUIRED_TARGET_NAME = (
    "RadishLex-Debian13-ARM64-L6-d75818f-"
    "crash-install-artifacts-staged-v4"
)
TARGET_DISK_IDENTIFIER = "FFF05A20-E829-493C-8F40-B40884425A3F"
TARGET_DISK_IMAGE_NAME = f"{TARGET_DISK_IDENTIFIER}.qcow2"
UTM_DOCUMENTS_RELATIVE_PATH = Path(
    "Library/Containers/com.utmapp.UTM/Data/Documents"
)
HEX_64 = re.compile(r"[0-9a-f]{64}")
KEY_NAME = re.compile(r"[a-z][a-z0-9_]*")


class PreparedBindingError(ValueError):
    pass


class PreparedBindingRequest(Protocol):
    prior_prepared_root: Path
    prior_prepared_manifest_sha256: str
    target_package_path: Path
    target_uuid: str
    target_name: str
    expected_vm_count: int


def validate_prepared_evidence(
    request: PreparedBindingRequest,
    v7_identity: dict[str, object],
) -> dict[str, object]:
    entries = _verify_bound_manifest(
        request.prior_prepared_root,
        request.prior_prepared_manifest_sha256,
        "prior-prepared",
    )
    expected_entries = frozenset(
        (
            "clone-once-manifest-check.evidence.txt",
            "materialize-control.sh",
            "postverify.evidence.txt",
            "preflight.evidence.txt",
            "registered-vms.postmaterialize.evidence.txt",
            "registered-vms.prematerialize.evidence.txt",
            "target-config.plist",
        )
    )
    if entries != expected_entries:
        raise PreparedBindingError("prior-prepared-manifest-entry-set-invalid")

    preflight = _read_key_value_evidence(
        request.prior_prepared_root / "preflight.evidence.txt",
        "prior-prepared-preflight",
    )
    postverify = _read_key_value_evidence(
        request.prior_prepared_root / "postverify.evidence.txt",
        "prior-prepared-postverify",
    )
    _require_exact_text_fields(
        preflight,
        {
            "clone_manifest_sha256": REQUIRED_CLONE_MANIFEST_SHA256,
            "format": PREPARED_PREFLIGHT_FORMAT,
            "materialization_authorized": "true",
            "no_automatic_start_retry_delete_authorized": "true",
            "preflight": "passed",
            "registered_vm_count": "21",
            "registered_vms": "all-stopped",
            "repository_head": REQUIRED_PREPARED_REPOSITORY_HEAD,
            "source_registration_target_handles": "0",
            "target_name": request.target_name,
            "target_network": "[]",
            "target_uuid": request.target_uuid,
        },
        {
            "clone_manifest_sha256",
            "control_script_sha256",
            "format",
            "materialization_authorized",
            "no_automatic_start_retry_delete_authorized",
            "preflight",
            "registered_vm_count",
            "registered_vms",
            "registered_vms_sha256",
            "repository_head",
            "source_config_sha256",
            "source_efi_sha256",
            "source_qcow2_sha256",
            "source_registration_target_handles",
            "target_config_sha256",
            "target_efi_before_sha256",
            "target_name",
            "target_network",
            "target_qcow2_before_sha256",
            "target_uuid",
        },
        "prior-prepared-preflight",
    )
    _require_exact_text_fields(
        postverify,
        {
            "automatic_delete": "not-performed",
            "automatic_retry": "not-performed",
            "automatic_start": "not-performed",
            "clone_manifest_sha256": REQUIRED_CLONE_MANIFEST_SHA256,
            "format": PREPARED_EVIDENCE_FORMAT,
            "guest_exec": "not-performed",
            "input_transfer": "not-performed",
            "materialization_authorized": "true",
            "no_automatic_start_retry_delete_authorized": "true",
            "operation_id": "not-generated",
            "postverify": "passed",
            "registered_vm_count": "21",
            "registered_vms": "all-stopped",
            "replacement_count": "2",
            "repository_head": REQUIRED_PREPARED_REPOSITORY_HEAD,
            "source_registration_target_handles": "0",
            "target_name": request.target_name,
            "target_network": "[]",
            "target_uuid": request.target_uuid,
            "transaction": "not-performed",
        },
        {
            "automatic_delete",
            "automatic_retry",
            "automatic_start",
            "clone_manifest_sha256",
            "control_script_sha256",
            "format",
            "guest_exec",
            "input_transfer",
            "materialization_authorized",
            "no_automatic_start_retry_delete_authorized",
            "operation_id",
            "postverify",
            "registered_vm_count",
            "registered_vms",
            "registered_vms_sha256",
            "replacement_count",
            "repository_head",
            "source_config_sha256",
            "source_efi_sha256",
            "source_qcow2_sha256",
            "source_registration_target_handles",
            "target_config_sha256",
            "target_efi_sha256",
            "target_name",
            "target_network",
            "target_qcow2_second_sha256",
            "target_qcow2_sha256",
            "target_uuid",
            "transaction",
        },
        "prior-prepared-postverify",
    )
    for key in (
        "control_script_sha256",
        "registered_vms_sha256",
        "source_config_sha256",
        "source_efi_sha256",
        "source_qcow2_sha256",
        "target_config_sha256",
        "target_efi_before_sha256",
        "target_qcow2_before_sha256",
    ):
        _require_sha256_text_field(preflight, key, "prior-prepared-preflight")
    for key in (
        "control_script_sha256",
        "registered_vms_sha256",
        "source_config_sha256",
        "source_efi_sha256",
        "source_qcow2_sha256",
        "target_config_sha256",
        "target_efi_sha256",
        "target_qcow2_second_sha256",
        "target_qcow2_sha256",
    ):
        _require_sha256_text_field(postverify, key, "prior-prepared-postverify")

    control_sha256 = _sha256_file(
        request.prior_prepared_root / "materialize-control.sh"
    )
    if (
        preflight["control_script_sha256"] != control_sha256
        or postverify["control_script_sha256"] != control_sha256
    ):
        raise PreparedBindingError("prior-prepared-control-identity-drift")
    if any(
        preflight[key] != postverify[key]
        for key in (
            "clone_manifest_sha256",
            "control_script_sha256",
            "registered_vms_sha256",
            "repository_head",
            "source_config_sha256",
            "source_efi_sha256",
            "source_qcow2_sha256",
            "target_config_sha256",
            "target_name",
            "target_uuid",
        )
    ):
        raise PreparedBindingError("prior-prepared-pre-post-identity-drift")
    if (
        postverify["target_efi_sha256"] != postverify["source_efi_sha256"]
        or postverify["target_qcow2_sha256"]
        != postverify["source_qcow2_sha256"]
        or postverify["target_qcow2_second_sha256"]
        != postverify["target_qcow2_sha256"]
    ):
        raise PreparedBindingError(
            "prior-prepared-target-source-identity-mismatch"
        )

    clone_check = _read_ascii_lines(
        request.prior_prepared_root
        / "clone-once-manifest-check.evidence.txt",
        "prior-prepared-clone-check",
    )
    expected_clone_check = [
        f"{name}: OK"
        for name in (
            "request.json",
            "binding-preflight.json",
            "target-package-preclone.json",
            "utmctl-list-preclone.json",
            "utmctl-clone.json",
            "utmctl-list-terminal.json",
            "target-package-terminal.json",
            "terminal.json",
        )
    ]
    if clone_check != expected_clone_check:
        raise PreparedBindingError("prior-prepared-clone-check-invalid")

    pre_list_path = (
        request.prior_prepared_root
        / "registered-vms.prematerialize.evidence.txt"
    )
    post_list_path = (
        request.prior_prepared_root
        / "registered-vms.postmaterialize.evidence.txt"
    )
    try:
        pre_list_bytes = pre_list_path.read_bytes()
        post_list_bytes = post_list_path.read_bytes()
    except OSError as exc:
        raise PreparedBindingError(
            "prior-prepared-inventory-unreadable"
        ) from exc
    registered_sha256 = hashlib.sha256(pre_list_bytes).hexdigest()
    if (
        pre_list_bytes != post_list_bytes
        or registered_sha256 != preflight["registered_vms_sha256"]
        or registered_sha256 != postverify["registered_vms_sha256"]
    ):
        raise PreparedBindingError("prior-prepared-inventory-drift")
    prepared_inventory = _parse_registered_inventory(
        pre_list_bytes, "prior-prepared-inventory"
    )
    _validate_prepared_inventory(prepared_inventory, request, v7_identity)

    try:
        config_payload = (
            request.prior_prepared_root / "target-config.plist"
        ).read_bytes()
    except OSError as exc:
        raise PreparedBindingError(
            "prior-prepared-target-config-unreadable"
        ) from exc
    if hashlib.sha256(config_payload).hexdigest() != postverify[
        "target_config_sha256"
    ]:
        raise PreparedBindingError("prior-prepared-target-config-drift")
    _validate_target_config_payload(
        config_payload,
        request.target_uuid,
        request.target_name,
        "prior-prepared-target-config",
    )

    return {
        "prepared_inventory_count": len(prepared_inventory),
        "prepared_registered_vms_sha256": registered_sha256,
        "prepared_target_config_sha256": postverify[
            "target_config_sha256"
        ],
        "prepared_target_efi_sha256": postverify["target_efi_sha256"],
        "prepared_target_qcow2_sha256": postverify[
            "target_qcow2_sha256"
        ],
        "prior_prepared_entries_verified": len(entries),
        "prior_prepared_manifest_sha256": (
            request.prior_prepared_manifest_sha256
        ),
        "prior_prepared_outcome": "passed",
    }


def expected_target_package_path(target_name: str) -> Path:
    try:
        home = Path(pwd.getpwuid(os.geteuid()).pw_dir)
    except (KeyError, OSError) as exc:
        raise PreparedBindingError("authoritative-home-unavailable") from exc
    if not home.is_absolute() or ".." in home.parts:
        raise PreparedBindingError("authoritative-home-invalid")
    return home / UTM_DOCUMENTS_RELATIVE_PATH / f"{target_name}.utm"


def validate_live_target_identity(
    request: PreparedBindingRequest,
    prepared_binding: dict[str, object],
) -> dict[str, object]:
    expected_path = expected_target_package_path(request.target_name)
    if request.target_package_path != expected_path:
        raise PreparedBindingError("target-package-path-mismatch")
    expected_hashes: dict[str, str] = {}
    for key in (
        "prepared_target_config_sha256",
        "prepared_target_efi_sha256",
        "prepared_target_qcow2_sha256",
    ):
        value = prepared_binding.get(key)
        if not isinstance(value, str) or not HEX_64.fullmatch(value):
            raise PreparedBindingError("prepared-target-identity-invalid")
        expected_hashes[key] = value

    package_fd = _open_verified_directory(
        request.target_package_path,
        None,
        "target-package",
        expected_mode=0o755,
    )
    try:
        package_stat = os.fstat(package_fd)
        config_identity, config_payload = _inspect_regular_file_at(
            package_fd,
            "config.plist",
            "target-config",
            expected_hashes["prepared_target_config_sha256"],
            package_stat,
            capture_payload=True,
        )
        data_fd = _open_verified_directory(
            Path("Data"),
            package_fd,
            "target-data",
            expected_mode=0o755,
            expected_owner=package_stat,
        )
        try:
            data_stat = os.fstat(data_fd)
            efi_identity, _ = _inspect_regular_file_at(
                data_fd,
                "efi_vars.fd",
                "target-efi",
                expected_hashes["prepared_target_efi_sha256"],
                package_stat,
            )
            qcow2_identity, _ = _inspect_regular_file_at(
                data_fd,
                TARGET_DISK_IMAGE_NAME,
                "target-qcow2",
                expected_hashes["prepared_target_qcow2_sha256"],
                package_stat,
            )
            data_after = os.fstat(data_fd)
            if _directory_identity(data_stat) != _directory_identity(
                data_after
            ):
                raise PreparedBindingError("target-data-identity-drift")
        finally:
            os.close(data_fd)
        if config_payload is None:
            raise PreparedBindingError("target-config-payload-missing")
        _validate_target_config_payload(
            config_payload,
            request.target_uuid,
            request.target_name,
            "live-target-config",
        )
        package_after = os.fstat(package_fd)
        if _directory_identity(package_stat) != _directory_identity(
            package_after
        ):
            raise PreparedBindingError("target-package-identity-drift")
    finally:
        os.close(package_fd)

    return {
        "format": EVIDENCE_FORMAT,
        "target_config": config_identity,
        "target_data": _directory_identity_payload(data_stat),
        "target_efi": efi_identity,
        "target_name": request.target_name,
        "target_package": _directory_identity_payload(package_stat),
        "target_package_name": request.target_package_path.name,
        "target_package_path_sha256": hashlib.sha256(
            str(request.target_package_path).encode("utf-8")
        ).hexdigest(),
        "target_qcow2": qcow2_identity,
        "target_uuid": request.target_uuid,
    }


def _verify_bound_manifest(
    root: Path, manifest_sha256: str, label: str
) -> frozenset[str]:
    manifest = root / "files.sha256"
    try:
        root_stat = root.lstat()
        manifest_stat = manifest.lstat()
    except OSError as exc:
        raise PreparedBindingError(f"{label}-evidence-unavailable") from exc
    if (
        not stat.S_ISDIR(root_stat.st_mode)
        or stat.S_ISLNK(root_stat.st_mode)
        or stat.S_IMODE(root_stat.st_mode) != 0o700
    ):
        raise PreparedBindingError(f"{label}-root-identity-invalid")
    if (
        not stat.S_ISREG(manifest_stat.st_mode)
        or stat.S_ISLNK(manifest_stat.st_mode)
        or stat.S_IMODE(manifest_stat.st_mode) != 0o600
        or manifest_stat.st_uid != root_stat.st_uid
        or manifest_stat.st_nlink != 1
    ):
        raise PreparedBindingError(f"{label}-manifest-identity-invalid")
    if _sha256_file(manifest) != manifest_sha256:
        raise PreparedBindingError(f"{label}-manifest-drift")
    try:
        lines = manifest.read_text(encoding="ascii").splitlines()
    except (OSError, UnicodeDecodeError) as exc:
        raise PreparedBindingError(f"{label}-manifest-unreadable") from exc
    if not lines:
        raise PreparedBindingError(f"{label}-manifest-empty")
    entries: set[str] = set()
    for line in lines:
        if len(line) < 67 or line[64:66] != "  ":
            raise PreparedBindingError(f"{label}-manifest-line-invalid")
        expected_hash = line[:64]
        name = line[66:]
        if (
            not HEX_64.fullmatch(expected_hash)
            or not name
            or name.startswith("/")
            or "\\" in name
            or "\x00" in name
            or any(part in ("", ".", "..") for part in name.split("/"))
            or name in entries
        ):
            raise PreparedBindingError(f"{label}-manifest-entry-invalid")
        path = root / name
        try:
            item_stat = path.lstat()
        except OSError as exc:
            raise PreparedBindingError(f"{label}-entry-unavailable") from exc
        if (
            not stat.S_ISREG(item_stat.st_mode)
            or stat.S_ISLNK(item_stat.st_mode)
            or stat.S_IMODE(item_stat.st_mode) != 0o600
            or item_stat.st_uid != root_stat.st_uid
            or item_stat.st_nlink != 1
            or _sha256_file(path) != expected_hash
        ):
            raise PreparedBindingError(f"{label}-entry-identity-drift")
        entries.add(name)
    return frozenset(entries)


def _read_key_value_evidence(path: Path, label: str) -> dict[str, str]:
    lines = _read_ascii_lines(path, label)
    values: dict[str, str] = {}
    for line in lines:
        key, separator, value = line.partition("=")
        if (
            separator != "="
            or not KEY_NAME.fullmatch(key)
            or not value
            or key in values
            or any(character in "\x00\r\n" for character in value)
        ):
            raise PreparedBindingError(f"{label}-line-invalid")
        values[key] = value
    if not values:
        raise PreparedBindingError(f"{label}-empty")
    return values


def _read_ascii_lines(path: Path, label: str) -> list[str]:
    try:
        payload = path.read_bytes()
    except OSError as exc:
        raise PreparedBindingError(f"{label}-unreadable") from exc
    if not payload or not payload.endswith(b"\n") or b"\r" in payload:
        raise PreparedBindingError(f"{label}-framing-invalid")
    try:
        return payload.decode("ascii").splitlines()
    except UnicodeDecodeError as exc:
        raise PreparedBindingError(f"{label}-not-ascii") from exc


def _require_exact_text_fields(
    value: dict[str, str],
    expected: dict[str, str],
    expected_keys: set[str],
    label: str,
) -> None:
    if set(value) != expected_keys:
        raise PreparedBindingError(f"{label}-field-set-invalid")
    for key, expected_value in expected.items():
        if value.get(key) != expected_value:
            raise PreparedBindingError(f"{label}-{key.replace('_', '-')}")


def _require_sha256_text_field(
    value: dict[str, str], key: str, label: str
) -> None:
    if not HEX_64.fullmatch(value.get(key, "")):
        raise PreparedBindingError(
            f"{label}-{key.replace('_', '-')}-invalid"
        )


def _parse_registered_inventory(
    payload: bytes, label: str
) -> tuple[start_control.RegisteredVm, ...]:
    observation = start_control.CommandObservation.from_bytes(
        ("utmctl", "list"), stdout=payload
    )
    try:
        return start_control.parse_utmctl_list(observation)
    except start_control.StartControlError as exc:
        raise PreparedBindingError(f"{label}-invalid") from exc


def _validate_prepared_inventory(
    inventory: tuple[start_control.RegisteredVm, ...],
    request: PreparedBindingRequest,
    v7_identity: dict[str, object],
) -> None:
    if (
        len(inventory) != request.expected_vm_count
        or request.expected_vm_count != 21
        or any(item.status != "stopped" for item in inventory)
    ):
        raise PreparedBindingError("prior-prepared-inventory-state-invalid")
    current = {item.uuid: item for item in inventory}
    if len(current) != len(inventory):
        raise PreparedBindingError("prior-prepared-inventory-duplicate")
    target = current.pop(request.target_uuid, None)
    if target is None or target.name != request.target_name:
        raise PreparedBindingError("prior-prepared-target-inventory-invalid")
    baseline = v7_identity.get("baseline_inventory")
    if not isinstance(baseline, list):
        raise PreparedBindingError("prior-prepared-v7-inventory-invalid")
    baseline_by_uuid: dict[str, tuple[str, str]] = {}
    for item in baseline:
        if not isinstance(item, dict):
            raise PreparedBindingError("prior-prepared-v7-inventory-invalid")
        vm_uuid = item.get("uuid")
        name = item.get("name")
        status_value = item.get("status")
        if (
            not isinstance(vm_uuid, str)
            or not isinstance(name, str)
            or status_value != "stopped"
            or vm_uuid in baseline_by_uuid
        ):
            raise PreparedBindingError("prior-prepared-v7-inventory-invalid")
        baseline_by_uuid[vm_uuid] = (name, status_value)
    if set(current) != set(baseline_by_uuid) or any(
        (current[vm_uuid].name, current[vm_uuid].status) != identity
        for vm_uuid, identity in baseline_by_uuid.items()
    ):
        raise PreparedBindingError("prior-prepared-not-v7-plus-target")


def _validate_target_config_payload(
    payload: bytes, target_uuid: str, target_name: str, label: str
) -> None:
    try:
        value = plistlib.loads(payload)
    except plistlib.InvalidFileException as exc:
        raise PreparedBindingError(f"{label}-plist-invalid") from exc
    if not isinstance(value, dict):
        raise PreparedBindingError(f"{label}-not-object")
    information = value.get("Information")
    system = value.get("System")
    drives = value.get("Drive")
    if (
        value.get("Backend") != "QEMU"
        or value.get("ConfigurationVersion") != 4
        or value.get("Network") != []
        or not isinstance(information, dict)
        or information.get("Name") != target_name
        or information.get("UUID") != target_uuid
        or not isinstance(system, dict)
        or system.get("Architecture") != "aarch64"
        or system.get("Target") != "virt"
        or not isinstance(drives, list)
    ):
        raise PreparedBindingError(f"{label}-contract-invalid")
    disk_drives = [
        item
        for item in drives
        if isinstance(item, dict) and item.get("ImageType") == "Disk"
    ]
    if len(disk_drives) != 1 or any(
        disk_drives[0].get(key) != expected
        for key, expected in {
            "Identifier": TARGET_DISK_IDENTIFIER,
            "ImageName": TARGET_DISK_IMAGE_NAME,
            "Interface": "VirtIO",
            "ReadOnly": False,
        }.items()
    ):
        raise PreparedBindingError(f"{label}-disk-invalid")


def _open_verified_directory(
    path: Path,
    dir_fd: int | None,
    label: str,
    *,
    expected_mode: int,
    expected_owner: os.stat_result | None = None,
) -> int:
    flags = (
        os.O_RDONLY
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_DIRECTORY", 0)
        | getattr(os, "O_NOFOLLOW", 0)
    )
    try:
        descriptor = os.open(path, flags, dir_fd=dir_fd)
    except OSError as exc:
        raise PreparedBindingError(f"{label}-unavailable") from exc
    try:
        item_stat = os.fstat(descriptor)
        expected_uid = (
            expected_owner.st_uid if expected_owner is not None else os.geteuid()
        )
        expected_gid = (
            expected_owner.st_gid if expected_owner is not None else os.getegid()
        )
        if (
            not stat.S_ISDIR(item_stat.st_mode)
            or stat.S_IMODE(item_stat.st_mode) != expected_mode
            or item_stat.st_uid != expected_uid
            or item_stat.st_gid != expected_gid
        ):
            raise PreparedBindingError(f"{label}-identity-invalid")
    except Exception:
        os.close(descriptor)
        raise
    return descriptor


def _inspect_regular_file_at(
    dir_fd: int,
    name: str,
    label: str,
    expected_sha256: str,
    expected_owner: os.stat_result,
    *,
    capture_payload: bool = False,
) -> tuple[dict[str, object], bytes | None]:
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(
        os, "O_NOFOLLOW", 0
    )
    try:
        descriptor = os.open(name, flags, dir_fd=dir_fd)
    except OSError as exc:
        raise PreparedBindingError(f"{label}-unavailable") from exc
    try:
        before = os.fstat(descriptor)
        if (
            not stat.S_ISREG(before.st_mode)
            or stat.S_IMODE(before.st_mode) != 0o644
            or before.st_uid != expected_owner.st_uid
            or before.st_gid != expected_owner.st_gid
            or before.st_nlink != 1
            or (capture_payload and before.st_size > 64 * 1024)
        ):
            raise PreparedBindingError(f"{label}-identity-invalid")
        digest = hashlib.sha256()
        captured = bytearray() if capture_payload else None
        while chunk := os.read(descriptor, 1024 * 1024):
            digest.update(chunk)
            if captured is not None:
                captured.extend(chunk)
        after = os.fstat(descriptor)
        if _regular_file_identity(before) != _regular_file_identity(after):
            raise PreparedBindingError(f"{label}-identity-drift")
        actual_sha256 = digest.hexdigest()
        if actual_sha256 != expected_sha256:
            raise PreparedBindingError(f"{label}-sha256-mismatch")
        return (
            {
                "device": before.st_dev,
                "ctime_ns": before.st_ctime_ns,
                "gid": before.st_gid,
                "inode": before.st_ino,
                "mtime_ns": before.st_mtime_ns,
                "mode": f"{stat.S_IMODE(before.st_mode):04o}",
                "nlink": before.st_nlink,
                "sha256": actual_sha256,
                "size": before.st_size,
                "uid": before.st_uid,
            },
            bytes(captured) if captured is not None else None,
        )
    finally:
        os.close(descriptor)


def _directory_identity(value: os.stat_result) -> tuple[int, ...]:
    return (
        value.st_dev,
        value.st_ino,
        value.st_mode,
        value.st_nlink,
        value.st_uid,
        value.st_gid,
        value.st_mtime_ns,
        value.st_ctime_ns,
    )


def _regular_file_identity(value: os.stat_result) -> tuple[int, ...]:
    return (*_directory_identity(value), value.st_size)


def _directory_identity_payload(
    value: os.stat_result,
) -> dict[str, object]:
    return {
        "ctime_ns": value.st_ctime_ns,
        "device": value.st_dev,
        "gid": value.st_gid,
        "inode": value.st_ino,
        "mode": f"{stat.S_IMODE(value.st_mode):04o}",
        "mtime_ns": value.st_mtime_ns,
        "nlink": value.st_nlink,
        "uid": value.st_uid,
    }


def _sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    try:
        with path.open("rb") as stream:
            while chunk := stream.read(1024 * 1024):
                digest.update(chunk)
    except OSError as exc:
        raise PreparedBindingError("file-hash-unavailable") from exc
    return digest.hexdigest()
