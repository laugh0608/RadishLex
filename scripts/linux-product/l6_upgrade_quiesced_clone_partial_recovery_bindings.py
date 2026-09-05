#!/usr/bin/env python3
from __future__ import annotations

import json
import os
import stat
from pathlib import Path
from types import SimpleNamespace
from typing import Protocol

import l6_upgrade_quiesced_case_contract as case_contract
import l6_upgrade_quiesced_clone_bindings as clone_bindings
import l6_upgrade_quiesced_registration_shell_move_bindings as move_bindings
import l6_utm_clone_once as clone_control


EVIDENCE_FORMAT = (
    "radishlex-linux-l6-upgrade-quiesced-clone-partial-recovery-v1"
)
CONTROL_RELATIVE_PATH = Path(
    "scripts/linux-product/"
    "l6_upgrade_quiesced_clone_partial_recovery.py"
)
BINDING_RELATIVE_PATH = Path(
    "scripts/linux-product/"
    "l6_upgrade_quiesced_clone_partial_recovery_bindings.py"
)
PARTIAL_CLONE_MEMBERS = (
    "request.json",
    "binding-preflight.json",
    "source-snapshot-preflight.json",
    "registration-shell-preflight.json",
    "target-package-preclone.json",
    "source-and-shell-handles-preflight.json",
    "utmctl-list-preclone.json",
    "inventory-classification-preclone.json",
    "utmctl-clone.json",
    "utmctl-list-postclone.json",
    "target-package-postclone.json",
    "terminal.json",
)
MOVE_PREPARE_MEMBERS = (
    "request.json",
    "binding-preflight.json",
    "source-snapshot-preflight.json",
    "registration-shell-preflight.json",
    "default-target-preflight.json",
    "authorized-target-preflight.json",
    "source-default-authorized-device.json",
    "source-shell-default-handles-preflight.json",
    "utmctl-list-pre-move.json",
    "inventory-pre-move.json",
    "source-shell-default-handles-post-list.json",
    "source-snapshot-terminal.json",
    "registration-shell-terminal.json",
    "default-target-terminal.json",
    "authorized-target-terminal.json",
    "terminal.json",
)
MOVE_ADOPT_MEMBERS = (
    "request.json",
    "binding-preflight.json",
    "source-snapshot-preflight.json",
    "registration-shell-preflight.json",
    "default-target-preflight.json",
    "authorized-target-preflight.json",
    "source-authorized-device.json",
    "source-shell-authorized-handles-preflight.json",
    "utmctl-list-pre-adopt.json",
    "inventory-pre-adopt.json",
    "authorized-target-post-list.json",
    "utmctl-list-terminal.json",
    "inventory-terminal.json",
    "source-shell-authorized-handles-terminal.json",
    "source-snapshot-terminal.json",
    "registration-shell-terminal.json",
    "default-target-terminal.json",
    "authorized-target-terminal.json",
    "terminal.json",
)
MATERIALIZE_MEMBERS = (
    "request.json",
    "binding-preflight.json",
    "source-snapshot-preflight.json",
    "registration-shell-preflight.json",
    "default-target-preflight.json",
    "target-before-materialization.json",
    "source-target-device.json",
    "source-shell-target-handles-preflight.json",
    "utmctl-list-prematerialization.json",
    "inventory-prematerialization.json",
    "target-post-list.json",
    "source-shell-target-handles-post-list.json",
    "clonefile-efi.json",
    "clonefile-qcow2.json",
    "source-shell-target-handles-pre-replacement.json",
    "replace-target-efi.json",
    "source-shell-target-handles-between-replacements.json",
    "replace-target-qcow2.json",
    "target-after-materialization.json",
    "utmctl-list-terminal.json",
    "inventory-terminal.json",
    "source-shell-target-handles-terminal.json",
    "source-snapshot-terminal.json",
    "registration-shell-terminal.json",
    "default-target-terminal.json",
    "target-terminal.json",
    "terminal.json",
)


class RecoveryBindingError(ValueError):
    pass


class CommonRequest(Protocol):
    repository_root: Path
    expected_repository_head: str
    operator_asset_root: Path
    utm_documents_root: Path
    asset_retirement_delete_root: Path
    asset_retirement_delete_manifest_sha256: str
    predecessor_failure_root: Path
    predecessor_failure_manifest_sha256: str
    partial_clone_root: Path
    partial_clone_manifest_sha256: str
    source_snapshot_root: Path
    registration_shell_evidence_root: Path
    registration_shell_manifest_sha256: str
    registration_shell_uuid: str
    registration_shell_name: str
    registration_shell_package_path: Path
    target_uuid: str
    target_name: str
    default_target_package_path: Path
    target_package_path: Path
    expected_vm_count: int
    expected_inventory_sha256: str


class AdoptRequest(CommonRequest, Protocol):
    move_prepare_root: Path
    move_prepare_manifest_sha256: str


class MaterializeRequest(AdoptRequest, Protocol):
    move_adopt_root: Path
    move_adopt_manifest_sha256: str


def validate_prepare_bindings(request: CommonRequest) -> dict[str, object]:
    return {**_validate_common_bindings(request), "phase": "move-prepare"}


