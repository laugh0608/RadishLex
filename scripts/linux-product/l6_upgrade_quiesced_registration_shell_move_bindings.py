#!/usr/bin/env python3
from __future__ import annotations

import json
import plistlib
import xml.etree.ElementTree as element_tree
from pathlib import Path
from typing import Protocol

import l6_upgrade_quiesced_case_contract as case_contract
import l6_upgrade_quiesced_registration_shell_bindings as shell_bindings
import l6_utm_clone_once as clone_control


EVIDENCE_FORMAT = (
    "radishlex-linux-l6-upgrade-quiesced-registration-shell-move-v1"
)
CONTROL_RELATIVE_PATH = Path(
    "scripts/linux-product/"
    "l6_upgrade_quiesced_registration_shell_move.py"
)
BINDING_RELATIVE_PATH = Path(
    "scripts/linux-product/"
    "l6_upgrade_quiesced_registration_shell_move_bindings.py"
)
DEFAULT_UTM_STORAGE_ROOT = Path(
    "/Users/luobo/Library/Containers/com.utmapp.UTM/Data/Documents"
)
REQUIRED_SHELL_UUID = "0BAA7355-A55A-463E-97FE-82A785E252A7"
REQUIRED_V4_CONTROL_MANIFEST_SHA256 = (
    "70251467e0dc514347426e95f7374272e47da91edc038b10267d97d187a0fb04"
)
REQUIRED_V4_EXECUTION_HEAD = "7ab855b08f24e7a47e826dbcc96d83e5c209517a"
REQUIRED_V4_ATTEMPT_ID = (
    "d75818f-upgrade-quiesced-registration-shell-20260830-v4"
)
REQUIRED_INVENTORY_COUNT = 22
REQUIRED_INVENTORY_SHA256 = (
    "1074717ca5ff9f8666e53974b9002d1c4697a33de877e5cdfbdcf24299f4714f"
)
EXPECTED_V4_PARTIAL = {
    "config_sha256": (
        "711db7f11293c2a485f480bb34b3f9e66b317d59e04101a80e1f141027c76479"
    ),
    "efi_sha256": (
        "7b0a7f26192011e6e98c770694269b40f8b70620ca58fc4973a232fb223600d5"
    ),
    "qcow2_name": "EF62CC7C-DF55-4A83-8633-F4630CF3B234.qcow2",
    "qcow2_sha256": (
        "ae44c4d0b6b789f232932cc0dfbda31dcd54edfb6849ed97e3a4a0c228c2b94b"
    ),
}
V4_CONTROL_MEMBERS = (
    "request.json",
    "binding-preflight.json",
    "source-snapshot-preflight.json",
    "registration-shell-package-precreate.json",
    "registration-shell-evidence-precreate.json",
    "source-and-shell-parent-device.json",
    "source-handles-precreate.json",
    "utmctl-list-precreate.json",
    "inventory-precreate.json",
    "create-new-registration-shell.json",
    "utmctl-list-postcreate.json",
    "inventory-postcreate.json",
    "registration-shell-package-postcreate.json",
    "terminal.json",
)
PREPARE_MEMBERS = (
    "request.json",
    "binding-preflight.json",
    "source-snapshot-preflight.json",
    "default-shell-preflight.json",
    "external-shell-preflight.json",
    "shell-evidence-preflight.json",
    "source-default-target-device.json",
    "source-and-default-handles-preflight.json",
    "utmctl-list-pre-move.json",
    "inventory-pre-move.json",
    "source-and-default-handles-post-list.json",
    "source-snapshot-terminal.json",
    "default-shell-terminal.json",
    "external-shell-terminal.json",
    "terminal.json",
)
COMPLETE_MEMBERS = (
    "request.json",
    "binding-preflight.json",
    "source-snapshot-preflight.json",
    "default-shell-preupdate.json",
    "external-shell-preupdate.json",
    "shell-evidence-preupdate.json",
    "source-and-shell-parent-device.json",
    "source-and-shell-handles-preupdate.json",
    "utmctl-list-preupdate.json",
    "inventory-preupdate.json",
    "update-registration-shell-configuration.json",
    "utmctl-list-postupdate.json",
    "inventory-postupdate.json",
    "external-shell-postupdate.json",
    "source-and-shell-handles-terminal.json",
    "source-snapshot-terminal.json",
    "default-shell-terminal.json",
    "external-shell-terminal.json",
    "registration-shell-evidence-freeze.json",
    "terminal.json",
)
UPSTREAM_UTM_TAG = "v4.7.5"
UPSTREAM_UTM_DATA_SOURCE = (
    "https://github.com/utmapp/UTM/blob/v4.7.5/Platform/UTMData.swift"
)


