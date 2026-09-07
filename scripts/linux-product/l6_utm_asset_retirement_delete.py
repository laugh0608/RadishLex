#!/usr/bin/env python3
from __future__ import annotations

import argparse
import base64
import hashlib
import json
import stat
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Callable, Protocol

import l6_utm_asset_retirement_prepare as prepare
import l6_utm_clone_once as clone_control


EVIDENCE_FORMAT = "radishlex-linux-l6-asset-retirement-delete-v1"
CONTROL_RELATIVE_PATH = Path(
    "scripts/linux-product/l6_utm_asset_retirement_delete.py"
)
PRIOR_PREPARE_FILES = frozenset(
    (
        "request.json",
        "binding-preflight.json",
        "evidence-anchors.json",
        "asset-identity-round-1.json",
        "utmctl-list-once.json",
        "handles-round-1.json",
        "handles-round-2.json",
        "asset-identity-round-2.json",
        "terminal.json",
    )
)
EXIT_DELETED = 0
EXIT_PRECONDITION_REJECTED = 11
EXIT_STATE_INDETERMINATE = 12
BATCH_AUTHORIZATION_FIELDS = {
    "first-four-v1": "delete_first_four_v1",
    "second-batch-v1": "delete_second_batch_v1",
    "third-batch-v1": "delete_third_batch_v1",
    "fourth-batch-v1": "delete_fourth_batch_v1",
    "fifth-batch-v1": "delete_fifth_batch_v1",
}
BATCH_ASSET_COUNTS = {
    "first-four-v1": 4,
    "second-batch-v1": 4,
    "third-batch-v1": 3,
    "fourth-batch-v1": 3,
    "fifth-batch-v1": 3,
}
SUPPORTED_BATCH_IDS = frozenset(BATCH_AUTHORIZATION_FIELDS)


class RetirementDeleteError(ValueError):
    pass