def validate_adopt_bindings(request: AdoptRequest) -> dict[str, object]:
    return {
        **_validate_common_bindings(request),
        "move_prepare_evidence": validate_move_prepare_evidence(request),
        "phase": "move-adopt",
    }


def validate_materialize_bindings(
    request: MaterializeRequest,
) -> dict[str, object]:
    return {
        **_validate_common_bindings(request),
        "move_adopt_evidence": validate_move_adopt_evidence(request),
        "phase": "materialize",
    }


def _validate_common_bindings(request: CommonRequest) -> dict[str, object]:
    head = _run_git(request.repository_root, ("rev-parse", "HEAD"))
    if head.decode("ascii").strip() != request.expected_repository_head:
        raise RecoveryBindingError("repository-head-drift")
    if _run_git(request.repository_root, ("status", "--porcelain")):
        raise RecoveryBindingError("repository-not-clean")
    case_contract.validate_repository_contract(request.repository_root)
    controls = validate_control_identity(request.repository_root)
    proxy = _front_door_proxy(request)
    retirement = clone_bindings.validate_post_retirement_baseline(proxy)
    predecessor = clone_bindings.validate_predecessor_failure(proxy)
    source = clone_bindings.validate_source_snapshot(proxy)
    registration = clone_bindings.validate_registration_shell(proxy)
    partial = validate_partial_clone_evidence(
        request, proxy, source, registration
    )
    return {
        **controls,
        "case_contract": "validated",
        "format": EVIDENCE_FORMAT,
        "partial_clone": partial,
        "post_retirement": retirement,
        "predecessor_failure": predecessor,
        "registration_shell": registration.as_json(),
        "repository_clean": True,
        "repository_head": request.expected_repository_head,
        "source_snapshot": source.as_json(),
    }


def validate_control_identity(repository_root: Path) -> dict[str, object]:
    control = repository_root / CONTROL_RELATIVE_PATH
    binding = repository_root / BINDING_RELATIVE_PATH
    for path, label in (
        (control, "partial-recovery-control"),
        (binding, "partial-recovery-binding"),
    ):
        _validate_committed_control(path, label)
    if Path(__file__).absolute() != binding:
        raise RecoveryBindingError("executed-binding-path-mismatch")
    sdef = (
        move_bindings.shell_bindings.UTM_BUNDLE_ROOT
        / "Contents/Resources/UTM.sdef"
    )
    move_bindings._validate_ui_only_move_surface(sdef)
    return {
        "binding_control_sha256": clone_bindings.sha256_file(binding),
        "move_surface": "utm-native-ui-only",
        "recovery_control_sha256": clone_bindings.sha256_file(control),
    }


def validate_partial_clone_evidence(
    request: CommonRequest,
    proxy: SimpleNamespace | None = None,
    source: clone_bindings.BundleIdentity | None = None,
    registration: clone_bindings.BundleIdentity | None = None,
) -> dict[str, object]:
    expected = _partial_contract()
    expected_root = request.operator_asset_root / str(
        expected["evidence_relative_path"]
    )
    if request.partial_clone_root != expected_root:
        raise RecoveryBindingError("partial-clone-root-drift")
    if request.partial_clone_manifest_sha256 != expected["manifest_sha256"]:
        raise RecoveryBindingError("partial-clone-manifest-request-drift")
    entries = _verify_manifest(
        request.partial_clone_root,
        request.partial_clone_manifest_sha256,
        PARTIAL_CLONE_MEMBERS,
        "partial-clone",
    )
    proxy = proxy or _front_door_proxy(request)
    source = source or clone_bindings.validate_source_snapshot(proxy)
    registration = registration or clone_bindings.validate_registration_shell(
        proxy
    )
    frozen_request = _read_json(request.partial_clone_root / "request.json")
    if frozen_request != _expected_partial_request(request):
        raise RecoveryBindingError("partial-clone-request-drift")
    binding = _read_json(
        request.partial_clone_root / "binding-preflight.json"
    )
    if binding != _expected_partial_binding(request):
        raise RecoveryBindingError("partial-clone-binding-drift")
    if _read_json(
        request.partial_clone_root / "source-snapshot-preflight.json"
    ) != source.as_json():
        raise RecoveryBindingError("partial-clone-source-drift")
    if _read_json(
        request.partial_clone_root / "registration-shell-preflight.json"
    ) != registration.as_json():
        raise RecoveryBindingError("partial-clone-registration-drift")
    absent = _absent_package_evidence(request.target_package_path)
    for name in ("target-package-preclone.json", "target-package-postclone.json"):
        if _read_json(request.partial_clone_root / name) != absent:
            raise RecoveryBindingError("partial-clone-authorized-target-drift")
    handles_argv = _lsof_argv(source, registration)
    handles = clone_bindings._observation_from_json(
        request.partial_clone_root / "source-and-shell-handles-preflight.json",
        handles_argv,
        "partial-clone-handles",
    )
    _require_zero_handles(handles, handles_argv)
    pre = _inventory_from_evidence(
        request.partial_clone_root / "utmctl-list-preclone.json",
        "partial-clone-pre",
    )
    classification = clone_bindings.validate_live_preclone_inventory(
        pre, proxy
    )
    if _read_json(
        request.partial_clone_root / "inventory-classification-preclone.json"
    ) != classification:
        raise RecoveryBindingError("partial-clone-classification-drift")
    clone = clone_bindings._observation_from_json(
        request.partial_clone_root / "utmctl-clone.json",
        (
            "utmctl",
            "clone",
            request.registration_shell_uuid,
            "--name",
            request.target_name,
        ),
        "partial-clone-command",
    )
    _require_silent_success(clone, "partial-clone-command")
    post = _inventory_from_evidence(
        request.partial_clone_root / "utmctl-list-postclone.json",
        "partial-clone-post",
    )
    _validate_frozen_postclone(pre, post, request)
    if _read_json(request.partial_clone_root / "terminal.json") != (
        _expected_partial_terminal(request)
    ):
        raise RecoveryBindingError("partial-clone-terminal-drift")
    return {
        "entries_verified": entries,
        "manifest_sha256": request.partial_clone_manifest_sha256,
        "outcome": expected["outcome"],
        "postclone_inventory_sha256": expected[
            "postclone_inventory_sha256"
        ],
        "replacement_count": expected["replacement_count"],
        "target_uuid": expected["target_uuid"],
    }


