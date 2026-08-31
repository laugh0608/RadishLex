#!/usr/bin/env python3
from __future__ import annotations

import argparse
import hashlib
import json
import os
import plistlib
import re
import stat
import sys
import uuid
from dataclasses import dataclass
from pathlib import Path, PurePosixPath
from typing import Callable, Protocol

import l6_utm_clone_once as clone_control


EVIDENCE_FORMAT = "radishlex-linux-l6-asset-retirement-prepare-v1"
ALLOWLIST_FORMAT = "radishlex-linux-l6-asset-retirement-allowlist-v1"
CONTROL_RELATIVE_PATH = Path(
    "scripts/linux-product/l6_utm_asset_retirement_prepare.py"
)
ALLOWLIST_RELATIVE_PATH = Path(
    "packaging/linux/l6-asset-retirement-allowlist.json"
)
EXIT_PREPARED = 0
EXIT_PRECONDITION_REJECTED = 11
HEX_40 = re.compile(r"[0-9a-f]{40}")
HEX_64 = re.compile(r"[0-9a-f]{64}")
SAFE_ID = re.compile(r"[a-z0-9][a-z0-9-]{0,95}")
STORAGES = frozenset(("operator", "utm-documents"))
SEMANTICS = frozenset(
    (
        "snapshot-source-vm",
        "release-pair",
        "repair-terminal",
        "opaque-hash",
        "frozen-disk-identity",
        "utm-start-terminal",
        "maintenance-repair-terminal",
        "install-prepared-failed-closed-terminal",
    )
)


class RetirementPrepareError(ValueError):
    pass


@dataclass(frozen=True)
class EvidenceAnchor:
    storage: str
    relative_path: str
    sha256: str
    mode: int
    semantic: str


@dataclass(frozen=True)
class RetirementAsset:
    asset_id: str
    storage: str
    relative_path: str
    name: str
    uuid: str
    role: str
    config_sha256: str
    efi_sha256: str
    qcow2_sha256: str
    qcow2_name: str
    evidence_anchors: tuple[EvidenceAnchor, ...]


@dataclass(frozen=True)
class RetirementBatch:
    batch_id: str
    asset_ids: tuple[str, ...]
    accounting_gib: str
    assets: tuple[RetirementAsset, ...]


@dataclass(frozen=True)
class RetirementAllowlist:
    operator_asset_root: Path
    utm_documents_root: Path
    batches: dict[str, RetirementBatch]


@dataclass(frozen=True)
class BindingResult:
    evidence: dict[str, object]
    batch: RetirementBatch


@dataclass(frozen=True)
class PrepareRequest:
    repository_root: Path
    expected_repository_head: str
    output_root: Path
    attempt_id: str
    operator_asset_root: Path
    utm_documents_root: Path
    expected_allowlist_sha256: str
    batch_id: str
    expected_vm_count: int
    expected_inventory_sha256: str
    command_timeout_seconds: int
    authorized_read_only_prepare: bool
    authorized_one_utmctl_list: bool
    authorized_two_handle_rounds: bool
    authorized_no_delete_start_clone_move_or_guest: bool

    def validate(self) -> None:
        for path, label in (
            (self.repository_root, "repository-root"),
            (self.output_root, "output-root"),
            (self.operator_asset_root, "operator-asset-root"),
            (self.utm_documents_root, "utm-documents-root"),
        ):
            if not path.is_absolute() or ".." in path.parts:
                raise RetirementPrepareError(f"{label}-must-be-absolute-normalized")
        if self.output_root.parent != self.operator_asset_root:
            raise RetirementPrepareError("output-root-parent-must-be-operator-root")
        if clone_control._path_is_within(self.output_root, self.repository_root):
            raise RetirementPrepareError("output-root-must-be-outside-repository")
        if clone_control._paths_overlap(
            self.operator_asset_root, self.utm_documents_root
        ):
            raise RetirementPrepareError("asset-roots-must-not-overlap")
        if not HEX_40.fullmatch(self.expected_repository_head):
            raise RetirementPrepareError("expected-repository-head-invalid")
        if not HEX_64.fullmatch(self.expected_allowlist_sha256):
            raise RetirementPrepareError("expected-allowlist-sha256-invalid")
        if not HEX_64.fullmatch(self.expected_inventory_sha256):
            raise RetirementPrepareError("expected-inventory-sha256-invalid")
        if not SAFE_ID.fullmatch(self.attempt_id):
            raise RetirementPrepareError("attempt-id-invalid")
        if not SAFE_ID.fullmatch(self.batch_id):
            raise RetirementPrepareError("batch-id-invalid")
        if not 1 <= self.expected_vm_count <= 128:
            raise RetirementPrepareError("expected-vm-count-out-of-range")
        if not 1 <= self.command_timeout_seconds <= 60:
            raise RetirementPrepareError("command-timeout-seconds-out-of-range")
        if not self.authorized_read_only_prepare:
            raise RetirementPrepareError("read-only-prepare-authorization-required")
        if not self.authorized_one_utmctl_list:
            raise RetirementPrepareError("one-utmctl-list-authorization-required")
        if not self.authorized_two_handle_rounds:
            raise RetirementPrepareError("two-handle-rounds-authorization-required")
        if not self.authorized_no_delete_start_clone_move_or_guest:
            raise RetirementPrepareError(
                "no-delete-start-clone-move-or-guest-authorization-required"
            )

    def as_json(self) -> dict[str, object]:
        return {
            "attempt_id": self.attempt_id,
            "authorization": {
                "no_delete_start_clone_move_or_guest": True,
                "one_utmctl_list": True,
                "read_only_prepare": True,
                "two_handle_rounds": True,
            },
            "batch_id": self.batch_id,
            "command_timeout_seconds": self.command_timeout_seconds,
            "expected_allowlist_sha256": self.expected_allowlist_sha256,
            "expected_inventory_sha256": self.expected_inventory_sha256,
            "expected_repository_head": self.expected_repository_head,
            "expected_vm_count": self.expected_vm_count,
            "format": EVIDENCE_FORMAT,
            "operator_asset_root_sha256": clone_control._sha256_text(
                str(self.operator_asset_root)
            ),
            "utm_documents_root_sha256": clone_control._sha256_text(
                str(self.utm_documents_root)
            ),
        }