@dataclass(frozen=True)
class DeleteRequest:
    repository_root: Path
    expected_repository_head: str
    output_root: Path
    attempt_id: str
    operator_asset_root: Path
    utm_documents_root: Path
    expected_allowlist_sha256: str
    batch_id: str
    prior_prepare_root: Path
    prior_prepare_manifest_sha256: str
    expected_predelete_vm_count: int
    expected_predelete_inventory_sha256: str
    command_timeout_seconds: int
    delete_timeout_seconds: int
    authorized_delete_first_four_v1: bool
    authorized_delete_second_batch_v1: bool
    authorized_delete_third_batch_v1: bool
    authorized_delete_fourth_batch_v1: bool
    authorized_delete_fifth_batch_v1: bool
    authorized_at_most_one_delete_per_asset: bool
    authorized_stop_without_retry_or_rollback: bool
    acknowledge_irreversible_bundle_removal: bool

    def validate(self) -> None:
        for path, label in (
            (self.repository_root, "repository-root"),
            (self.output_root, "output-root"),
            (self.operator_asset_root, "operator-asset-root"),
            (self.utm_documents_root, "utm-documents-root"),
            (self.prior_prepare_root, "prior-prepare-root"),
        ):
            if not path.is_absolute() or ".." in path.parts:
                raise RetirementDeleteError(f"{label}-must-be-absolute-normalized")
        if self.output_root.parent != self.operator_asset_root:
            raise RetirementDeleteError("output-root-parent-must-be-operator-root")
        if self.prior_prepare_root.parent != self.operator_asset_root:
            raise RetirementDeleteError(
                "prior-prepare-root-parent-must-be-operator-root"
            )
        if clone_control._paths_overlap(self.output_root, self.prior_prepare_root):
            raise RetirementDeleteError("output-root-overlaps-prior-prepare")
        if clone_control._path_is_within(self.output_root, self.repository_root):
            raise RetirementDeleteError("output-root-must-be-outside-repository")
        if clone_control._paths_overlap(
            self.operator_asset_root, self.utm_documents_root
        ):
            raise RetirementDeleteError("asset-roots-must-not-overlap")
        if not prepare.HEX_40.fullmatch(self.expected_repository_head):
            raise RetirementDeleteError("expected-repository-head-invalid")
        for value, label in (
            (self.expected_allowlist_sha256, "expected-allowlist"),
            (self.prior_prepare_manifest_sha256, "prior-prepare-manifest"),
            (self.expected_predelete_inventory_sha256, "predelete-inventory"),
        ):
            if not prepare.HEX_64.fullmatch(value):
                raise RetirementDeleteError(f"{label}-sha256-invalid")
        if not prepare.SAFE_ID.fullmatch(self.attempt_id):
            raise RetirementDeleteError("attempt-id-invalid")
        if self.batch_id not in SUPPORTED_BATCH_IDS:
            raise RetirementDeleteError("batch-id-not-supported")
        if not 1 <= self.expected_predelete_vm_count <= 128:
            raise RetirementDeleteError("expected-vm-count-out-of-range")
        if not 1 <= self.command_timeout_seconds <= 60:
            raise RetirementDeleteError("command-timeout-seconds-out-of-range")
        if not 1 <= self.delete_timeout_seconds <= 120:
            raise RetirementDeleteError("delete-timeout-seconds-out-of-range")
        batch_authorizations = {
            "first-four-v1": self.authorized_delete_first_four_v1,
            "second-batch-v1": self.authorized_delete_second_batch_v1,
            "third-batch-v1": self.authorized_delete_third_batch_v1,
            "fourth-batch-v1": self.authorized_delete_fourth_batch_v1,
            "fifth-batch-v1": self.authorized_delete_fifth_batch_v1,
        }
        if not batch_authorizations[self.batch_id]:
            raise RetirementDeleteError(
                f"{self.batch_id}-delete-authorization-required"
            )
        if sum(batch_authorizations.values()) != 1:
            raise RetirementDeleteError("batch-delete-authorization-not-exact")
        if not self.authorized_at_most_one_delete_per_asset:
            raise RetirementDeleteError("one-delete-per-asset-authorization-required")
        if not self.authorized_stop_without_retry_or_rollback:
            raise RetirementDeleteError("stop-without-retry-authorization-required")
        if not self.acknowledge_irreversible_bundle_removal:
            raise RetirementDeleteError("irreversible-removal-acknowledgement-required")

    def as_json(self) -> dict[str, object]:
        batch_authorization = BATCH_AUTHORIZATION_FIELDS[self.batch_id]
        return {
            "attempt_id": self.attempt_id,
            "authorization": {
                "acknowledge_irreversible_bundle_removal": True,
                "at_most_one_delete_per_asset": True,
                batch_authorization: True,
                "stop_without_retry_or_rollback": True,
            },
            "batch_id": self.batch_id,
            "command_timeout_seconds": self.command_timeout_seconds,
            "delete_timeout_seconds": self.delete_timeout_seconds,
            "expected_allowlist_sha256": self.expected_allowlist_sha256,
            "expected_predelete_inventory_sha256": (
                self.expected_predelete_inventory_sha256
            ),
            "expected_predelete_vm_count": self.expected_predelete_vm_count,
            "expected_repository_head": self.expected_repository_head,
            "format": EVIDENCE_FORMAT,
            "operator_asset_root_sha256": clone_control._sha256_text(
                str(self.operator_asset_root)
            ),
            "prior_prepare_manifest_sha256": (
                self.prior_prepare_manifest_sha256
            ),
            "prior_prepare_root_sha256": clone_control._sha256_text(
                str(self.prior_prepare_root)
            ),
            "utm_documents_root_sha256": clone_control._sha256_text(
                str(self.utm_documents_root)
            ),
        }

    def prepare_request(self) -> prepare.PrepareRequest:
        return prepare.PrepareRequest(
            repository_root=self.repository_root,
            expected_repository_head=self.expected_repository_head,
            output_root=self.output_root,
            attempt_id=self.attempt_id,
            operator_asset_root=self.operator_asset_root,
            utm_documents_root=self.utm_documents_root,
            expected_allowlist_sha256=self.expected_allowlist_sha256,
            batch_id=self.batch_id,
            expected_vm_count=self.expected_predelete_vm_count,
            expected_inventory_sha256=self.expected_predelete_inventory_sha256,
            command_timeout_seconds=self.command_timeout_seconds,
            authorized_read_only_prepare=True,
            authorized_one_utmctl_list=True,
            authorized_two_handle_rounds=True,
            authorized_no_delete_start_clone_move_or_guest=True,
        )