def validate_default_partial_target(
    request: CommonRequest,
    registration: clone_bindings.BundleIdentity,
    source: clone_bindings.BundleIdentity,
) -> clone_bindings.BundleIdentity:
    return _validate_partial_target(
        request,
        request.default_target_package_path,
        registration,
        source,
    )


def validate_authorized_partial_target(
    request: CommonRequest,
    registration: clone_bindings.BundleIdentity,
    source: clone_bindings.BundleIdentity,
) -> clone_bindings.BundleIdentity:
    return _validate_partial_target(
        request, request.target_package_path, registration, source
    )


def validate_materialized_target(
    request: CommonRequest,
    registration: clone_bindings.BundleIdentity,
    source: clone_bindings.BundleIdentity,
) -> clone_bindings.BundleIdentity:
    proxy = _target_proxy(request, request.target_package_path)
    identity = clone_bindings.validate_target_bundle(
        proxy,
        registration,
        request.target_uuid,
        materialized=True,
        source=source,
    )
    partial = _partial_contract()["default_package"]
    if (
        identity.config_sha256 != partial["config_sha256"]
        or identity.qcow2_name != partial["qcow2_name"]
    ):
        raise RecoveryBindingError("materialized-target-config-drift")
    _validate_exact_bundle_members(request.target_package_path, identity)
    return identity


def validate_live_inventory(
    inventory: tuple[clone_control.RegisteredVm, ...],
    request: CommonRequest,
) -> dict[str, object]:
    expected = _partial_contract()
    if len(inventory) != request.expected_vm_count:
        raise RecoveryBindingError("inventory-count-drift")
    if any(item.status != "stopped" for item in inventory):
        raise RecoveryBindingError("inventory-not-all-stopped")
    digest = clone_control.canonical_inventory_sha256(inventory)
    if digest != request.expected_inventory_sha256:
        raise RecoveryBindingError("inventory-sha256-drift")
    target = clone_control.RegisteredVm(
        request.target_uuid, "stopped", request.target_name
    )
    if sum(item == target for item in inventory) != 1:
        raise RecoveryBindingError("inventory-target-drift")
    baseline, remainder = clone_bindings._classify_managed_inventory(
        inventory
    )
    foreign = tuple(item for item in remainder if item.uuid != request.target_uuid)
    if (
        len(baseline) + 1 != expected["postclone_managed_vm_count"]
        or len(foreign) != expected["foreign_vm_count"]
    ):
        raise RecoveryBindingError("inventory-overlay-classification-drift")
    return {
        "all_registered_vms_stopped": True,
        "foreign_vm_count": len(foreign),
        "inventory_sha256": digest,
        "managed_vm_count": len(baseline) + 1,
        "registered_vm_count": len(inventory),
        "target_uuid": request.target_uuid,
    }


