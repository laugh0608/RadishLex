#!/usr/bin/env python3
from __future__ import annotations

import hashlib
import os
import plistlib
import stat
import uuid
import xml.etree.ElementTree as element_tree
from dataclasses import dataclass
from pathlib import Path
from typing import Protocol

import l6_upgrade_quiesced_case_contract as case_contract
import l6_utm_clone_once as clone_control


CONTROL_FORMAT = (
    "radishlex-linux-l6-upgrade-quiesced-registration-shell-control-v2"
)
SHELL_EVIDENCE_FORMAT = (
    "radishlex-linux-l6-upgrade-quiesced-dedicated-registration-shell-v1"
)
CONTROL_RELATIVE_PATH = Path(
    "scripts/linux-product/l6_upgrade_quiesced_registration_shell.py"
)
BINDING_RELATIVE_PATH = Path(
    "scripts/linux-product/"
    "l6_upgrade_quiesced_registration_shell_bindings.py"
)
TRANSPORT_RELATIVE_PATH = Path(
    "scripts/linux-product/"
    "l6_upgrade_quiesced_registration_shell.applescript"
)
TRANSPORT_SOURCE = (
    'on run arguments\n'
    '    if (count of arguments) is less than 2 then error '
    '"expected-action-and-shell-name"\n'
    '    set shellAction to item 1 of arguments\n'
    '    tell application id "com.utmapp.UTM"\n'
    '        if shellAction is "create" then\n'
    '            if (count of arguments) is not 2 then error '
    '"create-expected-shell-name"\n'
    '            set shellName to item 2 of arguments\n'
    '            set shellMachine to make new virtual machine with '
    'properties {backend:qemu, configuration:{name:shellName, '
    'architecture:"aarch64", drives:{{interface:VirtIO, guest size:1024, '
    'raw:false}}}}\n'
    '        else if shellAction is "update" then\n'
    '            if (count of arguments) is not 3 then error '
    '"update-expected-shell-id-and-name"\n'
    '            set shellIdentifier to item 2 of arguments\n'
    '            set shellName to item 3 of arguments\n'
    '            set shellMachine to virtual machine id shellIdentifier\n'
    '            if status of shellMachine is not stopped then error '
    '"registration-shell-not-stopped"\n'
    '            set shellConfiguration to configuration of shellMachine\n'
    '            set name of shellConfiguration to shellName\n'
    '            set icon of shellConfiguration to "linux"\n'
    '            set notes of shellConfiguration to "RadishLex L6 '
    'upgrade_quiesced registration-only shell; never started."\n'
    '            set architecture of shellConfiguration to "aarch64"\n'
    '            set machine of shellConfiguration to "virt"\n'
    '            set memory of shellConfiguration to 4096\n'
    '            set cpu cores of shellConfiguration to 0\n'
    '            set hypervisor of shellConfiguration to true\n'
    '            set uefi of shellConfiguration to true\n'
    '            set directory share mode of shellConfiguration to '
    'VirtFS\n'
    '            set network interfaces of shellConfiguration to {}\n'
    '            set «class SrPt» of shellConfiguration to {}\n'
    '            set displays of shellConfiguration to '
    '{{hardware:"virtio-gpu-pci", dynamic resolution:true}}\n'
    '            set qemu additional arguments of shellConfiguration to '
    '{}\n'
    '            update configuration of shellMachine with '
    'shellConfiguration\n'
    '        else\n'
    '            error "unsupported-registration-shell-action"\n'
    '        end if\n'
    '        return id of shellMachine\n'
    '    end tell\n'
    'end run\n'
).encode("utf-8")
UTM_BUNDLE_ROOT = Path("/Applications/UTM.app")
EXPECTED_UTM_BUNDLE_ID = "com.utmapp.UTM"
EXPECTED_UTM_VERSION = "4.7.5"
EXPECTED_UTM_BUILD = "118"
REQUIRED_SHELL_NAME = (
    "RadishLex-L6-Registration-Shell-d75818f-Upgrade-Quiesced-v1"
)
REQUIRED_SHELL_NOTES = (
    "RadishLex L6 upgrade_quiesced registration-only shell; never started."
)


class RegistrationShellBindingError(ValueError):
    pass


class RegistrationShellRequestLike(Protocol):
    repository_root: Path
    expected_repository_head: str
    source_snapshot_root: Path
    registration_shell_name: str
    registration_shell_package_path: Path
    expected_precreate_inventory_sha256: str


class RegistrationShellEvidenceRequestLike(Protocol):
    registration_shell_name: str
    registration_shell_package_path: Path