@dataclass(frozen=True)
class DeleteBinding:
    evidence: dict[str, object]
    batch: prepare.RetirementBatch


@dataclass(frozen=True)
class DeleteResult:
    outcome: str
    exit_code: int
    evidence_root: Path
    manifest_sha256: str
    inventory_queries: int
    delete_invocations: int
    deleted_assets: int


class CommandRunner(Protocol):
    def run(
        self, argv: tuple[str, ...], timeout_seconds: int
    ) -> clone_control.CommandObservation: ...


BindingValidator = Callable[[DeleteRequest], DeleteBinding]


def run_delete(
    request: DeleteRequest,
    *,
    runner: CommandRunner | None = None,
    binding_validator: BindingValidator | None = None,
) -> DeleteResult:
    request.validate()
    writer = clone_control.EvidenceWriter.create(request.output_root)
    writer.write_json("request.json", request.as_json())
    command_runner = runner or clone_control.SubprocessCommandRunner()
    validate_bindings = binding_validator or validate_delete_bindings
    stage = "binding-preflight"
    outcome = "precondition-rejected"
    reason = "not-run"
    inventory_queries = 0
    handle_rounds = 0
    asset_hash_rounds = 0
    delete_invocations = 0
    deleted_asset_ids: list[str] = []

    try:
        binding = validate_bindings(request)
        writer.write_json("binding-preflight.json", binding.evidence)
        preflight = request.prepare_request()

        stage = "evidence-anchors"
        anchors = prepare.validate_evidence_anchors(preflight, binding.batch)
        writer.write_json("evidence-anchors.json", anchors)

        stage = "asset-identity-round-1"
        first = prepare.read_batch_identities(preflight, binding.batch)
        asset_hash_rounds = 1
        writer.write_json(
            "asset-identity-round-1.json", prepare._identities_json(first)
        )

        stage = "predelete-utmctl-list"
        inventory_queries = 1
        list_observation = command_runner.run(
            ("utmctl", "list"), request.command_timeout_seconds
        )
        writer.write_json("predelete-utmctl-list.json", list_observation.as_json())
        current_inventory = clone_control.parse_utmctl_list(list_observation)
        prepare.validate_inventory(current_inventory, preflight, binding.batch)

        lsof_argv = prepare._lsof_argv(first)
        for round_number in (1, 2):
            stage = f"handles-round-{round_number}"
            observation = command_runner.run(
                lsof_argv, request.command_timeout_seconds
            )
            handle_rounds = round_number
            writer.write_json(
                f"handles-round-{round_number}.json", observation.as_json()
            )
            prepare._require_zero_handles(observation, lsof_argv)

        stage = "asset-identity-round-2"
        second = prepare.read_batch_identities(preflight, binding.batch)
        asset_hash_rounds = 2
        writer.write_json(
            "asset-identity-round-2.json", prepare._identities_json(second)
        )
        if first != second:
            raise RetirementDeleteError("asset-identity-changed-between-rounds")

        for index, asset in enumerate(binding.batch.assets, start=1):
            stage = f"delete-{index:02d}-command"
            delete_argv = ("utmctl", "delete", asset.uuid)
            delete_invocations += 1
            delete_observation = command_runner.run(
                delete_argv, request.delete_timeout_seconds
            )
            writer.write_json(
                f"delete-{index:02d}-command.json", delete_observation.as_json()
            )

            stage = f"delete-{index:02d}-post-list"
            inventory_queries += 1
            post_list = command_runner.run(
                ("utmctl", "list"), request.command_timeout_seconds
            )
            writer.write_json(
                f"delete-{index:02d}-post-list.json", post_list.as_json()
            )
            post_inventory = clone_control.parse_utmctl_list(post_list)

            stage = f"delete-{index:02d}-package"
            package = observe_package(_asset_path(request, asset))
            writer.write_json(f"delete-{index:02d}-package.json", package)
            validate_deleted_transition(
                delete_observation,
                delete_argv,
                current_inventory,
                post_inventory,
                asset,
                package,
            )
            deleted_asset_ids.append(asset.asset_id)
            current_inventory = post_inventory

        outcome = "deleted"
        reason = f"{request.batch_id}-deleted-and-verified"
    except (
        RetirementDeleteError,
        prepare.RetirementPrepareError,
        clone_control.CloneControlError,
        OSError,
    ) as exc:
        if delete_invocations:
            outcome = "state-indeterminate"
        reason = f"{stage}:{exc}"

    terminal = {
        "asset_hash_rounds": asset_hash_rounds,
        "automatic_retry": "not-performed",
        "automatic_rollback": "not-performed",
        "batch_id": request.batch_id,
        "clone": "not-performed",
        "delete_invocations": delete_invocations,
        "deleted_asset_ids": deleted_asset_ids,
        "deleted_assets": len(deleted_asset_ids),
        "format": EVIDENCE_FORMAT,
        "guest": "not-entered",
        "handle_rounds": handle_rounds,
        "inventory_queries": inventory_queries,
        "move": "not-performed",
        "outcome": outcome,
        "reason": reason,
        "start": "not-performed",
    }
    writer.write_json("terminal.json", terminal)
    manifest_sha256 = writer.write_manifest()
    return DeleteResult(
        outcome=outcome,
        exit_code=(
            EXIT_DELETED
            if outcome == "deleted"
            else (
                EXIT_STATE_INDETERMINATE
                if outcome == "state-indeterminate"
                else EXIT_PRECONDITION_REJECTED
            )
        ),
        evidence_root=request.output_root,
        manifest_sha256=manifest_sha256,
        inventory_queries=inventory_queries,
        delete_invocations=delete_invocations,
        deleted_assets=len(deleted_asset_ids),
    )