def validate_move_prepare_evidence(
    request: AdoptRequest,
) -> dict[str, object]:
    entries = _verify_manifest(
        request.move_prepare_root,
        request.move_prepare_manifest_sha256,
        MOVE_PREPARE_MEMBERS,
        "move-prepare",
    )
    prepared = _read_json(request.move_prepare_root / "request.json")
    if not _same_common_request(prepared, request, "move-prepare"):
        raise RecoveryBindingError("move-prepare-request-drift")
    _validate_phase_binding(
        request.move_prepare_root / "binding-preflight.json",
        prepared,
        "move-prepare",
    )
    proxy = _front_door_proxy(request)
    source = clone_bindings.validate_source_snapshot(proxy)
    registration = clone_bindings.validate_registration_shell(proxy)
    partial = _expected_partial_identity(request)
    for name in (
        "source-snapshot-preflight.json",
        "source-snapshot-terminal.json",
    ):
        if _read_json(request.move_prepare_root / name) != source.as_json():
            raise RecoveryBindingError("move-prepare-source-drift")
    for name in (
        "registration-shell-preflight.json",
        "registration-shell-terminal.json",
    ):
        if _read_json(request.move_prepare_root / name) != (
            registration.as_json()
        ):
            raise RecoveryBindingError("move-prepare-registration-drift")
    for name in ("default-target-preflight.json", "default-target-terminal.json"):
        if _read_json(request.move_prepare_root / name) != partial:
            raise RecoveryBindingError("move-prepare-default-target-drift")
    absent = _absent_package_evidence(request.target_package_path)
    for name in (
        "authorized-target-preflight.json",
        "authorized-target-terminal.json",
    ):
        if _read_json(request.move_prepare_root / name) != absent:
            raise RecoveryBindingError("move-prepare-authorized-target-drift")
    if _read_json(
        request.move_prepare_root / "source-default-authorized-device.json"
    ) != {"format": EVIDENCE_FORMAT, "same_device": True}:
        raise RecoveryBindingError("move-prepare-device-drift")
    handles_argv = _lsof_argv_for_target_path(
        source,
        registration,
        request.default_target_package_path,
    )
    for name in (
        "source-shell-default-handles-preflight.json",
        "source-shell-default-handles-post-list.json",
    ):
        handles = clone_bindings._observation_from_json(
            request.move_prepare_root / name,
            handles_argv,
            "move-prepare-handles",
        )
        _require_zero_handles(handles, handles_argv)
    terminal = _read_json(request.move_prepare_root / "terminal.json")
    if terminal != _expected_move_prepare_terminal(request):
        raise RecoveryBindingError("move-prepare-terminal-drift")
    inventory = _inventory_from_evidence(
        request.move_prepare_root / "utmctl-list-pre-move.json",
        "move-prepare",
    )
    validate_live_inventory(inventory, request)
    if _read_json(
        request.move_prepare_root / "inventory-pre-move.json"
    ) != validate_live_inventory(inventory, request):
        raise RecoveryBindingError("move-prepare-inventory-drift")
    return {
        "entries_verified": entries,
        "manifest_sha256": request.move_prepare_manifest_sha256,
        "outcome": "move-ready",
    }


def validate_move_adopt_evidence(
    request: MaterializeRequest,
) -> dict[str, object]:
    prepare = validate_move_prepare_evidence(request)
    entries = _verify_manifest(
        request.move_adopt_root,
        request.move_adopt_manifest_sha256,
        MOVE_ADOPT_MEMBERS,
        "move-adopt",
    )
    adopted = _read_json(request.move_adopt_root / "request.json")
    if (
        not _same_common_request(adopted, request, "move-adopt")
        or adopted.get("move_prepare_manifest_sha256")
        != request.move_prepare_manifest_sha256
        or adopted.get("move_prepare_root_sha256")
        != clone_bindings.sha256_text(str(request.move_prepare_root))
    ):
        raise RecoveryBindingError("move-adopt-request-drift")
    _validate_phase_binding(
        request.move_adopt_root / "binding-preflight.json",
        adopted,
        "move-adopt",
    )
    proxy = _front_door_proxy(request)
    source = clone_bindings.validate_source_snapshot(proxy)
    registration = clone_bindings.validate_registration_shell(proxy)
    partial = _expected_partial_identity(request)
    absent = _absent_package_evidence(request.default_target_package_path)
    for name in (
        "source-snapshot-preflight.json",
        "source-snapshot-terminal.json",
    ):
        if _read_json(request.move_adopt_root / name) != source.as_json():
            raise RecoveryBindingError("move-adopt-source-drift")
    for name in (
        "registration-shell-preflight.json",
        "registration-shell-terminal.json",
    ):
        if _read_json(request.move_adopt_root / name) != (
            registration.as_json()
        ):
            raise RecoveryBindingError("move-adopt-registration-drift")
    for name in ("default-target-preflight.json", "default-target-terminal.json"):
        if _read_json(request.move_adopt_root / name) != absent:
            raise RecoveryBindingError("move-adopt-default-target-drift")
    for name in (
        "authorized-target-preflight.json",
        "authorized-target-post-list.json",
        "authorized-target-terminal.json",
    ):
        if _read_json(request.move_adopt_root / name) != partial:
            raise RecoveryBindingError("move-adopt-authorized-target-drift")
    if _read_json(
        request.move_adopt_root / "source-authorized-device.json"
    ) != {"format": EVIDENCE_FORMAT, "same_device": True}:
        raise RecoveryBindingError("move-adopt-device-drift")
    handles_argv = _lsof_argv_for_target_path(
        source, registration, request.target_package_path
    )
    for name in (
        "source-shell-authorized-handles-preflight.json",
        "source-shell-authorized-handles-terminal.json",
    ):
        handles = clone_bindings._observation_from_json(
            request.move_adopt_root / name,
            handles_argv,
            "move-adopt-handles",
        )
        _require_zero_handles(handles, handles_argv)
    terminal = _read_json(request.move_adopt_root / "terminal.json")
    if terminal != _expected_move_adopt_terminal(request):
        raise RecoveryBindingError("move-adopt-terminal-drift")
    inventories: list[tuple[clone_control.RegisteredVm, ...]] = []
    for observation_name, inventory_name in (
        ("utmctl-list-pre-adopt.json", "inventory-pre-adopt.json"),
        ("utmctl-list-terminal.json", "inventory-terminal.json"),
    ):
        inventory = _inventory_from_evidence(
            request.move_adopt_root / observation_name, "move-adopt"
        )
        evidence = validate_live_inventory(inventory, request)
        if _read_json(request.move_adopt_root / inventory_name) != evidence:
            raise RecoveryBindingError("move-adopt-inventory-drift")
        inventories.append(inventory)
    if inventories[0] != inventories[1]:
        raise RecoveryBindingError("move-adopt-inventory-query-drift")
    return {
        "entries_verified": entries,
        "manifest_sha256": request.move_adopt_manifest_sha256,
        "outcome": "move-adopted",
        "prepare_evidence": prepare,
    }