class ShellMoveBindingError(ValueError):
    pass


class CommonRequest(Protocol):
    repository_root: Path
    expected_repository_head: str
    prior_v4_control_root: Path
    prior_v4_manifest_sha256: str
    source_snapshot_root: Path
    default_shell_package_path: Path
    registration_shell_package_path: Path
    registration_shell_evidence_root: Path
    registration_shell_name: str
    registration_shell_uuid: str
    expected_vm_count: int
    expected_inventory_sha256: str


class CompleteRequest(CommonRequest, Protocol):
    prepare_root: Path
    prepare_manifest_sha256: str


def validate_prepare_bindings(request: CommonRequest) -> dict[str, object]:
    common = _validate_common_bindings(request)
    return {
        **common,
        "format": EVIDENCE_FORMAT,
        "phase": "prepare",
    }


def validate_complete_bindings(
    request: CompleteRequest,
) -> dict[str, object]:
    common = _validate_common_bindings(request)
    prepare = validate_prepare_evidence(request)
    return {
        **common,
        "format": EVIDENCE_FORMAT,
        "phase": "complete",
        "prepare_evidence": prepare,
    }


def _validate_common_bindings(request: CommonRequest) -> dict[str, object]:
    head = _run_git(
        request.repository_root, ("rev-parse", "HEAD")
    ).decode("ascii").strip()
    if head != request.expected_repository_head:
        raise ShellMoveBindingError("repository-head-drift")
    if _run_git(request.repository_root, ("status", "--porcelain")):
        raise ShellMoveBindingError("repository-not-clean")
    case_contract.validate_repository_contract(request.repository_root)
    controls = validate_control_identity(request.repository_root)
    source = shell_bindings.validate_source_snapshot(request)
    utm = shell_bindings.validate_utm_bundle(
        shell_bindings.UTM_BUNDLE_ROOT
    )
    v4 = validate_frozen_v4_control(request)
    return {
        **controls,
        "case_contract": "validated",
        "frozen_v4_control": v4,
        "repository_clean": True,
        "repository_head": head,
        "source_snapshot": source.as_json(),
        "upstream_utm_data_source": UPSTREAM_UTM_DATA_SOURCE,
        "upstream_utm_tag": UPSTREAM_UTM_TAG,
        **utm,
    }


def validate_control_identity(repository_root: Path) -> dict[str, object]:
    control_path = repository_root / CONTROL_RELATIVE_PATH
    binding_path = repository_root / BINDING_RELATIVE_PATH
    for path, label in (
        (control_path, "move-control"),
        (binding_path, "move-binding-control"),
    ):
        shell_bindings._validate_committed_control_file(path, label)
    if Path(__file__).absolute() != binding_path:
        raise ShellMoveBindingError(
            "executed-move-binding-control-path-mismatch"
        )
    base = shell_bindings.validate_control_identity(repository_root)
    sdef_path = (
        shell_bindings.UTM_BUNDLE_ROOT / "Contents/Resources/UTM.sdef"
    )
    _validate_ui_only_move_surface(sdef_path)
    return {
        **base,
        "move_binding_control_sha256": shell_bindings.sha256_file(
            binding_path
        ),
        "move_control_sha256": shell_bindings.sha256_file(control_path),
        "utm_move_surface": "ui-only-no-sdef-command",
    }