class ShellIdentityLike(Protocol):
    config_sha256: str
    efi_sha256: str
    qcow2_name: str
    qcow2_sha256: str
    uuid: str
    network: object


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


def validate_registration_shell_bindings(
    request: RegistrationShellRequestLike,
) -> dict[str, object]:
    repository_head = _run_git(
        request.repository_root, ("rev-parse", "HEAD")
    ).decode("ascii").strip()
    if repository_head != request.expected_repository_head:
        raise RegistrationShellBindingError("repository-head-drift")
    if _run_git(request.repository_root, ("status", "--porcelain")):
        raise RegistrationShellBindingError("repository-not-clean")
    case_contract.validate_repository_contract(request.repository_root)
    control_identity = validate_control_identity(request.repository_root)
    source = validate_source_snapshot(request)
    utm_identity = validate_utm_bundle(UTM_BUNDLE_ROOT)
    return {
        **control_identity,
        "case_contract": "validated",
        "expected_precreate_inventory_sha256": (
            request.expected_precreate_inventory_sha256
        ),
        "format": CONTROL_FORMAT,
        "repository_clean": True,
        "repository_head": repository_head,
        "source_snapshot": source.as_json(),
        **utm_identity,
    }


def validate_control_identity(repository_root: Path) -> dict[str, object]:
    control_path = repository_root / CONTROL_RELATIVE_PATH
    binding_path = repository_root / BINDING_RELATIVE_PATH
    transport_path = repository_root / TRANSPORT_RELATIVE_PATH
    for path, label in (
        (control_path, "executed-control"),
        (binding_path, "binding-control"),
        (transport_path, "configuration-transport"),
    ):
        _validate_committed_control_file(path, label)
    if Path(__file__).absolute() != binding_path:
        raise RegistrationShellBindingError(
            "executed-binding-control-path-mismatch"
        )
    try:
        transport_source = transport_path.read_bytes()
    except OSError as exc:
        raise RegistrationShellBindingError(
            "configuration-transport-unreadable"
        ) from exc
    if transport_source != TRANSPORT_SOURCE:
        raise RegistrationShellBindingError(
            "configuration-transport-source-contract-drift"
        )
    return {
        "binding_control_sha256": sha256_file(binding_path),
        "control_sha256": sha256_file(control_path),
        "configuration_transport_sha256": sha256_file(transport_path),
    }


def validate_source_snapshot(
    request: RegistrationShellRequestLike,
) -> BundleIdentity:
    identity = read_bundle(request.source_snapshot_root, root_mode=0o700)
    expected = case_contract.EXPECTED_START
    if (
        identity.config_sha256 != expected["config_sha256"]
        or identity.efi_sha256 != expected["efi_sha256"]
        or identity.qcow2_sha256 != expected["qcow2_sha256"]
    ):
        raise RegistrationShellBindingError(
            "source-snapshot-disk-identity-drift"
        )
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


def validate_registration_shell_bundle(
    request: RegistrationShellRequestLike,
    expected_uuid: str,
) -> BundleIdentity:
    identity = read_bundle(
        request.registration_shell_package_path, root_mode=0o755
    )
    if (
        identity.name != request.registration_shell_name
        or identity.name != REQUIRED_SHELL_NAME
        or identity.uuid != expected_uuid
        or identity.network != []
    ):
        raise RegistrationShellBindingError(
            "registration-shell-runtime-identity-drift"
        )
    config = _read_plist(identity.config_path, "registration-shell-config")
    information = config.get("Information")
    system = config.get("System")
    qemu = config.get("QEMU")
    drives = config.get("Drive")
    displays = config.get("Display")
    sharing = config.get("Sharing")
    if (
        config.get("Backend") != "QEMU"
        or config.get("ConfigurationVersion") != 4
        or not isinstance(information, dict)
        or information.get("Notes") != REQUIRED_SHELL_NOTES
        or not isinstance(system, dict)
        or system.get("Architecture") != "aarch64"
        or system.get("Target") != "virt"
        or system.get("MemorySize") != 4096
        or system.get("CPUCount") != 0
        or not isinstance(qemu, dict)
        or qemu.get("Hypervisor") is not True
        or qemu.get("UEFIBoot") is not True
        or not isinstance(drives, list)
        or len(drives) != 1
        or not isinstance(displays, list)
        or len(displays) != 1
        or not isinstance(displays[0], dict)
        or displays[0].get("Hardware") != "virtio-gpu-pci"
        or not isinstance(sharing, dict)
        or sharing.get("DirectoryShareMode") != "VirtFS"
        or config.get("Network") != []
    ):
        raise RegistrationShellBindingError(
            "registration-shell-configuration-contract-drift"
        )
    disk = drives[0]
    if (
        not isinstance(disk, dict)
        or disk.get("ImageType") != "Disk"
        or disk.get("Interface") != "VirtIO"
        or disk.get("ReadOnly") is not False
        or disk.get("ImageName") != identity.qcow2_name
    ):
        raise RegistrationShellBindingError(
            "registration-shell-disk-contract-drift"
        )
    return identity