def _validate_partial_target(
    request: CommonRequest,
    path: Path,
    registration: clone_bindings.BundleIdentity,
    source: clone_bindings.BundleIdentity,
) -> clone_bindings.BundleIdentity:
    proxy = _target_proxy(request, path)
    identity = clone_bindings.validate_target_bundle(
        proxy,
        registration,
        request.target_uuid,
        materialized=False,
    )
    expected = _partial_contract()["default_package"]
    if (
        identity.config_sha256 != expected["config_sha256"]
        or identity.efi_sha256 != expected["efi_sha256"]
        or identity.qcow2_name != expected["qcow2_name"]
        or identity.qcow2_sha256 != expected["qcow2_sha256"]
    ):
        raise RecoveryBindingError("partial-target-identity-drift")
    if (
        identity.efi_sha256 == source.efi_sha256
        or identity.qcow2_sha256 == source.qcow2_sha256
    ):
        raise RecoveryBindingError("partial-target-already-materialized")
    _validate_exact_bundle_members(path, identity)
    return identity


def _validate_frozen_postclone(
    pre: tuple[clone_control.RegisteredVm, ...],
    post: tuple[clone_control.RegisteredVm, ...],
    request: CommonRequest,
) -> None:
    expected = _partial_contract()
    if (
        len(pre) != expected["preclone_registered_vm_count"]
        or clone_control.canonical_inventory_sha256(pre)
        != expected["preclone_inventory_sha256"]
        or len(post) != expected["postclone_registered_vm_count"]
        or clone_control.canonical_inventory_sha256(post)
        != expected["postclone_inventory_sha256"]
    ):
        raise RecoveryBindingError("partial-clone-inventory-drift")
    baseline = {item.uuid: item for item in pre}
    if any(
        next((x for x in post if x.uuid == key), None) != value
        for key, value in baseline.items()
    ):
        raise RecoveryBindingError("partial-clone-overlay-drift")
    added = tuple(item for item in post if item.uuid not in baseline)
    if added != (
        clone_control.RegisteredVm(
            request.target_uuid, "stopped", request.target_name
        ),
    ):
        raise RecoveryBindingError("partial-clone-target-drift")


def _front_door_proxy(request: CommonRequest) -> SimpleNamespace:
    partial = _partial_contract()
    return SimpleNamespace(
        repository_root=request.repository_root,
        expected_repository_head=partial["repository_head"],
        operator_asset_root=request.operator_asset_root,
        utm_documents_root=request.utm_documents_root,
        asset_retirement_delete_root=request.asset_retirement_delete_root,
        asset_retirement_delete_manifest_sha256=(
            request.asset_retirement_delete_manifest_sha256
        ),
        predecessor_failure_root=request.predecessor_failure_root,
        predecessor_failure_manifest_sha256=(
            request.predecessor_failure_manifest_sha256
        ),
        source_snapshot_root=request.source_snapshot_root,
        registration_shell_evidence_root=(
            request.registration_shell_evidence_root
        ),
        registration_shell_manifest_sha256=(
            request.registration_shell_manifest_sha256
        ),
        registration_shell_uuid=request.registration_shell_uuid,
        registration_shell_name=request.registration_shell_name,
        registration_shell_package_path=(
            request.registration_shell_package_path
        ),
        target_name=request.target_name,
        target_package_path=request.target_package_path,
        expected_vm_count=partial["preclone_registered_vm_count"],
        expected_preclone_inventory_sha256=(
            partial["preclone_inventory_sha256"]
        ),
    )


def _target_proxy(request: CommonRequest, path: Path) -> SimpleNamespace:
    return SimpleNamespace(
        target_name=request.target_name,
        target_package_path=path,
    )