def validate_delete_bindings(request: DeleteRequest) -> DeleteBinding:
    head = clone_control._run_git(
        request.repository_root, ("rev-parse", "HEAD")
    ).decode("ascii").strip()
    if head != request.expected_repository_head:
        raise RetirementDeleteError("repository-head-drift")
    if clone_control._run_git(
        request.repository_root, ("status", "--porcelain")
    ):
        raise RetirementDeleteError("repository-not-clean")
    control_path = request.repository_root / CONTROL_RELATIVE_PATH
    control_sha256 = prepare._validate_repository_file(control_path, Path(__file__))
    allowlist_path = request.repository_root / prepare.ALLOWLIST_RELATIVE_PATH
    allowlist_sha256 = prepare._validate_repository_file(
        allowlist_path, allowlist_path
    )
    if allowlist_sha256 != request.expected_allowlist_sha256:
        raise RetirementDeleteError("allowlist-sha256-drift")
    allowlist = prepare.load_allowlist(allowlist_path)
    if (
        allowlist.operator_asset_root != request.operator_asset_root
        or allowlist.utm_documents_root != request.utm_documents_root
    ):
        raise RetirementDeleteError("allowlist-storage-root-drift")
    batch = allowlist.batches.get(request.batch_id)
    expected_asset_count = BATCH_ASSET_COUNTS.get(request.batch_id)
    if (
        batch is None
        or expected_asset_count is None
        or len(batch.assets) != expected_asset_count
        or (
            request.batch_id == "fifth-batch-v1"
            and batch.asset_ids != prepare.FIFTH_BATCH_ASSET_IDS
        )
    ):
        raise RetirementDeleteError("retirement-batch-invalid")
    fifth_batch_control_sha256 = None
    if request.batch_id == "fifth-batch-v1":
        fifth_batch_control_sha256 = prepare._validate_repository_file(
            request.repository_root / prepare.FIFTH_BATCH_CONTROL_RELATIVE_PATH,
            Path(prepare.fifth_batch.__file__),
        )
    prior = validate_prior_prepare(
        request,
        batch,
        expected_fifth_batch_control_sha256=fifth_batch_control_sha256,
    )
    evidence = {
        "allowlist_asset_count": len(batch.assets),
        "allowlist_sha256": allowlist_sha256,
        "batch_accounting_gib": batch.accounting_gib,
        "batch_id": batch.batch_id,
        "control_sha256": control_sha256,
        "format": EVIDENCE_FORMAT,
        "prior_prepare_entries_verified": prior,
        "prior_prepare_manifest_sha256": (
            request.prior_prepare_manifest_sha256
        ),
        "repository_clean": True,
        "repository_head": head,
    }
    if fifth_batch_control_sha256 is not None:
        evidence["fifth_batch_control_sha256"] = fifth_batch_control_sha256
    return DeleteBinding(evidence=evidence, batch=batch)