def validate_registration_shell_before_update(
    request: RegistrationShellRequestLike,
    expected_uuid: str,
) -> BundleIdentity:
    identity = read_bundle(
        request.registration_shell_package_path, root_mode=0o755
    )
    if (
        identity.name != request.registration_shell_name
        or identity.name != REQUIRED_SHELL_NAME
        or identity.uuid != expected_uuid
    ):
        raise RegistrationShellBindingError(
            "registration-shell-preupdate-identity-drift"
        )
    config = _read_plist(
        identity.config_path, "registration-shell-preupdate-config"
    )
    system = config.get("System")
    if (
        config.get("Backend") != "QEMU"
        or config.get("ConfigurationVersion") != 4
        or not isinstance(system, dict)
        or system.get("Architecture") != "aarch64"
        or system.get("Target") != "virt"
    ):
        raise RegistrationShellBindingError(
            "registration-shell-preupdate-configuration-drift"
        )
    return identity


def expected_shell_evidence(
    request: RegistrationShellEvidenceRequestLike,
    shell: ShellIdentityLike,
) -> dict[str, object]:
    return {
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
        "vm_uuid": shell.uuid,
    }


def validate_utm_bundle(bundle_root: Path) -> dict[str, object]:
    try:
        bundle_stat = bundle_root.lstat()
    except OSError as exc:
        raise RegistrationShellBindingError("utm-bundle-unavailable") from exc
    if not stat.S_ISDIR(bundle_stat.st_mode) or stat.S_ISLNK(
        bundle_stat.st_mode
    ):
        raise RegistrationShellBindingError("utm-bundle-identity-invalid")
    info_path = bundle_root / "Contents/Info.plist"
    sdef_path = bundle_root / "Contents/Resources/UTM.sdef"
    for path, label in (
        (info_path, "utm-info-plist"),
        (sdef_path, "utm-sdef"),
    ):
        _validate_external_regular_file(path, label)
    info = _read_plist(info_path, "utm-info-plist")
    expected_info = {
        "CFBundleIdentifier": EXPECTED_UTM_BUNDLE_ID,
        "CFBundleShortVersionString": EXPECTED_UTM_VERSION,
        "CFBundleVersion": EXPECTED_UTM_BUILD,
    }
    for key, expected in expected_info.items():
        if info.get(key) != expected:
            raise RegistrationShellBindingError(
                f"utm-info-{key}-mismatch"
            )
    try:
        sdef_root = element_tree.parse(sdef_path).getroot()
    except (OSError, element_tree.ParseError) as exc:
        raise RegistrationShellBindingError("utm-sdef-invalid") from exc
    make_commands = [
        item
        for item in sdef_root.iter("command")
        if item.get("name") == "make" and item.get("code") == "corecrel"
    ]
    if len(make_commands) != 1:
        raise RegistrationShellBindingError("utm-sdef-make-command-invalid")
    parameters = {
        item.get("name"): (item.get("code"), item.get("type"))
        for item in make_commands[0].findall("parameter")
    }
    if parameters != {
        "new": ("kocl", "type"),
        "with properties": ("prdt", "record"),
    }:
        raise RegistrationShellBindingError(
            "utm-sdef-make-parameters-invalid"
        )
    result = make_commands[0].find("result")
    if result is None or result.get("type") != "specifier":
        raise RegistrationShellBindingError("utm-sdef-make-result-invalid")
    _validate_sdef_configuration_contract(sdef_root)
    return {
        "utm_build": EXPECTED_UTM_BUILD,
        "utm_bundle_id": EXPECTED_UTM_BUNDLE_ID,
        "utm_info_plist_sha256": sha256_file(info_path),
        "utm_sdef_sha256": sha256_file(sdef_path),
        "utm_version": EXPECTED_UTM_VERSION,
    }