def validate_frozen_v4_control(
    request: CommonRequest,
) -> dict[str, object]:
    if (
        request.prior_v4_manifest_sha256
        != REQUIRED_V4_CONTROL_MANIFEST_SHA256
    ):
        raise ShellMoveBindingError("frozen-v4-manifest-request-drift")
    manifest = request.prior_v4_control_root / "files.sha256"
    if shell_bindings.sha256_file(manifest) != (
        request.prior_v4_manifest_sha256
    ):
        raise ShellMoveBindingError("frozen-v4-manifest-drift")
    try:
        entries = clone_control._verify_sha256_manifest(
            request.prior_v4_control_root, manifest
        )
    except clone_control.CloneControlError as exc:
        raise ShellMoveBindingError(str(exc)) from exc
    _require_manifest_members(
        request.prior_v4_control_root,
        manifest,
        V4_CONTROL_MEMBERS,
        entries,
        "frozen-v4",
    )
    frozen_request = _read_json(
        request.prior_v4_control_root / "request.json",
        "frozen-v4-request",
    )
    binding = _read_json(
        request.prior_v4_control_root / "binding-preflight.json",
        "frozen-v4-binding",
    )
    create = _read_json(
        request.prior_v4_control_root / "create-new-registration-shell.json",
        "frozen-v4-create",
    )
    inventory = _read_json(
        request.prior_v4_control_root / "inventory-postcreate.json",
        "frozen-v4-inventory",
    )
    package = _read_json(
        request.prior_v4_control_root
        / "registration-shell-package-postcreate.json",
        "frozen-v4-package",
    )
    terminal = _read_json(
        request.prior_v4_control_root / "terminal.json",
        "frozen-v4-terminal",
    )
    if (
        frozen_request.get("attempt_id") != REQUIRED_V4_ATTEMPT_ID
        or frozen_request.get("expected_repository_head")
        != REQUIRED_V4_EXECUTION_HEAD
        or frozen_request.get("registration_shell_name")
        != shell_bindings.REQUIRED_SHELL_NAME
        or binding.get("repository_head") != REQUIRED_V4_EXECUTION_HEAD
        or binding.get("utm_version") != shell_bindings.EXPECTED_UTM_VERSION
        or binding.get("utm_build") != shell_bindings.EXPECTED_UTM_BUILD
        or create.get("exit_code") != 0
        or create.get("timed_out") is not False
        or inventory
        != {
            "all_stopped": True,
            "canonical_inventory_sha256": REQUIRED_INVENTORY_SHA256,
            "format": shell_bindings.CONTROL_FORMAT,
            "registered_vm_count": REQUIRED_INVENTORY_COUNT,
        }
        or package.get("state") != "absent"
        or terminal.get("outcome") != "state-indeterminate"
        or terminal.get("reason")
        != (
            "registration-shell-package-postcreate:"
            "registration-shell-package-not-created"
        )
        or terminal.get("registration_shell_uuid")
        != REQUIRED_SHELL_UUID
        or terminal.get("create_invocations") != 1
        or terminal.get("update_invocations") != 0
        or terminal.get("clone_invocations") != 0
        or terminal.get("automatic_delete") != "not-performed"
        or terminal.get("automatic_retry") != "not-performed"
        or terminal.get("automatic_start") != "not-performed"
    ):
        raise ShellMoveBindingError("frozen-v4-semantic-drift")
    stdout = create.get("stdout")
    stderr = create.get("stderr")
    if (
        not isinstance(stdout, dict)
        or stdout.get("prefix_utf8") != f"{REQUIRED_SHELL_UUID}\n"
        or stdout.get("truncated") is not False
        or not isinstance(stderr, dict)
        or stderr.get("total_bytes") != 0
    ):
        raise ShellMoveBindingError("frozen-v4-create-output-drift")
    return {
        "entries_verified": entries,
        "execution_head": REQUIRED_V4_EXECUTION_HEAD,
        "inventory_sha256": REQUIRED_INVENTORY_SHA256,
        "manifest_sha256": request.prior_v4_manifest_sha256,
        "registration_shell_uuid": REQUIRED_SHELL_UUID,
    }