def _expected_partial_request(request: CommonRequest) -> dict[str, object]:
    partial = _partial_contract()
    return {
        "asset_retirement_delete_manifest_sha256": (
            request.asset_retirement_delete_manifest_sha256
        ),
        "asset_retirement_delete_root_sha256": clone_bindings.sha256_text(
            str(request.asset_retirement_delete_root)
        ),
        "attempt_id": partial["attempt_id"],
        "authorization": {
            "dedicated_registration_shell": True,
            "no_automatic_retry_delete_start_or_rollback": True,
            "upgrade_quiesced_clone": True,
        },
        "clone_timeout_seconds": 120,
        "command_timeout_seconds": 15,
        "expected_preclone_inventory_sha256": partial[
            "preclone_inventory_sha256"
        ],
        "expected_repository_head": partial["repository_head"],
        "expected_vm_count": partial["preclone_registered_vm_count"],
        "format": clone_bindings.EVIDENCE_FORMAT,
        "operator_asset_root_sha256": clone_bindings.sha256_text(
            str(request.operator_asset_root)
        ),
        "predecessor_failure_manifest_sha256": (
            request.predecessor_failure_manifest_sha256
        ),
        "predecessor_failure_root_sha256": clone_bindings.sha256_text(
            str(request.predecessor_failure_root)
        ),
        "registration_shell_evidence_root_sha256": (
            clone_bindings.sha256_text(str(request.registration_shell_evidence_root))
        ),
        "registration_shell_manifest_sha256": (
            request.registration_shell_manifest_sha256
        ),
        "registration_shell_name": request.registration_shell_name,
        "registration_shell_package_path_sha256": clone_bindings.sha256_text(
            str(request.registration_shell_package_path)
        ),
        "registration_shell_uuid": request.registration_shell_uuid,
        "source_snapshot_path_sha256": clone_bindings.sha256_text(
            str(request.source_snapshot_root)
        ),
        "target_name": request.target_name,
        "target_package_path_sha256": clone_bindings.sha256_text(
            str(request.target_package_path)
        ),
        "utm_documents_root_sha256": clone_bindings.sha256_text(
            str(request.utm_documents_root)
        ),
    }


def _expected_partial_binding(request: CommonRequest) -> dict[str, object]:
    partial = _partial_contract()
    baseline = case_contract.EXPECTED_CLONE_FRONT_DOOR["preclone_baseline"]
    predecessor = case_contract.EXPECTED_CLONE_FRONT_DOOR[
        "predecessor_failure"
    ]
    return {
        "asset_retirement_delete_entries_verified": 18,
        "asset_retirement_delete_manifest_sha256": (
            request.asset_retirement_delete_manifest_sha256
        ),
        "case_contract": "validated",
        "control_sha256": partial["control_sha256"],
        "dedicated_registration_shell": True,
        "deleted_packages_absent": 3,
        "format": clone_bindings.EVIDENCE_FORMAT,
        "post_retirement_inventory_sha256": baseline["inventory_sha256"],
        "post_retirement_vm_count": baseline["registered_vm_count"],
        "predecessor_failure_entries_verified": 8,
        "predecessor_failure_foreign_vm_count": predecessor[
            "foreign_vm_count"
        ],
        "predecessor_failure_manifest_sha256": (
            request.predecessor_failure_manifest_sha256
        ),
        "predecessor_failure_outcome": predecessor["outcome"],
        "registration_shell_entries_verified": 1,
        "registration_shell_manifest_sha256": (
            request.registration_shell_manifest_sha256
        ),
        "repository_clean": True,
        "repository_head": partial["repository_head"],
    }


def _expected_partial_terminal(request: CommonRequest) -> dict[str, object]:
    partial = _partial_contract()
    return {
        "automatic_delete": "not-performed",
        "automatic_retry": "not-performed",
        "automatic_rollback": "not-performed",
        "automatic_start": "not-performed",
        "clone_command_exit_code": 0,
        "clone_command_timed_out": False,
        "clone_invocations": partial["clone_invocations"],
        "format": clone_bindings.EVIDENCE_FORMAT,
        "guest_exec": "not-performed",
        "input_transfer": "not-performed",
        "operation_id": "not-generated",
        "outcome": partial["outcome"],
        "preclone_inventory": {
            "all_registered_vms_stopped": True,
            "foreign_overlay_allowed": True,
            "foreign_vm_count": partial["foreign_vm_count"],
            "inventory_sha256": partial["preclone_inventory_sha256"],
            "managed_vm_count": 5,
            "registered_vm_count": partial["preclone_registered_vm_count"],
        },
        "reason": partial["reason"],
        "replacement_count": partial["replacement_count"],
        "target_name": request.target_name,
        "target_package_state": "absent",
        "target_uuid": None,
        "transaction": "not-performed",
    }


def _expected_move_prepare_terminal(request: CommonRequest) -> dict[str, object]:
    return {
        "automatic_delete": "not-performed",
        "automatic_retry": "not-performed",
        "automatic_rollback": "not-performed",
        "automatic_start": "not-performed",
        "clone_invocations": 0,
        "copy_invocations": 0,
        "format": EVIDENCE_FORMAT,
        "guest_exec": "not-performed",
        "inventory_query_invocations": 1,
        "materialization_invocations": 0,
        "move_invocations": 0,
        "next_external_action": "one-utm-ui-move-to-authorized-target-path",
        "operation_id": "not-generated",
        "outcome": "move-ready",
        "reason": "one-utm-ui-move-may-be-authorized-separately",
        "replacement_count": 0,
        "target_name": request.target_name,
        "target_uuid": request.target_uuid,
        "transaction": "not-performed",
    }


def _expected_move_adopt_terminal(request: CommonRequest) -> dict[str, object]:
    return {
        "automatic_delete": "not-performed",
        "automatic_retry": "not-performed",
        "automatic_rollback": "not-performed",
        "automatic_start": "not-performed",
        "clone_invocations": 0,
        "copy_invocations": 0,
        "format": EVIDENCE_FORMAT,
        "guest_exec": "not-performed",
        "inventory_query_invocations": 2,
        "materialization_invocations": 0,
        "move_invocations": 0,
        "next_external_action": "none",
        "operation_id": "not-generated",
        "outcome": "move-adopted",
        "reason": "utm-ui-move-adopted-with-partial-bytes-unchanged",
        "replacement_count": 0,
        "target_name": request.target_name,
        "target_uuid": request.target_uuid,
        "transaction": "not-performed",
    }