def read_bundle(root: Path, *, root_mode: int) -> BundleIdentity:
    _validate_directory(root, root_mode, "bundle-root")
    data = root / "Data"
    _validate_directory(data, root_mode, "bundle-data")
    config_path = root / "config.plist"
    _validate_regular_file(config_path, 0o644, None, "config")
    config = _read_plist(config_path, "config")
    information = config.get("Information")
    drives = config.get("Drive")
    if not isinstance(information, dict) or not isinstance(drives, list):
        raise RegistrationShellBindingError(
            "config-required-fields-invalid"
        )
    disks = [
        item
        for item in drives
        if isinstance(item, dict) and item.get("ImageType") == "Disk"
    ]
    if len(disks) != 1 or disks[0].get("Interface") != "VirtIO":
        raise RegistrationShellBindingError("config-disk-contract-invalid")
    qcow2_name = disks[0].get("ImageName")
    if (
        not isinstance(qcow2_name, str)
        or Path(qcow2_name).name != qcow2_name
        or not qcow2_name.endswith(".qcow2")
    ):
        raise RegistrationShellBindingError("config-qcow2-name-invalid")
    name = information.get("Name")
    vm_uuid = information.get("UUID")
    if not isinstance(name, str) or not isinstance(vm_uuid, str):
        raise RegistrationShellBindingError("config-information-invalid")
    canonical_uuid = _canonical_uuid(vm_uuid, "config-uuid")
    if canonical_uuid != vm_uuid:
        raise RegistrationShellBindingError("config-uuid-not-canonical")
    efi_path = data / "efi_vars.fd"
    qcow2_path = data / qcow2_name
    _validate_regular_file(efi_path, 0o644, None, "efi")
    _validate_regular_file(qcow2_path, 0o644, None, "qcow2")
    return BundleIdentity(
        config_path=config_path,
        efi_path=efi_path,
        qcow2_path=qcow2_path,
        config_sha256=sha256_file(config_path),
        efi_sha256=sha256_file(efi_path),
        qcow2_sha256=sha256_file(qcow2_path),
        qcow2_name=qcow2_name,
        name=name,
        uuid=vm_uuid,
        network=config.get("Network"),
    )


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    try:
        with path.open("rb") as source:
            while block := source.read(1024 * 1024):
                digest.update(block)
    except OSError as exc:
        raise RegistrationShellBindingError(
            f"sha256-unreadable:{path.name}"
        ) from exc
    return digest.hexdigest()


def sha256_text(value: str) -> str:
    return hashlib.sha256(value.encode("utf-8")).hexdigest()


def _validate_sdef_configuration_contract(
    root: element_tree.Element,
) -> None:
    vm_extensions = [
        item
        for item in root.iter("class-extension")
        if item.get("extends") == "virtual machine"
        and any(
            child.get("name") == "configuration"
            and child.get("code") == "CoFg"
            for child in item.findall("property")
        )
    ]
    update_commands = [
        item
        for item in root.iter("command")
        if item.get("name") == "update configuration"
        and item.get("code") == "UTMcUpDt"
    ]
    qemu_records = [
        item
        for item in root.iter("record-type")
        if item.get("name") == "qemu configuration"
        and item.get("code") == "QeCf"
    ]
    drive_records = [
        item
        for item in root.iter("record-type")
        if item.get("name") == "qemu drive configuration"
        and item.get("code") == "QdEc"
    ]
    drive_enums = [
        item
        for item in root.iter("enumeration")
        if item.get("name") == "qemu drive interface"
        and item.get("code") == "QeDi"
    ]
    if (
        len(vm_extensions) != 1
        or len(update_commands) != 1
        or len(qemu_records) != 1
        or len(drive_records) != 1
        or len(drive_enums) != 1
    ):
        raise RegistrationShellBindingError(
            "utm-sdef-qemu-create-contract-invalid"
        )
    configuration_properties = [
        item
        for item in vm_extensions[0].findall("property")
        if item.get("name") == "configuration"
        and item.get("code") == "CoFg"
        and item.get("access") == "r"
    ]
    update_parameters = [
        item
        for item in update_commands[0].findall("parameter")
        if item.get("name") == "with" and item.get("code") == "UpCf"
    ]
    if (
        len(configuration_properties) != 1
        or {
            item.get("type")
            for item in configuration_properties[0].findall("type")
        }
        != {"qemu configuration", "apple configuration"}
        or len(update_parameters) != 1
        or {
            item.get("type")
            for item in update_parameters[0].findall("type")
        }
        != {"qemu configuration", "apple configuration"}
    ):
        raise RegistrationShellBindingError(
            "utm-sdef-qemu-update-contract-invalid"
        )
    qemu_properties = {
        item.get("name"): item.get("code")
        for item in qemu_records[0].findall("property")
    }
    if not {
        "name": "pnam",
        "architecture": "ArCh",
        "machine": "MaCh",
        "memory": "MeMy",
        "cpu cores": "CpUc",
        "hypervisor": "HyPr",
        "uefi": "UeFi",
        "directory share mode": "DrSm",
        "drives": "DrVs",
        "network interfaces": "NtIf",
        "serial ports": "SrPt",
        "displays": "DiPs",
        "qemu additional arguments": "QeAd",
    }.items() <= qemu_properties.items():
        raise RegistrationShellBindingError(
            "utm-sdef-qemu-configuration-fields-invalid"
        )
    drive_properties = {
        item.get("name") for item in drive_records[0].findall("property")
    }
    if not {"interface", "guest size", "raw"}.issubset(drive_properties):
        raise RegistrationShellBindingError(
            "utm-sdef-qemu-drive-fields-invalid"
        )
    virtio = [
        item
        for item in drive_enums[0].findall("enumerator")
        if item.get("name") == "VirtIO" and item.get("code") == "QdIv"
    ]
    if len(virtio) != 1:
        raise RegistrationShellBindingError(
            "utm-sdef-qemu-virtio-enumerator-invalid"
        )