@dataclass(frozen=True)
class BundleIdentity:
    asset_id: str
    config_path: Path
    efi_path: Path
    qcow2_path: Path
    config_sha256: str
    efi_sha256: str
    qcow2_sha256: str
    config_size: int
    efi_size: int
    qcow2_size: int

    def as_json(self) -> dict[str, object]:
        return {
            "asset_id": self.asset_id,
            "config_sha256": self.config_sha256,
            "config_size": self.config_size,
            "efi_sha256": self.efi_sha256,
            "efi_size": self.efi_size,
            "qcow2_sha256": self.qcow2_sha256,
            "qcow2_size": self.qcow2_size,
        }


@dataclass(frozen=True)
class PrepareResult:
    outcome: str
    exit_code: int
    evidence_root: Path
    manifest_sha256: str
    inventory_queries: int
    handle_rounds: int
    asset_hash_rounds: int


class CommandRunner(Protocol):
    def run(
        self, argv: tuple[str, ...], timeout_seconds: int
    ) -> clone_control.CommandObservation: ...


BindingValidator = Callable[[PrepareRequest], BindingResult]


def run_prepare(
    request: PrepareRequest,
    *,
    runner: CommandRunner | None = None,
    binding_validator: BindingValidator | None = None,
) -> PrepareResult:
    request.validate()
    writer = clone_control.EvidenceWriter.create(request.output_root)
    writer.write_json("request.json", request.as_json())
    command_runner = runner or clone_control.SubprocessCommandRunner()
    validate_bindings = binding_validator or validate_prepare_bindings
    stage = "binding-preflight"
    outcome = "precondition-rejected"
    reason = "not-run"
    inventory_queries = 0
    handle_rounds = 0
    asset_hash_rounds = 0

    try:
        binding = validate_bindings(request)
        writer.write_json("binding-preflight.json", binding.evidence)

        stage = "evidence-anchors"
        anchor_result = validate_evidence_anchors(request, binding.batch)
        writer.write_json("evidence-anchors.json", anchor_result)

        stage = "asset-identity-round-1"
        first = read_batch_identities(request, binding.batch)
        asset_hash_rounds = 1
        writer.write_json(
            "asset-identity-round-1.json", _identities_json(first)
        )

        stage = "utmctl-list-once"
        inventory_queries = 1
        inventory_observation = command_runner.run(
            ("utmctl", "list"), request.command_timeout_seconds
        )
        writer.write_json(
            "utmctl-list-once.json", inventory_observation.as_json()
        )
        registered = clone_control.parse_utmctl_list(inventory_observation)
        validate_inventory(registered, request, binding.batch)

        lsof_argv = _lsof_argv(first)
        for round_number in (1, 2):
            stage = f"handles-round-{round_number}"
            observation = command_runner.run(
                lsof_argv, request.command_timeout_seconds
            )
            handle_rounds = round_number
            writer.write_json(
                f"handles-round-{round_number}.json", observation.as_json()
            )
            _require_zero_handles(observation, lsof_argv)

        stage = "asset-identity-round-2"
        second = read_batch_identities(request, binding.batch)
        asset_hash_rounds = 2
        writer.write_json(
            "asset-identity-round-2.json", _identities_json(second)
        )
        if first != second:
            raise RetirementPrepareError("asset-identity-changed-between-rounds")

        outcome = "prepared"
        reason = "batch-read-only-prepared-for-separate-deletion-authorization"
    except (
        RetirementPrepareError,
        clone_control.CloneControlError,
        OSError,
    ) as exc:
        reason = f"{stage}:{exc}"

    terminal = {
        "asset_hash_rounds": asset_hash_rounds,
        "automatic_delete": "not-performed",
        "automatic_retry": "not-performed",
        "batch_id": request.batch_id,
        "clone": "not-performed",
        "format": EVIDENCE_FORMAT,
        "guest": "not-entered",
        "handle_rounds": handle_rounds,
        "inventory_queries": inventory_queries,
        "move": "not-performed",
        "outcome": outcome,
        "reason": reason,
        "start": "not-performed",
        "utmctl_delete_invocations": 0,
    }
    writer.write_json("terminal.json", terminal)
    manifest_sha256 = writer.write_manifest()
    return PrepareResult(
        outcome=outcome,
        exit_code=(
            EXIT_PREPARED if outcome == "prepared" else EXIT_PRECONDITION_REJECTED
        ),
        evidence_root=request.output_root,
        manifest_sha256=manifest_sha256,
        inventory_queries=inventory_queries,
        handle_rounds=handle_rounds,
        asset_hash_rounds=asset_hash_rounds,
    )