def validate_prepare_evidence(
    request: CompleteRequest,
) -> dict[str, object]:
    manifest = request.prepare_root / "files.sha256"
    if shell_bindings.sha256_file(manifest) != request.prepare_manifest_sha256:
        raise ShellMoveBindingError("prepare-manifest-drift")
    try:
        entries = clone_control._verify_sha256_manifest(
            request.prepare_root, manifest
        )
    except clone_control.CloneControlError as exc:
        raise ShellMoveBindingError(str(exc)) from exc
    _require_manifest_members(
        request.prepare_root,
        manifest,
        PREPARE_MEMBERS,
        entries,
        "prepare",
    )
    prepared_request = _read_json(
        request.prepare_root / "request.json", "prepare-request"
    )
    binding = _read_json(
        request.prepare_root / "binding-preflight.json", "prepare-binding"
    )
    default_shell = _read_json(
        request.prepare_root / "default-shell-terminal.json",
        "prepare-default-shell",
    )
    external = _read_json(
        request.prepare_root / "external-shell-terminal.json",
        "prepare-external-shell",
    )
    inventory = _read_json(
        request.prepare_root / "inventory-pre-move.json",
        "prepare-inventory",
    )
    device = _read_json(
        request.prepare_root / "source-default-target-device.json",
        "prepare-device",
    )
    terminal = _read_json(
        request.prepare_root / "terminal.json", "prepare-terminal"
    )
    if (
        prepared_request.get("format") != EVIDENCE_FORMAT
        or prepared_request.get("phase") != "prepare"
        or prepared_request.get("expected_repository_head")
        != request.expected_repository_head
        or prepared_request.get("prior_v4_manifest_sha256")
        != request.prior_v4_manifest_sha256
        or prepared_request.get("registration_shell_uuid")
        != request.registration_shell_uuid
        or prepared_request.get("default_shell_package_path_sha256")
        != shell_bindings.sha256_text(
            str(request.default_shell_package_path)
        )
        or prepared_request.get("registration_shell_package_path_sha256")
        != shell_bindings.sha256_text(
            str(request.registration_shell_package_path)
        )
        or binding.get("repository_head") != request.expected_repository_head
        or default_shell.get("config_sha256")
        != EXPECTED_V4_PARTIAL["config_sha256"]
        or default_shell.get("efi_sha256")
        != EXPECTED_V4_PARTIAL["efi_sha256"]
        or default_shell.get("qcow2_sha256")
        != EXPECTED_V4_PARTIAL["qcow2_sha256"]
        or external.get("state") != "absent"
        or inventory.get("canonical_inventory_sha256")
        != request.expected_inventory_sha256
        or inventory.get("registered_vm_count") != request.expected_vm_count
        or inventory.get("all_stopped") is not True
        or device
        != {
            "format": EVIDENCE_FORMAT,
            "same_device": True,
        }
        or terminal.get("outcome") != "move-ready"
        or terminal.get("registration_shell_uuid")
        != request.registration_shell_uuid
        or terminal.get("inventory_query_invocations") != 1
        or terminal.get("ui_move_invocations") != 0
        or terminal.get("update_invocations") != 0
        or terminal.get("next_external_action")
        != "one-utm-ui-move-to-authorized-package-path"
    ):
        raise ShellMoveBindingError("prepare-evidence-semantic-drift")
    return {
        "entries_verified": entries,
        "manifest_sha256": request.prepare_manifest_sha256,
        "outcome": "move-ready",
    }


def validate_default_partial_shell(
    request: CommonRequest,
) -> shell_bindings.BundleIdentity:
    if request.default_shell_package_path != (
        DEFAULT_UTM_STORAGE_ROOT
        / f"{shell_bindings.REQUIRED_SHELL_NAME}.utm"
    ):
        raise ShellMoveBindingError("default-shell-path-drift")
    identity = shell_bindings.read_bundle(
        request.default_shell_package_path, root_mode=0o755
    )
    _validate_partial_shell(request, identity)
    _validate_exact_bundle_members(request.default_shell_package_path, identity)
    return identity


def validate_external_partial_shell(
    request: CommonRequest,
) -> shell_bindings.BundleIdentity:
    identity = shell_bindings.read_bundle(
        request.registration_shell_package_path, root_mode=0o755
    )
    _validate_partial_shell(request, identity)
    _validate_exact_bundle_members(
        request.registration_shell_package_path, identity
    )
    return identity


def validate_final_shell(
    request: CommonRequest,
) -> shell_bindings.BundleIdentity:
    return shell_bindings.validate_registration_shell_bundle(
        request, request.registration_shell_uuid
    )