def validate_prior_prepare(
    request: DeleteRequest,
    batch: prepare.RetirementBatch,
    *,
    expected_fifth_batch_control_sha256: str | None = None,
) -> int:
    manifest = request.prior_prepare_root / "files.sha256"
    if clone_control._sha256_file(manifest) != request.prior_prepare_manifest_sha256:
        raise RetirementDeleteError("prior-prepare-manifest-sha256-drift")
    entries = clone_control._verify_sha256_manifest(
        request.prior_prepare_root, manifest
    )
    actual_names = {path.name for path in request.prior_prepare_root.iterdir()}
    if actual_names != PRIOR_PREPARE_FILES | {"files.sha256"}:
        raise RetirementDeleteError("prior-prepare-entry-set-drift")
    if entries != len(PRIOR_PREPARE_FILES):
        raise RetirementDeleteError("prior-prepare-entry-count-drift")
    root_stat = request.prior_prepare_root.lstat()
    operator_stat = request.operator_asset_root.lstat()
    if (root_stat.st_uid, root_stat.st_gid) != (
        operator_stat.st_uid,
        operator_stat.st_gid,
    ):
        raise RetirementDeleteError("prior-prepare-owner-drift")

    prior_request = _read_json(request.prior_prepare_root / "request.json")
    prior_binding = _read_json(
        request.prior_prepare_root / "binding-preflight.json"
    )
    prior_terminal = _read_json(request.prior_prepare_root / "terminal.json")
    if (
        prior_request.get("format") != prepare.EVIDENCE_FORMAT
        or prior_request.get("batch_id") != request.batch_id
        or prior_request.get("expected_allowlist_sha256")
        != request.expected_allowlist_sha256
        or prior_request.get("expected_inventory_sha256")
        != request.expected_predelete_inventory_sha256
        or prior_request.get("expected_vm_count")
        != request.expected_predelete_vm_count
    ):
        raise RetirementDeleteError("prior-prepare-request-semantic-drift")
    authorization = prior_request.get("authorization")
    if not isinstance(authorization, dict) or set(authorization.values()) != {True}:
        raise RetirementDeleteError("prior-prepare-authorization-drift")
    if (
        prior_binding.get("format") != prepare.EVIDENCE_FORMAT
        or prior_binding.get("batch_id") != request.batch_id
        or prior_binding.get("allowlist_sha256")
        != request.expected_allowlist_sha256
        or prior_binding.get("repository_head")
        != prior_request.get("expected_repository_head")
        or prior_binding.get("repository_clean") is not True
    ):
        raise RetirementDeleteError("prior-prepare-binding-semantic-drift")
    if request.batch_id == "fifth-batch-v1" and (
        expected_fifth_batch_control_sha256 is None
        or prior_binding.get("fifth_batch_control_sha256")
        != expected_fifth_batch_control_sha256
    ):
        raise RetirementDeleteError("prior-prepare-fifth-control-drift")
    if (
        prior_terminal.get("format") != prepare.EVIDENCE_FORMAT
        or prior_terminal.get("batch_id") != request.batch_id
        or prior_terminal.get("outcome") != "prepared"
        or prior_terminal.get("inventory_queries") != 1
        or prior_terminal.get("handle_rounds") != 2
        or prior_terminal.get("asset_hash_rounds") != 2
        or prior_terminal.get("utmctl_delete_invocations") != 0
    ):
        raise RetirementDeleteError("prior-prepare-terminal-semantic-drift")

    first = _read_json(
        request.prior_prepare_root / "asset-identity-round-1.json"
    )
    second = _read_json(
        request.prior_prepare_root / "asset-identity-round-2.json"
    )
    if first != second:
        raise RetirementDeleteError("prior-prepare-identity-round-drift")
    validate_prior_identities(first, batch)
    validate_prior_anchors(
        _read_json(request.prior_prepare_root / "evidence-anchors.json"),
        batch,
    )
    prior_inventory = _observation_from_json(
        request.prior_prepare_root / "utmctl-list-once.json",
        ("utmctl", "list"),
    )
    registered = clone_control.parse_utmctl_list(prior_inventory)
    prepare.validate_inventory(registered, request.prepare_request(), batch)
    expected_lsof = _expected_lsof_argv(request, batch)
    for round_number in (1, 2):
        observation = _observation_from_json(
            request.prior_prepare_root / f"handles-round-{round_number}.json",
            expected_lsof,
        )
        prepare._require_zero_handles(observation, expected_lsof)
    return entries