def validate_prepare_bindings(request: PrepareRequest) -> BindingResult:
    head = clone_control._run_git(
        request.repository_root, ("rev-parse", "HEAD")
    ).decode("ascii").strip()
    if head != request.expected_repository_head:
        raise RetirementPrepareError("repository-head-drift")
    if clone_control._run_git(
        request.repository_root, ("status", "--porcelain")
    ):
        raise RetirementPrepareError("repository-not-clean")
    control_path = request.repository_root / CONTROL_RELATIVE_PATH
    allowlist_path = request.repository_root / ALLOWLIST_RELATIVE_PATH
    control_sha256 = _validate_repository_file(control_path, Path(__file__))
    allowlist_sha256 = _validate_repository_file(allowlist_path, allowlist_path)
    if allowlist_sha256 != request.expected_allowlist_sha256:
        raise RetirementPrepareError("allowlist-sha256-drift")
    allowlist = load_allowlist(allowlist_path)
    if (
        request.operator_asset_root != allowlist.operator_asset_root
        or request.utm_documents_root != allowlist.utm_documents_root
    ):
        raise RetirementPrepareError("allowlist-storage-root-drift")
    batch = allowlist.batches.get(request.batch_id)
    if batch is None:
        raise RetirementPrepareError("batch-not-allowlisted")
    return BindingResult(
        evidence={
            "allowlist_asset_count": len(batch.assets),
            "allowlist_sha256": allowlist_sha256,
            "batch_accounting_gib": batch.accounting_gib,
            "batch_id": batch.batch_id,
            "control_sha256": control_sha256,
            "format": EVIDENCE_FORMAT,
            "repository_clean": True,
            "repository_head": head,
        },
        batch=batch,
    )


def load_allowlist(path: Path) -> RetirementAllowlist:
    try:
        payload = path.read_bytes()
        if len(payload) > 128 * 1024:
            raise RetirementPrepareError("allowlist-too-large")
        value = json.loads(payload)
    except (OSError, json.JSONDecodeError) as exc:
        raise RetirementPrepareError("allowlist-unreadable") from exc
    _require_keys(
        value,
        {"format", "storage_roots", "assets", "batches"},
        "allowlist",
    )
    if value["format"] != ALLOWLIST_FORMAT:
        raise RetirementPrepareError("allowlist-format-invalid")
    if not isinstance(value["assets"], list) or not isinstance(
        value["batches"], list
    ):
        raise RetirementPrepareError("allowlist-collections-invalid")
    _require_keys(
        value["storage_roots"], {"operator", "utm-documents"}, "storage-roots"
    )
    operator_asset_root = _require_absolute_root(
        value["storage_roots"]["operator"], "operator-storage-root"
    )
    utm_documents_root = _require_absolute_root(
        value["storage_roots"]["utm-documents"], "utm-documents-storage-root"
    )
    if clone_control._paths_overlap(operator_asset_root, utm_documents_root):
        raise RetirementPrepareError("allowlist-storage-roots-overlap")
    assets: dict[str, RetirementAsset] = {}
    paths: set[tuple[str, str]] = set()
    uuids: set[str] = set()
    names: set[str] = set()
    for item in value["assets"]:
        asset = _parse_asset(item)
        if asset.asset_id in assets:
            raise RetirementPrepareError("allowlist-asset-id-duplicate")
        identity = (asset.storage, asset.relative_path)
        if identity in paths or asset.uuid in uuids or asset.name in names:
            raise RetirementPrepareError("allowlist-asset-identity-duplicate")
        assets[asset.asset_id] = asset
        paths.add(identity)
        uuids.add(asset.uuid)
        names.add(asset.name)
    batches: dict[str, RetirementBatch] = {}
    assigned_asset_ids: set[str] = set()
    for item in value["batches"]:
        _require_keys(item, {"id", "asset_ids", "accounting_gib"}, "batch")
        batch_id = _require_safe_id(item["id"], "batch-id")
        asset_ids = _require_string_tuple(item["asset_ids"], "batch-asset-ids")
        if not 1 <= len(asset_ids) <= 4 or len(set(asset_ids)) != len(asset_ids):
            raise RetirementPrepareError("batch-asset-count-or-duplicates-invalid")
        if any(asset_id not in assets for asset_id in asset_ids):
            raise RetirementPrepareError("batch-asset-not-allowlisted")
        if assigned_asset_ids.intersection(asset_ids):
            raise RetirementPrepareError("batch-asset-reused")
        accounting_gib = item["accounting_gib"]
        if not isinstance(accounting_gib, str) or not re.fullmatch(
            r"[0-9]{1,3}\.[0-9]{2}", accounting_gib
        ):
            raise RetirementPrepareError("batch-accounting-gib-invalid")
        if batch_id in batches:
            raise RetirementPrepareError("batch-id-duplicate")
        batches[batch_id] = RetirementBatch(
            batch_id=batch_id,
            asset_ids=asset_ids,
            accounting_gib=accounting_gib,
            assets=tuple(assets[asset_id] for asset_id in asset_ids),
        )
        assigned_asset_ids.update(asset_ids)
    if not batches:
        raise RetirementPrepareError("allowlist-batches-empty")
    if assigned_asset_ids != set(assets):
        raise RetirementPrepareError("allowlist-assets-not-assigned-once")
    return RetirementAllowlist(
        operator_asset_root=operator_asset_root,
        utm_documents_root=utm_documents_root,
        batches=batches,
    )