def _validate_partial_shell(
    request: CommonRequest,
    identity: shell_bindings.BundleIdentity,
) -> None:
    expected = EXPECTED_V4_PARTIAL
    if (
        identity.name != request.registration_shell_name
        or identity.name != shell_bindings.REQUIRED_SHELL_NAME
        or identity.uuid != request.registration_shell_uuid
        or identity.uuid != REQUIRED_SHELL_UUID
        or identity.config_sha256 != expected["config_sha256"]
        or identity.efi_sha256 != expected["efi_sha256"]
        or identity.qcow2_name != expected["qcow2_name"]
        or identity.qcow2_sha256 != expected["qcow2_sha256"]
    ):
        raise ShellMoveBindingError("v4-partial-shell-identity-drift")
    config = _read_plist(identity.config_path, "v4-partial-config")
    system = config.get("System")
    qemu = config.get("QEMU")
    network = config.get("Network")
    serial = config.get("Serial")
    if (
        config.get("Backend") != "QEMU"
        or config.get("ConfigurationVersion") != 4
        or config.get("Display") != []
        or not isinstance(system, dict)
        or system.get("Architecture") != "aarch64"
        or system.get("Target") != "virt"
        or system.get("MemorySize") != 512
        or system.get("CPUCount") != 0
        or not isinstance(qemu, dict)
        or qemu.get("Hypervisor") is not True
        or qemu.get("UEFIBoot") is not True
        or qemu.get("AdditionalArguments") != []
        or not isinstance(network, list)
        or len(network) != 1
        or not isinstance(network[0], dict)
        or network[0].get("Mode") != "Shared"
        or network[0].get("Hardware") != "virtio-net-pci"
        or network[0].get("PortForward") != []
        or not isinstance(serial, list)
        or len(serial) != 1
        or not isinstance(serial[0], dict)
        or serial[0].get("Mode") != "Ptty"
    ):
        raise ShellMoveBindingError("v4-partial-shell-config-drift")


def _validate_exact_bundle_members(
    root: Path, identity: shell_bindings.BundleIdentity
) -> None:
    try:
        root_members = {item.name for item in root.iterdir()}
        data_members = {item.name for item in (root / "Data").iterdir()}
    except OSError as exc:
        raise ShellMoveBindingError("bundle-members-unreadable") from exc
    if root_members != {"config.plist", "Data"} or data_members != {
        "efi_vars.fd",
        identity.qcow2_name,
    }:
        raise ShellMoveBindingError("bundle-members-drift")


def _validate_ui_only_move_surface(sdef_path: Path) -> None:
    try:
        root = element_tree.parse(sdef_path).getroot()
    except (OSError, element_tree.ParseError) as exc:
        raise ShellMoveBindingError("utm-sdef-invalid") from exc
    commands = [item.get("name") for item in root.iter("command")]
    if "move" in commands:
        raise ShellMoveBindingError("utm-sdef-unexpected-move-command")
    for required in ("make", "duplicate", "import", "export"):
        if commands.count(required) != 1:
            raise ShellMoveBindingError(
                f"utm-sdef-{required}-command-invalid"
            )


def _require_manifest_members(
    root: Path,
    manifest: Path,
    expected: tuple[str, ...],
    entries: int,
    label: str,
) -> None:
    try:
        lines = manifest.read_text(encoding="ascii").splitlines()
        files = {item.name for item in root.iterdir()}
    except (OSError, UnicodeDecodeError) as exc:
        raise ShellMoveBindingError(f"{label}-manifest-unreadable") from exc
    names = tuple(line[66:] for line in lines)
    if (
        entries != len(expected)
        or names != expected
        or files != {*expected, "files.sha256"}
    ):
        raise ShellMoveBindingError(f"{label}-manifest-members-invalid")


def _read_json(path: Path, label: str) -> dict[str, object]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise ShellMoveBindingError(f"{label}-invalid") from exc
    if not isinstance(value, dict):
        raise ShellMoveBindingError(f"{label}-root-invalid")
    return value


def _read_plist(path: Path, label: str) -> dict[str, object]:
    try:
        with path.open("rb") as source:
            value = plistlib.load(source)
    except (OSError, plistlib.InvalidFileException) as exc:
        raise ShellMoveBindingError(f"{label}-invalid") from exc
    if not isinstance(value, dict):
        raise ShellMoveBindingError(f"{label}-root-invalid")
    return value


def _run_git(repository_root: Path, arguments: tuple[str, ...]) -> bytes:
    observation = clone_control.SubprocessCommandRunner().run(
        ("/usr/bin/git", "-C", str(repository_root), *arguments), 30
    )
    try:
        clone_control._require_successful_observation(observation, "git")
    except clone_control.CloneControlError as exc:
        raise ShellMoveBindingError(str(exc)) from exc
    return observation.stdout.prefix