def validate_prior_anchors(
    value: dict[str, object], batch: prepare.RetirementBatch
) -> None:
    expected = [
        {
            "asset_id": asset.asset_id,
            "index": index,
            "semantic": anchor.semantic,
            "sha256": anchor.sha256,
        }
        for asset in batch.assets
        for index, anchor in enumerate(asset.evidence_anchors, start=1)
    ]
    if value != {
        "anchors_verified": len(expected),
        "assets_verified": len(batch.assets),
        "format": prepare.EVIDENCE_FORMAT,
        "verified": expected,
    }:
        raise RetirementDeleteError("prior-prepare-anchors-drift")


def validate_prior_identities(
    value: dict[str, object], batch: prepare.RetirementBatch
) -> None:
    assets = value.get("assets")
    if (
        value.get("format") != prepare.EVIDENCE_FORMAT
        or value.get("assets_verified") != len(batch.assets)
        or not isinstance(assets, list)
        or len(assets) != len(batch.assets)
    ):
        raise RetirementDeleteError("prior-prepare-identities-shape-drift")
    for actual, expected in zip(assets, batch.assets, strict=True):
        if not isinstance(actual, dict) or (
            actual.get("asset_id") != expected.asset_id
            or actual.get("config_sha256") != expected.config_sha256
            or actual.get("efi_sha256") != expected.efi_sha256
            or actual.get("qcow2_sha256") != expected.qcow2_sha256
        ):
            raise RetirementDeleteError("prior-prepare-asset-identity-drift")
        for key in ("config_size", "efi_size", "qcow2_size"):
            if not isinstance(actual.get(key), int) or actual[key] <= 0:
                raise RetirementDeleteError("prior-prepare-asset-size-invalid")


def validate_deleted_transition(
    delete_observation: clone_control.CommandObservation,
    expected_argv: tuple[str, ...],
    before: tuple[clone_control.RegisteredVm, ...],
    after: tuple[clone_control.RegisteredVm, ...],
    asset: prepare.RetirementAsset,
    package: dict[str, object],
) -> None:
    if delete_observation.argv != expected_argv:
        raise RetirementDeleteError("delete-argv-drift")
    if (
        delete_observation.timed_out
        or delete_observation.exit_code != 0
        or delete_observation.stderr.total_bytes != 0
        or delete_observation.stdout.truncated
        or delete_observation.stderr.truncated
    ):
        raise RetirementDeleteError("delete-command-not-clean-success")
    if any(item.status != "stopped" for item in after):
        raise RetirementDeleteError("postdelete-inventory-not-all-stopped")
    if len(after) != len(before) - 1:
        raise RetirementDeleteError("postdelete-inventory-count-delta-invalid")
    expected_after = {item.uuid: item for item in before if item.uuid != asset.uuid}
    actual_after = {item.uuid: item for item in after}
    if len(actual_after) != len(after) or actual_after != expected_after:
        raise RetirementDeleteError("postdelete-inventory-member-drift")
    if package.get("state") != "absent":
        raise RetirementDeleteError("postdelete-package-not-absent")