def _parse_asset(value: object) -> RetirementAsset:
    keys = {
        "id",
        "kind",
        "action",
        "storage",
        "relative_path",
        "name",
        "uuid",
        "role",
        "config_sha256",
        "efi_sha256",
        "qcow2_sha256",
        "qcow2_name",
        "evidence_anchors",
    }
    _require_keys(value, keys, "asset")
    if value["kind"] != "utm-vm-bundle":
        raise RetirementPrepareError("asset-kind-invalid")
    if value["action"] != "evidence-archived-delete-candidate":
        raise RetirementPrepareError("asset-action-not-delete-candidate")
    storage = _require_storage(value["storage"])
    relative_path = _require_relative(value["relative_path"], "asset-path")
    name = _require_text(value["name"], "asset-name")
    if "/" in relative_path or not relative_path.endswith(".utm"):
        raise RetirementPrepareError("asset-path-invalid")
    vm_uuid = _require_uuid(value["uuid"])
    role = _require_text(value["role"], "asset-role")
    qcow2_name = _require_relative(value["qcow2_name"], "qcow2-name")
    if "/" in qcow2_name or not qcow2_name.endswith(".qcow2"):
        raise RetirementPrepareError("qcow2-name-invalid")
    anchors_value = value["evidence_anchors"]
    if not isinstance(anchors_value, list) or not anchors_value:
        raise RetirementPrepareError("evidence-anchors-empty")
    anchors: list[EvidenceAnchor] = []
    for anchor_value in anchors_value:
        _require_keys(
            anchor_value,
            {"storage", "relative_path", "sha256", "mode", "semantic"},
            "evidence-anchor",
        )
        semantic = anchor_value["semantic"]
        if semantic not in SEMANTICS:
            raise RetirementPrepareError("evidence-anchor-semantic-invalid")
        mode_text = anchor_value["mode"]
        if mode_text not in ("0600", "0644"):
            raise RetirementPrepareError("evidence-anchor-mode-invalid")
        anchors.append(
            EvidenceAnchor(
                storage=_require_storage(anchor_value["storage"]),
                relative_path=_require_relative(
                    anchor_value["relative_path"], "evidence-anchor-path"
                ),
                sha256=_require_hash(anchor_value["sha256"], "evidence-anchor"),
                mode=int(mode_text, 8),
                semantic=semantic,
            )
        )
    return RetirementAsset(
        asset_id=_require_safe_id(value["id"], "asset-id"),
        storage=storage,
        relative_path=relative_path,
        name=name,
        uuid=vm_uuid,
        role=role,
        config_sha256=_require_hash(value["config_sha256"], "config"),
        efi_sha256=_require_hash(value["efi_sha256"], "efi"),
        qcow2_sha256=_require_hash(value["qcow2_sha256"], "qcow2"),
        qcow2_name=qcow2_name,
        evidence_anchors=tuple(anchors),
    )


def validate_evidence_anchors(
    request: PrepareRequest, batch: RetirementBatch
) -> dict[str, object]:
    verified: list[dict[str, object]] = []
    owner = _root_owner(request.operator_asset_root)
    for asset in batch.assets:
        for index, anchor in enumerate(asset.evidence_anchors, start=1):
            path = _storage_root(request, anchor.storage) / anchor.relative_path
            _validate_regular(path, anchor.mode, owner, "evidence-anchor")
            if clone_control._sha256_file(path) != anchor.sha256:
                raise RetirementPrepareError("evidence-anchor-sha256-drift")
            _validate_anchor_semantics(path, anchor, asset)
            verified.append(
                {
                    "asset_id": asset.asset_id,
                    "index": index,
                    "semantic": anchor.semantic,
                    "sha256": anchor.sha256,
                }
            )
    return {
        "anchors_verified": len(verified),
        "assets_verified": len(batch.assets),
        "format": EVIDENCE_FORMAT,
        "verified": verified,
    }