def _validate_committed_control_file(path: Path, label: str) -> None:
    try:
        metadata = path.lstat()
    except OSError as exc:
        raise RegistrationShellBindingError(f"{label}-unavailable") from exc
    if (
        not stat.S_ISREG(metadata.st_mode)
        or stat.S_ISLNK(metadata.st_mode)
        or metadata.st_nlink != 1
        or stat.S_IMODE(metadata.st_mode) & 0o022
    ):
        raise RegistrationShellBindingError(f"{label}-identity-invalid")


def _validate_external_regular_file(path: Path, label: str) -> None:
    try:
        metadata = path.lstat()
    except OSError as exc:
        raise RegistrationShellBindingError(f"{label}-unavailable") from exc
    if (
        not stat.S_ISREG(metadata.st_mode)
        or stat.S_ISLNK(metadata.st_mode)
        or metadata.st_nlink != 1
        or stat.S_IMODE(metadata.st_mode) & 0o022
    ):
        raise RegistrationShellBindingError(f"{label}-identity-invalid")


def _validate_directory(path: Path, mode: int, label: str) -> None:
    try:
        metadata = path.lstat()
    except OSError as exc:
        raise RegistrationShellBindingError(f"{label}-unavailable") from exc
    if (
        not stat.S_ISDIR(metadata.st_mode)
        or stat.S_ISLNK(metadata.st_mode)
        or stat.S_IMODE(metadata.st_mode) != mode
        or metadata.st_uid != os.getuid()
        or metadata.st_gid != os.getgid()
    ):
        raise RegistrationShellBindingError(f"{label}-identity-invalid")


def _validate_regular_file(
    path: Path,
    mode: int,
    expected_sha256: str | None,
    label: str,
) -> None:
    try:
        metadata = path.lstat()
    except OSError as exc:
        raise RegistrationShellBindingError(f"{label}-unavailable") from exc
    if (
        not stat.S_ISREG(metadata.st_mode)
        or stat.S_ISLNK(metadata.st_mode)
        or stat.S_IMODE(metadata.st_mode) != mode
        or metadata.st_uid != os.getuid()
        or metadata.st_gid != os.getgid()
        or metadata.st_nlink != 1
    ):
        raise RegistrationShellBindingError(f"{label}-identity-invalid")
    if expected_sha256 is not None and sha256_file(path) != expected_sha256:
        raise RegistrationShellBindingError(f"{label}-sha256-mismatch")


def _read_plist(path: Path, label: str) -> dict[str, object]:
    try:
        with path.open("rb") as source:
            value = plistlib.load(source)
    except (OSError, plistlib.InvalidFileException) as exc:
        raise RegistrationShellBindingError(f"{label}-invalid") from exc
    if not isinstance(value, dict):
        raise RegistrationShellBindingError(f"{label}-root-invalid")
    return value


def _canonical_uuid(value: str, label: str) -> str:
    try:
        return str(uuid.UUID(value)).upper()
    except ValueError as exc:
        raise RegistrationShellBindingError(f"{label}-invalid") from exc


def _run_git(repository_root: Path, arguments: tuple[str, ...]) -> bytes:
    observation = clone_control.SubprocessCommandRunner().run(
        ("/usr/bin/git", "-C", str(repository_root), *arguments), 30
    )
    try:
        clone_control._require_successful_observation(observation, "git")
    except clone_control.CloneControlError as exc:
        raise RegistrationShellBindingError(str(exc)) from exc
    return observation.stdout.prefix