def _same_common_request(
    value: dict[str, object], request: CommonRequest, phase: str
) -> bool:
    authorization = {
        "move-prepare": {
            "emit_one_utm_ui_move": True,
            "move_prepare": True,
            "no_move_materialize_start_clone_delete_retry_guest_or_transaction": True,
            "one_inventory_query": True,
        },
        "move-adopt": {
            "adopt_one_utm_ui_move_result": True,
            "no_materialize_start_clone_delete_retry_guest_or_transaction": True,
            "two_inventory_queries": True,
        },
    }.get(phase)
    if authorization is None:
        return False
    expected_keys = {
        "asset_retirement_delete_manifest_sha256",
        "attempt_id",
        "authorization",
        "command_timeout_seconds",
        "default_target_package_path_sha256",
        "expected_inventory_sha256",
        "expected_repository_head",
        "expected_vm_count",
        "format",
        "operator_asset_root_sha256",
        "partial_clone_manifest_sha256",
        "partial_clone_root_sha256",
        "phase",
        "predecessor_failure_manifest_sha256",
        "registration_shell_manifest_sha256",
        "target_name",
        "target_package_path_sha256",
        "target_uuid",
        "utm_documents_root_sha256",
    }
    if phase == "move-adopt":
        expected_keys |= {
            "move_prepare_manifest_sha256",
            "move_prepare_root_sha256",
        }
    repository_head = value.get("expected_repository_head")
    attempt_id = value.get("attempt_id")
    timeout = value.get("command_timeout_seconds")
    return (
        set(value) == expected_keys
        and value.get("authorization") == authorization
        and isinstance(repository_head, str)
        and len(repository_head) == 40
        and all(
            character in "0123456789abcdef"
            for character in repository_head
        )
        and isinstance(attempt_id, str)
        and bool(attempt_id)
        and len(attempt_id) <= 96
        and all(
            character.islower()
            or character.isdigit()
            or character == "-"
            for character in attempt_id
        )
        and isinstance(timeout, int)
        and 1 <= timeout <= 60
        and value.get("format") == EVIDENCE_FORMAT
        and value.get("phase") == phase
        and value.get("asset_retirement_delete_manifest_sha256")
        == request.asset_retirement_delete_manifest_sha256
        and value.get("predecessor_failure_manifest_sha256")
        == request.predecessor_failure_manifest_sha256
        and value.get("registration_shell_manifest_sha256")
        == request.registration_shell_manifest_sha256
        and value.get("expected_vm_count") == request.expected_vm_count
        and value.get("expected_inventory_sha256")
        == request.expected_inventory_sha256
        and value.get("operator_asset_root_sha256")
        == clone_bindings.sha256_text(str(request.operator_asset_root))
        and value.get("partial_clone_manifest_sha256")
        == request.partial_clone_manifest_sha256
        and value.get("partial_clone_root_sha256")
        == clone_bindings.sha256_text(str(request.partial_clone_root))
        and value.get("target_name") == request.target_name
        and value.get("target_uuid") == request.target_uuid
        and value.get("default_target_package_path_sha256")
        == clone_bindings.sha256_text(str(request.default_target_package_path))
        and value.get("target_package_path_sha256")
        == clone_bindings.sha256_text(str(request.target_package_path))
        and value.get("utm_documents_root_sha256")
        == clone_bindings.sha256_text(str(request.utm_documents_root))
    )


def _validate_phase_binding(
    path: Path, phase_request: dict[str, object], phase: str
) -> None:
    binding = _read_json(path)
    partial = binding.get("partial_clone")
    if (
        binding.get("format") != EVIDENCE_FORMAT
        or binding.get("phase") != phase
        or binding.get("case_contract") != "validated"
        or binding.get("repository_clean") is not True
        or binding.get("repository_head")
        != phase_request.get("expected_repository_head")
        or not isinstance(partial, dict)
        or partial.get("manifest_sha256")
        != phase_request.get("partial_clone_manifest_sha256")
        or partial.get("outcome") != "state-indeterminate"
        or partial.get("replacement_count") != 0
    ):
        raise RecoveryBindingError(f"{phase}-binding-drift")


def _expected_partial_identity(request: CommonRequest) -> dict[str, object]:
    expected = _partial_contract()["default_package"]
    return {
        "config_sha256": expected["config_sha256"],
        "efi_sha256": expected["efi_sha256"],
        "name": request.target_name,
        "network": [],
        "qcow2_name": expected["qcow2_name"],
        "qcow2_sha256": expected["qcow2_sha256"],
        "uuid": request.target_uuid,
    }