def _validate_anchor_semantics(
    path: Path, anchor: EvidenceAnchor, asset: RetirementAsset
) -> None:
    if anchor.semantic == "opaque-hash":
        return
    if anchor.semantic == "frozen-disk-identity":
        _validate_frozen_disk_identity(path, asset)
        return
    if anchor.semantic == "install-prepared-failed-closed-terminal":
        _validate_install_prepared_failed_closed(path)
        return
    try:
        value = json.loads(path.read_bytes())
    except (OSError, json.JSONDecodeError) as exc:
        raise RetirementPrepareError("evidence-anchor-json-invalid") from exc
    if anchor.semantic == "release-pair":
        if value.get("evidence_format") != "radishlex-linux-l6-release-pair-evidence-v1":
            raise RetirementPrepareError("release-pair-evidence-format-invalid")
        return
    if anchor.semantic == "snapshot-source-vm":
        if value.get("format") != "radishlex-linux-l6-local-snapshot-evidence-v1":
            raise RetirementPrepareError("snapshot-evidence-format-invalid")
        source = value.get("source_vm")
        snapshot = value.get("snapshot")
        if not isinstance(source, dict) or not isinstance(snapshot, dict):
            raise RetirementPrepareError("snapshot-evidence-shape-invalid")
        if (
            source.get("relative_path") != asset.relative_path
            or source.get("name") != asset.name
            or source.get("uuid") != asset.uuid
        ):
            raise RetirementPrepareError("snapshot-source-identity-mismatch")
        files = snapshot.get("restorable_files")
        if not isinstance(files, list):
            raise RetirementPrepareError("snapshot-restorable-files-invalid")
        actual = {
            item.get("path"): item.get("sha256")
            for item in files
            if isinstance(item, dict)
        }
        expected = {
            "config.plist": asset.config_sha256,
            "Data/efi_vars.fd": asset.efi_sha256,
            f"Data/{asset.qcow2_name}": asset.qcow2_sha256,
        }
        if actual != expected:
            raise RetirementPrepareError("snapshot-restorable-identity-mismatch")
        return
    if anchor.semantic == "utm-start-terminal":
        _validate_utm_start_terminal(value, asset)
        return
    if anchor.semantic == "maintenance-repair-terminal":
        _validate_maintenance_repair_terminal(value, asset)
        return
    if value.get("format") != "radishlex-linux-l6-repair-aborted-preserved-host-evidence-v1":
        raise RetirementPrepareError("repair-terminal-format-invalid")
    guest = value.get("guest")
    disk = value.get("host_disk_identity")
    after = disk.get("after") if isinstance(disk, dict) else None
    if not isinstance(guest, dict) or not isinstance(after, dict):
        raise RetirementPrepareError("repair-terminal-shape-invalid")
    if guest.get("name") != asset.name or guest.get("uuid") != asset.uuid:
        raise RetirementPrepareError("repair-terminal-guest-identity-mismatch")
    expected_after = {
        "config_plist_sha256": asset.config_sha256,
        "efi_vars_sha256": asset.efi_sha256,
        "qcow2_sha256": asset.qcow2_sha256,
    }
    if after != expected_after:
        raise RetirementPrepareError("repair-terminal-disk-identity-mismatch")


def _validate_frozen_disk_identity(path: Path, asset: RetirementAsset) -> None:
    value = _parse_key_value_evidence(path)
    evidence_format = value.get("format")
    if evidence_format == (
        "radishlex-linux-l6-install-prepared-failed-closed-disk-identity-v1"
    ):
        uuid_key = "clone_uuid"
        config_key = "config_sha256"
        efi_key = "efi_sha256"
        qcow2_key = "qcow2_sha256"
        if (
            value.get("registered_vms") != "all-stopped"
            or value.get("clone_disk_handles") != "0"
            or value.get("disposable_restore") != "passed"
        ):
            raise RetirementPrepareError("frozen-disk-terminal-invalid")
    elif evidence_format == (
        "radishlex-linux-l6-install-artifacts-staged-start-retry-"
        "failure-postverify-d75818f-v1"
    ):
        uuid_key = None
        config_key = "clone_config_sha256"
        efi_key = "clone_efi_sha256"
        qcow2_key = "clone_qcow2_sha256"
        if (
            value.get("registered_vms") != "all-stopped"
            or value.get("target_vm") != "stopped"
            or value.get("source_and_clone_disk_handles") != "0"
            or value.get("terminal") != "failed-closed-stopped"
            or value.get("postverify") != "failed-closed-preserved"
        ):
            raise RetirementPrepareError("frozen-disk-terminal-invalid")
    elif evidence_format == "radishlex-linux-l6-d75818f-v3-start-failure-postverify-v1":
        uuid_key = "target_uuid"
        config_key = "target_config_sha256"
        efi_key = "target_efi_sha256"
        qcow2_key = "target_qcow2_sha256"
        if (
            value.get("registered_vms") != "all-stopped"
            or value.get("target_vm") != "stopped"
            or value.get("source_target_handles") != "0"
            or value.get("terminal") != "failed-closed-stopped"
            or value.get("postverify") != "failed-closed-preserved"
        ):
            raise RetirementPrepareError("frozen-disk-terminal-invalid")
    else:
        raise RetirementPrepareError("frozen-disk-evidence-format-invalid")
    if uuid_key is not None and value.get(uuid_key) != asset.uuid:
        raise RetirementPrepareError("frozen-disk-uuid-mismatch")
    if (
        value.get(config_key) != asset.config_sha256
        or value.get(efi_key) != asset.efi_sha256
        or value.get(qcow2_key) != asset.qcow2_sha256
    ):
        raise RetirementPrepareError("frozen-disk-identity-mismatch")