def observe_package(path: Path) -> dict[str, object]:
    try:
        item = path.lstat()
    except FileNotFoundError:
        return {"format": EVIDENCE_FORMAT, "state": "absent"}
    except OSError as exc:
        raise RetirementDeleteError("package-observation-failed") from exc
    if stat.S_ISLNK(item.st_mode):
        state = "symlink"
    elif stat.S_ISDIR(item.st_mode):
        state = "directory"
    elif stat.S_ISREG(item.st_mode):
        state = "regular-file"
    else:
        state = "other"
    return {
        "format": EVIDENCE_FORMAT,
        "mode": f"{stat.S_IMODE(item.st_mode):04o}",
        "state": state,
    }


def _asset_path(request: DeleteRequest, asset: prepare.RetirementAsset) -> Path:
    root = (
        request.operator_asset_root
        if asset.storage == "operator"
        else request.utm_documents_root
    )
    return root / asset.relative_path


def _expected_lsof_argv(
    request: DeleteRequest, batch: prepare.RetirementBatch
) -> tuple[str, ...]:
    paths: list[str] = []
    for asset in batch.assets:
        root = _asset_path(request, asset)
        paths.extend(
            (
                str(root / "config.plist"),
                str(root / "Data" / "efi_vars.fd"),
                str(root / "Data" / asset.qcow2_name),
            )
        )
    return ("/usr/sbin/lsof", "-n", "-P", "-F", "ctfn", *sorted(paths))


def _read_json(path: Path) -> dict[str, object]:
    try:
        value = json.loads(path.read_bytes())
    except (OSError, json.JSONDecodeError) as exc:
        raise RetirementDeleteError("prior-prepare-json-invalid") from exc
    if not isinstance(value, dict):
        raise RetirementDeleteError("prior-prepare-json-not-object")
    return value


def _observation_from_json(
    path: Path, expected_argv: tuple[str, ...]
) -> clone_control.CommandObservation:
    value = _read_json(path)
    if set(value) != {"argv", "exit_code", "timed_out", "stdout", "stderr"}:
        raise RetirementDeleteError("prior-observation-keys-invalid")
    if value["argv"] != list(expected_argv):
        raise RetirementDeleteError("prior-observation-argv-drift")
    stdout = _captured_bytes(value["stdout"])
    stderr = _captured_bytes(value["stderr"])
    observation = clone_control.CommandObservation.from_bytes(
        expected_argv,
        exit_code=value["exit_code"],
        timed_out=value["timed_out"],
        stdout=stdout,
        stderr=stderr,
    )
    if observation.as_json() != value:
        raise RetirementDeleteError("prior-observation-content-drift")
    return observation