def _lsof_argv_for_target_path(
    source: clone_bindings.BundleIdentity,
    registration: clone_bindings.BundleIdentity,
    target_root: Path,
) -> tuple[str, ...]:
    expected = _partial_contract()["default_package"]
    paths = [
        source.config_path,
        source.efi_path,
        source.qcow2_path,
        registration.config_path,
        registration.efi_path,
        registration.qcow2_path,
        target_root / "config.plist",
        target_root / "Data/efi_vars.fd",
        target_root / "Data" / str(expected["qcow2_name"]),
    ]
    return (
        "/usr/sbin/lsof",
        "-n",
        "-P",
        "-F",
        "ctfn",
        *(str(path) for path in paths),
    )


def _partial_contract() -> dict[str, object]:
    return case_contract.EXPECTED_CLONE_FRONT_DOOR["partial_recovery"][
        "partial_clone"
    ]


def _inventory_from_evidence(
    path: Path, label: str
) -> tuple[clone_control.RegisteredVm, ...]:
    observation = clone_bindings._observation_from_json(
        path, ("utmctl", "list"), label
    )
    try:
        return clone_control.parse_utmctl_list(observation)
    except clone_control.CloneControlError as exc:
        raise RecoveryBindingError(f"{label}-inventory-invalid") from exc


def _verify_manifest(
    root: Path,
    expected_sha256: str,
    members: tuple[str, ...],
    label: str,
) -> int:
    manifest = root / "files.sha256"
    if clone_bindings.sha256_file(manifest) != expected_sha256:
        raise RecoveryBindingError(f"{label}-manifest-drift")
    try:
        entries = clone_control._verify_sha256_manifest(root, manifest)
        lines = manifest.read_text(encoding="ascii").splitlines()
        files = {item.name for item in root.iterdir()}
    except (clone_control.CloneControlError, OSError, UnicodeDecodeError) as exc:
        raise RecoveryBindingError(f"{label}-evidence-invalid") from exc
    if (
        entries != len(members)
        or tuple(line[66:] for line in lines) != members
        or files != {*members, "files.sha256"}
    ):
        raise RecoveryBindingError(f"{label}-member-set-drift")
    return entries


def _absent_package_evidence(path: Path) -> dict[str, object]:
    return {
        "package_name": path.name,
        "package_path_sha256": clone_bindings.sha256_text(str(path)),
        "state": "absent",
    }


def _lsof_argv(
    *identities: clone_bindings.BundleIdentity,
) -> tuple[str, ...]:
    paths: list[str] = []
    for identity in identities:
        paths.extend(
            str(path)
            for path in (
                identity.config_path,
                identity.efi_path,
                identity.qcow2_path,
            )
        )
    return ("/usr/sbin/lsof", "-n", "-P", "-F", "ctfn", *paths)


def _require_zero_handles(
    observation: clone_control.CommandObservation,
    expected_argv: tuple[str, ...],
) -> None:
    if observation.argv != expected_argv:
        raise RecoveryBindingError("lsof-argv-drift")
    if (
        observation.timed_out
        or observation.exit_code != 1
        or observation.stdout.total_bytes != 0
        or observation.stderr.total_bytes != 0
        or observation.stdout.truncated
        or observation.stderr.truncated
    ):
        raise RecoveryBindingError("disk-open-handles-or-lsof-indeterminate")


def _require_silent_success(
    observation: clone_control.CommandObservation, label: str
) -> None:
    if (
        observation.timed_out
        or observation.exit_code != 0
        or observation.stdout.total_bytes != 0
        or observation.stderr.total_bytes != 0
    ):
        raise RecoveryBindingError(f"{label}-failed")


def _validate_exact_bundle_members(
    root: Path, identity: clone_bindings.BundleIdentity
) -> None:
    try:
        root_members = {item.name for item in root.iterdir()}
        data_members = {item.name for item in (root / "Data").iterdir()}
    except OSError as exc:
        raise RecoveryBindingError("target-bundle-members-unreadable") from exc
    if root_members != {"config.plist", "Data"} or data_members != {
        "efi_vars.fd",
        identity.qcow2_name,
    }:
        raise RecoveryBindingError("target-bundle-members-drift")


def _validate_committed_control(path: Path, label: str) -> None:
    try:
        metadata = path.lstat()
    except OSError as exc:
        raise RecoveryBindingError(f"{label}-unavailable") from exc
    if (
        not stat.S_ISREG(metadata.st_mode)
        or stat.S_ISLNK(metadata.st_mode)
        or metadata.st_nlink != 1
        or stat.S_IMODE(metadata.st_mode) & 0o022
        or metadata.st_uid != os.getuid()
        or metadata.st_gid != os.getgid()
    ):
        raise RecoveryBindingError(f"{label}-identity-invalid")


def _read_json(path: Path) -> dict[str, object]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise RecoveryBindingError(f"json-invalid:{path.name}") from exc
    if not isinstance(value, dict):
        raise RecoveryBindingError(f"json-root-invalid:{path.name}")
    return value


def _run_git(repository_root: Path, arguments: tuple[str, ...]) -> bytes:
    observation = clone_control.SubprocessCommandRunner().run(
        ("/usr/bin/git", "-C", str(repository_root), *arguments), 30
    )
    try:
        clone_control._require_successful_observation(observation, "git")
    except clone_control.CloneControlError as exc:
        raise RecoveryBindingError(str(exc)) from exc
    return observation.stdout.prefix