def _validate_install_prepared_failed_closed(path: Path) -> None:
    value = _parse_key_value_evidence(path)
    if (
        value.get("format")
        != "radishlex-linux-l6-install-prepared-failed-closed-host-summary-v1"
        or value.get("case") != "install_prepared"
        or value.get("acceptance_invocations") != "1"
        or value.get("checkpoint_count") != "1"
        or value.get("dpkg_mutation") != "not_started"
        or value.get("resume_invocations") != "0"
        or value.get("postflight_invocations") != "0"
        or value.get("outcome") != "failed-closed"
    ):
        raise RetirementPrepareError("install-prepared-terminal-invalid")


def _validate_utm_start_terminal(
    value: dict[str, object], asset: RetirementAsset
) -> None:
    if (
        value.get("format") != "radishlex-linux-l6-utm-start-once-v1"
        or value.get("clone_name") != asset.name
        or value.get("clone_uuid") != asset.uuid
        or value.get("outcome") != "failed-closed-stopped"
        or value.get("start_invocations") != 1
        or value.get("automatic_retry") != "not-performed"
        or value.get("automatic_stop") != "not-performed"
        or value.get("guest_exec") != "not-performed"
        or value.get("input_transfer") != "not-performed"
        or value.get("operation_id") != "not-generated"
        or value.get("transaction") != "not-performed"
    ):
        raise RetirementPrepareError("utm-start-terminal-invalid")


def _validate_maintenance_repair_terminal(
    value: dict[str, object], asset: RetirementAsset
) -> None:
    clone = value.get("clone")
    terminal = value.get("terminal")
    virtual_machines = value.get("virtual_machines")
    after = clone.get("after_shutdown") if isinstance(clone, dict) else None
    if (
        value.get("format")
        != "radishlex-linux-l6-maintenance-refresh-repair-completed-noop-local-evidence-v1"
        or not isinstance(clone, dict)
        or not isinstance(after, dict)
        or not isinstance(terminal, dict)
        or not isinstance(virtual_machines, dict)
        or clone.get("name") != asset.name
        or clone.get("uuid") != asset.uuid
        or after.get("config_sha256") != asset.config_sha256
        or after.get("efi_sha256") != asset.efi_sha256
        or after.get("qcow2_sha256") != asset.qcow2_sha256
        or after.get("qcow2_open_handles") != 0
        or terminal.get("maintenance_outcome") != "completed"
        or terminal.get("terminal_classification")
        != "completed_without_package_reapply"
        or terminal.get("dpkg_mutation_executed") is not False
        or virtual_machines.get("all_stopped_after") is not True
    ):
        raise RetirementPrepareError("maintenance-repair-terminal-invalid")


def _parse_key_value_evidence(path: Path) -> dict[str, str]:
    try:
        lines = path.read_text(encoding="utf-8").splitlines()
    except (OSError, UnicodeDecodeError) as exc:
        raise RetirementPrepareError("key-value-evidence-invalid") from exc
    value: dict[str, str] = {}
    for line in lines:
        if not line or "=" not in line:
            raise RetirementPrepareError("key-value-evidence-invalid")
        key, item = line.split("=", 1)
        if not re.fullmatch(r"[a-z0-9_]+", key) or key in value:
            raise RetirementPrepareError("key-value-evidence-invalid")
        value[key] = item
    if not value:
        raise RetirementPrepareError("key-value-evidence-invalid")
    return value


def read_batch_identities(
    request: PrepareRequest, batch: RetirementBatch
) -> tuple[BundleIdentity, ...]:
    owner = _root_owner(request.operator_asset_root)
    return tuple(_read_bundle(request, asset, owner) for asset in batch.assets)


def _read_bundle(
    request: PrepareRequest, asset: RetirementAsset, owner: tuple[int, int]
) -> BundleIdentity:
    root = _storage_root(request, asset.storage) / asset.relative_path
    _validate_directory(root, 0o755, owner, "bundle-root")
    data = root / "Data"
    _validate_directory(data, 0o755, owner, "bundle-data")
    config = root / "config.plist"
    efi = data / "efi_vars.fd"
    qcow2 = data / asset.qcow2_name
    for path, label in ((config, "config"), (efi, "efi"), (qcow2, "qcow2")):
        _validate_regular(path, 0o644, owner, label)
    try:
        plist = plistlib.loads(config.read_bytes())
    except (OSError, plistlib.InvalidFileException) as exc:
        raise RetirementPrepareError("config-plist-invalid") from exc
    information = plist.get("Information") if isinstance(plist, dict) else None
    drives = plist.get("Drive") if isinstance(plist, dict) else None
    disks = (
        [item for item in drives if isinstance(item, dict) and item.get("ImageType") == "Disk"]
        if isinstance(drives, list)
        else []
    )
    if (
        not isinstance(information, dict)
        or information.get("Name") != asset.name
        or information.get("UUID") != asset.uuid
        or len(disks) != 1
        or disks[0].get("ImageName") != asset.qcow2_name
    ):
        raise RetirementPrepareError("bundle-config-identity-mismatch")
    hashes = (
        clone_control._sha256_file(config),
        clone_control._sha256_file(efi),
        clone_control._sha256_file(qcow2),
    )
    if hashes != (
        asset.config_sha256,
        asset.efi_sha256,
        asset.qcow2_sha256,
    ):
        raise RetirementPrepareError("bundle-disk-sha256-drift")
    return BundleIdentity(
        asset_id=asset.asset_id,
        config_path=config,
        efi_path=efi,
        qcow2_path=qcow2,
        config_sha256=hashes[0],
        efi_sha256=hashes[1],
        qcow2_sha256=hashes[2],
        config_size=config.stat().st_size,
        efi_size=efi.stat().st_size,
        qcow2_size=qcow2.stat().st_size,
    )