def _captured_bytes(value: object) -> bytes:
    if not isinstance(value, dict) or set(value) != {
        "prefix_base64",
        "prefix_utf8",
        "sha256",
        "total_bytes",
        "truncated",
    }:
        raise RetirementDeleteError("prior-captured-output-keys-invalid")
    if value.get("truncated") is not False:
        raise RetirementDeleteError("prior-captured-output-truncated")
    try:
        payload = base64.b64decode(value["prefix_base64"], validate=True)
    except (TypeError, ValueError) as exc:
        raise RetirementDeleteError("prior-captured-output-base64-invalid") from exc
    if (
        value.get("total_bytes") != len(payload)
        or value.get("sha256") != hashlib.sha256(payload).hexdigest()
        or value.get("prefix_utf8") != payload.decode("utf-8", errors="replace")
    ):
        raise RetirementDeleteError("prior-captured-output-identity-drift")
    return payload


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Delete exactly one supported prepared retirement batch, at most once "
            "per UUID, with exact inventory deltas and no retry or rollback."
        )
    )
    parser.add_argument("command", choices=("delete-once",))
    parser.add_argument("--repository-root", type=Path, required=True)
    parser.add_argument("--expected-repository-head", required=True)
    parser.add_argument("--output-root", type=Path, required=True)
    parser.add_argument("--attempt-id", required=True)
    parser.add_argument("--operator-asset-root", type=Path, required=True)
    parser.add_argument("--utm-documents-root", type=Path, required=True)
    parser.add_argument("--expected-allowlist-sha256", required=True)
    parser.add_argument("--batch-id", required=True)
    parser.add_argument("--prior-prepare-root", type=Path, required=True)
    parser.add_argument("--prior-prepare-manifest-sha256", required=True)
    parser.add_argument("--expected-predelete-vm-count", type=int, required=True)
    parser.add_argument("--expected-predelete-inventory-sha256", required=True)
    parser.add_argument("--command-timeout-seconds", type=int, default=15)
    parser.add_argument("--delete-timeout-seconds", type=int, default=60)
    parser.add_argument(
        "--authorized-delete-first-four-v1", action="store_true"
    )
    parser.add_argument(
        "--authorized-delete-second-batch-v1", action="store_true"
    )
    parser.add_argument(
        "--authorized-delete-third-batch-v1", action="store_true"
    )
    parser.add_argument(
        "--authorized-delete-fourth-batch-v1", action="store_true"
    )
    parser.add_argument(
        "--authorized-delete-fifth-batch-v1", action="store_true"
    )
    parser.add_argument(
        "--authorized-at-most-one-delete-per-asset", action="store_true"
    )
    parser.add_argument(
        "--authorized-stop-without-retry-or-rollback", action="store_true"
    )
    parser.add_argument(
        "--acknowledge-irreversible-bundle-removal", action="store_true"
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    request = DeleteRequest(
        repository_root=args.repository_root,
        expected_repository_head=args.expected_repository_head,
        output_root=args.output_root,
        attempt_id=args.attempt_id,
        operator_asset_root=args.operator_asset_root,
        utm_documents_root=args.utm_documents_root,
        expected_allowlist_sha256=args.expected_allowlist_sha256,
        batch_id=args.batch_id,
        prior_prepare_root=args.prior_prepare_root,
        prior_prepare_manifest_sha256=args.prior_prepare_manifest_sha256,
        expected_predelete_vm_count=args.expected_predelete_vm_count,
        expected_predelete_inventory_sha256=(
            args.expected_predelete_inventory_sha256
        ),
        command_timeout_seconds=args.command_timeout_seconds,
        delete_timeout_seconds=args.delete_timeout_seconds,
        authorized_delete_first_four_v1=args.authorized_delete_first_four_v1,
        authorized_delete_second_batch_v1=(
            args.authorized_delete_second_batch_v1
        ),
        authorized_delete_third_batch_v1=(
            args.authorized_delete_third_batch_v1
        ),
        authorized_delete_fourth_batch_v1=(
            args.authorized_delete_fourth_batch_v1
        ),
        authorized_delete_fifth_batch_v1=(
            args.authorized_delete_fifth_batch_v1
        ),
        authorized_at_most_one_delete_per_asset=(
            args.authorized_at_most_one_delete_per_asset
        ),
        authorized_stop_without_retry_or_rollback=(
            args.authorized_stop_without_retry_or_rollback
        ),
        acknowledge_irreversible_bundle_removal=(
            args.acknowledge_irreversible_bundle_removal
        ),
    )
    try:
        result = run_delete(request)
    except (
        RetirementDeleteError,
        prepare.RetirementPrepareError,
        clone_control.CloneControlError,
    ) as exc:
        print(f"l6_asset_retirement_delete_error={exc}", file=sys.stderr)
        return EXIT_PRECONDITION_REJECTED
    print(f"asset_retirement_delete_outcome={result.outcome}")
    print(f"deleted_assets={result.deleted_assets}")
    print(f"evidence_root={result.evidence_root}")
    print(f"files_sha256={result.manifest_sha256}")
    return result.exit_code


if __name__ == "__main__":
    raise SystemExit(main())