def validate_inventory(
    registered: tuple[clone_control.RegisteredVm, ...],
    request: PrepareRequest,
    batch: RetirementBatch,
) -> None:
    if len(registered) != request.expected_vm_count:
        raise RetirementPrepareError("inventory-vm-count-mismatch")
    if any(item.status != "stopped" for item in registered):
        raise RetirementPrepareError("inventory-not-all-stopped")
    if len({item.name for item in registered}) != len(registered):
        raise RetirementPrepareError("inventory-name-duplicate")
    if clone_control.canonical_inventory_sha256(registered) != (
        request.expected_inventory_sha256
    ):
        raise RetirementPrepareError("inventory-sha256-mismatch")
    by_uuid = {item.uuid: item for item in registered}
    for asset in batch.assets:
        item = by_uuid.get(asset.uuid)
        if item is None or item.name != asset.name:
            raise RetirementPrepareError("inventory-batch-asset-identity-mismatch")


def _lsof_argv(identities: tuple[BundleIdentity, ...]) -> tuple[str, ...]:
    paths = sorted(
        str(path)
        for identity in identities
        for path in (identity.config_path, identity.efi_path, identity.qcow2_path)
    )
    return ("/usr/sbin/lsof", "-n", "-P", "-F", "ctfn", *paths)


def _require_zero_handles(
    observation: clone_control.CommandObservation,
    expected_argv: tuple[str, ...],
) -> None:
    if observation.argv != expected_argv:
        raise RetirementPrepareError("lsof-argv-drift")
    if (
        observation.timed_out
        or observation.exit_code != 1
        or observation.stdout.total_bytes != 0
        or observation.stderr.total_bytes != 0
        or observation.stdout.truncated
        or observation.stderr.truncated
    ):
        raise RetirementPrepareError("asset-open-handles-or-lsof-indeterminate")


def _storage_root(request: PrepareRequest, storage: str) -> Path:
    return (
        request.operator_asset_root
        if storage == "operator"
        else request.utm_documents_root
    )


def _root_owner(root: Path) -> tuple[int, int]:
    try:
        root_stat = root.lstat()
    except OSError as exc:
        raise RetirementPrepareError("asset-root-unavailable") from exc
    if not stat.S_ISDIR(root_stat.st_mode) or stat.S_ISLNK(root_stat.st_mode):
        raise RetirementPrepareError("asset-root-invalid")
    return root_stat.st_uid, root_stat.st_gid


def _validate_directory(
    path: Path, mode: int, owner: tuple[int, int], label: str
) -> None:
    _validate_ancestors(path)
    try:
        item = path.lstat()
    except OSError as exc:
        raise RetirementPrepareError(f"{label}-unavailable") from exc
    if (
        not stat.S_ISDIR(item.st_mode)
        or stat.S_ISLNK(item.st_mode)
        or stat.S_IMODE(item.st_mode) != mode
        or (item.st_uid, item.st_gid) != owner
    ):
        raise RetirementPrepareError(f"{label}-identity-invalid")


def _validate_regular(
    path: Path, mode: int, owner: tuple[int, int], label: str
) -> None:
    _validate_ancestors(path)
    try:
        item = path.lstat()
    except OSError as exc:
        raise RetirementPrepareError(f"{label}-unavailable") from exc
    if (
        not stat.S_ISREG(item.st_mode)
        or stat.S_ISLNK(item.st_mode)
        or stat.S_IMODE(item.st_mode) != mode
        or (item.st_uid, item.st_gid) != owner
        or item.st_nlink != 1
    ):
        raise RetirementPrepareError(f"{label}-identity-invalid")


def _validate_ancestors(path: Path) -> None:
    current = path.parent
    while current != current.parent:
        try:
            item = current.lstat()
        except OSError as exc:
            raise RetirementPrepareError("path-ancestor-unavailable") from exc
        if not stat.S_ISDIR(item.st_mode) or stat.S_ISLNK(item.st_mode):
            raise RetirementPrepareError("path-ancestor-invalid")
        current = current.parent


def _validate_repository_file(path: Path, invoked: Path) -> str:
    if path.absolute() != invoked.absolute():
        raise RetirementPrepareError("repository-file-path-mismatch")
    try:
        item = path.lstat()
    except OSError as exc:
        raise RetirementPrepareError("repository-file-unavailable") from exc
    if (
        not stat.S_ISREG(item.st_mode)
        or stat.S_ISLNK(item.st_mode)
        or item.st_nlink != 1
        or stat.S_IMODE(item.st_mode) & 0o022
    ):
        raise RetirementPrepareError("repository-file-identity-invalid")
    return clone_control._sha256_file(path)


def _identities_json(
    identities: tuple[BundleIdentity, ...]
) -> dict[str, object]:
    return {
        "assets": [identity.as_json() for identity in identities],
        "assets_verified": len(identities),
        "format": EVIDENCE_FORMAT,
    }


def _require_keys(value: object, expected: set[str], label: str) -> None:
    if not isinstance(value, dict) or set(value) != expected:
        raise RetirementPrepareError(f"{label}-keys-invalid")


def _require_safe_id(value: object, label: str) -> str:
    if not isinstance(value, str) or not SAFE_ID.fullmatch(value):
        raise RetirementPrepareError(f"{label}-invalid")
    return value


def _require_storage(value: object) -> str:
    if value not in STORAGES:
        raise RetirementPrepareError("storage-invalid")
    return str(value)


def _require_relative(value: object, label: str) -> str:
    if not isinstance(value, str) or not value or "\\" in value or "\x00" in value:
        raise RetirementPrepareError(f"{label}-invalid")
    path = PurePosixPath(value)
    if path.is_absolute() or any(part in ("", ".", "..") for part in path.parts):
        raise RetirementPrepareError(f"{label}-invalid")
    return value


def _require_absolute_root(value: object, label: str) -> Path:
    if not isinstance(value, str) or not value or "\x00" in value:
        raise RetirementPrepareError(f"{label}-invalid")
    path = Path(value)
    if not path.is_absolute() or ".." in path.parts or str(path) != value:
        raise RetirementPrepareError(f"{label}-invalid")
    return path


def _require_text(value: object, label: str) -> str:
    if (
        not isinstance(value, str)
        or not value
        or len(value.encode("utf-8")) > 160
        or any(character in "\x00\r\n/" for character in value)
    ):
        raise RetirementPrepareError(f"{label}-invalid")
    return value


def _require_uuid(value: object) -> str:
    if not isinstance(value, str):
        raise RetirementPrepareError("uuid-invalid")
    try:
        canonical = str(uuid.UUID(value)).upper()
    except ValueError as exc:
        raise RetirementPrepareError("uuid-invalid") from exc
    if canonical != value:
        raise RetirementPrepareError("uuid-not-uppercase-canonical")
    return value


def _require_hash(value: object, label: str) -> str:
    if not isinstance(value, str) or not HEX_64.fullmatch(value):
        raise RetirementPrepareError(f"{label}-sha256-invalid")
    return value


def _require_string_tuple(value: object, label: str) -> tuple[str, ...]:
    if not isinstance(value, list) or not all(isinstance(item, str) for item in value):
        raise RetirementPrepareError(f"{label}-invalid")
    return tuple(value)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Prepare one exact allowlisted L6 VM retirement batch with one "
            "read-only inventory query, two handle rounds, and no mutation."
        )
    )
    parser.add_argument("command", choices=("prepare-once",))
    parser.add_argument("--repository-root", type=Path, required=True)
    parser.add_argument("--expected-repository-head", required=True)
    parser.add_argument("--output-root", type=Path, required=True)
    parser.add_argument("--attempt-id", required=True)
    parser.add_argument("--operator-asset-root", type=Path, required=True)
    parser.add_argument("--utm-documents-root", type=Path, required=True)
    parser.add_argument("--expected-allowlist-sha256", required=True)
    parser.add_argument("--batch-id", required=True)
    parser.add_argument("--expected-vm-count", type=int, required=True)
    parser.add_argument("--expected-inventory-sha256", required=True)
    parser.add_argument("--command-timeout-seconds", type=int, default=15)
    parser.add_argument("--authorized-read-only-prepare", action="store_true")
    parser.add_argument("--authorized-one-utmctl-list", action="store_true")
    parser.add_argument("--authorized-two-handle-rounds", action="store_true")
    parser.add_argument(
        "--authorized-no-delete-start-clone-move-or-guest", action="store_true"
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    request = PrepareRequest(
        repository_root=args.repository_root,
        expected_repository_head=args.expected_repository_head,
        output_root=args.output_root,
        attempt_id=args.attempt_id,
        operator_asset_root=args.operator_asset_root,
        utm_documents_root=args.utm_documents_root,
        expected_allowlist_sha256=args.expected_allowlist_sha256,
        batch_id=args.batch_id,
        expected_vm_count=args.expected_vm_count,
        expected_inventory_sha256=args.expected_inventory_sha256,
        command_timeout_seconds=args.command_timeout_seconds,
        authorized_read_only_prepare=args.authorized_read_only_prepare,
        authorized_one_utmctl_list=args.authorized_one_utmctl_list,
        authorized_two_handle_rounds=args.authorized_two_handle_rounds,
        authorized_no_delete_start_clone_move_or_guest=(
            args.authorized_no_delete_start_clone_move_or_guest
        ),
    )
    try:
        result = run_prepare(request)
    except (RetirementPrepareError, clone_control.CloneControlError) as exc:
        print(f"l6_asset_retirement_prepare_error={exc}", file=sys.stderr)
        return EXIT_PRECONDITION_REJECTED
    print(f"asset_retirement_prepare_outcome={result.outcome}")
    print(f"evidence_root={result.evidence_root}")
    print(f"files_sha256={result.manifest_sha256}")
    return result.exit_code


if __name__ == "__main__":
    raise SystemExit(main())
