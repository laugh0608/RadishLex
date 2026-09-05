#!/usr/bin/env python3
from __future__ import annotations

import argparse
import os
import re
import sys
import uuid
from dataclasses import dataclass
from pathlib import Path
from typing import Callable, Protocol

import l6_upgrade_quiesced_case_contract as case_contract
import l6_upgrade_quiesced_clone_bindings as clone_bindings
import l6_upgrade_quiesced_clone_partial_recovery_bindings as bindings
import l6_utm_clone_once as clone_control


EVIDENCE_FORMAT = bindings.EVIDENCE_FORMAT
EXIT_COMPLETE = 0
EXIT_PRECONDITION_REJECTED = 11
EXIT_STATE_INDETERMINATE = 12
HEX_40 = re.compile(r"[0-9a-f]{40}")
HEX_64 = re.compile(r"[0-9a-f]{64}")
SAFE_ATTEMPT_ID = re.compile(r"[a-z0-9][a-z0-9-]{0,95}")


class RecoveryControlError(ValueError):
    pass


class CommandRunner(Protocol):
    def run(
        self, argv: tuple[str, ...], timeout_seconds: int
    ) -> clone_control.CommandObservation: ...


@dataclass(frozen=True)
class RecoveryRequestBase:
    repository_root: Path
    expected_repository_head: str
    output_root: Path
    attempt_id: str
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
    command_timeout_seconds: int

    def validate_common(self) -> None:
        paths = (
            (self.repository_root, "repository-root"),
            (self.output_root, "output-root"),
            (self.operator_asset_root, "operator-asset-root"),
            (self.utm_documents_root, "utm-documents-root"),
            (self.asset_retirement_delete_root, "retirement-root"),
            (self.predecessor_failure_root, "predecessor-root"),
            (self.partial_clone_root, "partial-clone-root"),
            (self.source_snapshot_root, "source-snapshot-root"),
            (self.registration_shell_evidence_root, "shell-evidence-root"),
            (self.registration_shell_package_path, "shell-package-path"),
            (self.default_target_package_path, "default-target-path"),
            (self.target_package_path, "target-package-path"),
        )
        for path, label in paths:
            if not path.is_absolute() or ".." in path.parts:
                raise RecoveryControlError(
                    f"{label}-must-be-absolute-normalized"
                )
        if clone_control._path_is_within(
            self.output_root, self.repository_root
        ):
            raise RecoveryControlError("output-root-must-be-outside-repository")
        protected = (
            self.utm_documents_root,
            self.asset_retirement_delete_root,
            self.predecessor_failure_root,
            self.partial_clone_root,
            self.source_snapshot_root,
            self.registration_shell_evidence_root,
            self.registration_shell_package_path,
            self.default_target_package_path,
            self.target_package_path,
        )
        if any(
            clone_control._paths_overlap(self.output_root, path)
            for path in protected
        ):
            raise RecoveryControlError("output-root-overlaps-protected-path")
        partial = case_contract.EXPECTED_CLONE_FRONT_DOOR[
            "partial_recovery"
        ]["partial_clone"]
        expected_partial_root = self.operator_asset_root / str(
            partial["evidence_relative_path"]
        )
        expected_source = self.operator_asset_root / str(
            case_contract.EXPECTED_START["relative_path"]
        )
        expected_default_target = self.utm_documents_root / (
            f"{partial['target_name']}.utm"
        )
        expected_target = self.operator_asset_root / (
            f"{partial['target_name']}.utm"
        )
        baseline = case_contract.EXPECTED_CLONE_FRONT_DOOR[
            "preclone_baseline"
        ]
        predecessor = case_contract.EXPECTED_CLONE_FRONT_DOOR[
            "predecessor_failure"
        ]
        expected_delete = self.operator_asset_root / str(
            baseline["delete_evidence_relative_path"]
        )
        expected_predecessor = self.operator_asset_root / str(
            predecessor["evidence_relative_path"]
        )
        if self.partial_clone_root != expected_partial_root:
            raise RecoveryControlError("partial-clone-root-drift")
        if self.source_snapshot_root != expected_source:
            raise RecoveryControlError("source-snapshot-root-drift")
        if self.default_target_package_path != expected_default_target:
            raise RecoveryControlError("default-target-path-drift")
        if self.target_package_path != expected_target:
            raise RecoveryControlError("target-package-path-drift")
        if self.asset_retirement_delete_root != expected_delete:
            raise RecoveryControlError("retirement-root-drift")
        if self.predecessor_failure_root != expected_predecessor:
            raise RecoveryControlError("predecessor-root-drift")
        if self.registration_shell_package_path.parent != (
            self.operator_asset_root
        ):
            raise RecoveryControlError("registration-shell-path-drift")
        if self.default_target_package_path == self.target_package_path:
            raise RecoveryControlError("default-and-authorized-target-overlap")
        for value, expected, label in (
            (
                self.asset_retirement_delete_manifest_sha256,
                baseline["delete_manifest_sha256"],
                "retirement-manifest",
            ),
            (
                self.predecessor_failure_manifest_sha256,
                predecessor["manifest_sha256"],
                "predecessor-manifest",
            ),
            (
                self.partial_clone_manifest_sha256,
                partial["manifest_sha256"],
                "partial-clone-manifest",
            ),
        ):
            if value != expected:
                raise RecoveryControlError(f"{label}-drift")
        if self.target_name != partial["target_name"]:
            raise RecoveryControlError("target-name-drift")
        if self.target_uuid != partial["target_uuid"]:
            raise RecoveryControlError("target-uuid-drift")
        if self.expected_vm_count != partial["postclone_registered_vm_count"]:
            raise RecoveryControlError("expected-vm-count-drift")
        if self.expected_inventory_sha256 != partial[
            "postclone_inventory_sha256"
        ]:
            raise RecoveryControlError("expected-inventory-drift")
        if not HEX_40.fullmatch(self.expected_repository_head):
            raise RecoveryControlError("expected-repository-head-invalid")
        for value, label in (
            (
                self.registration_shell_manifest_sha256,
                "registration-shell-manifest",
            ),
            (self.expected_inventory_sha256, "expected-inventory"),
        ):
            if not HEX_64.fullmatch(value):
                raise RecoveryControlError(f"{label}-invalid")
        if not SAFE_ATTEMPT_ID.fullmatch(self.attempt_id):
            raise RecoveryControlError("attempt-id-invalid")
        _validate_uuid(self.registration_shell_uuid, "registration-shell-uuid")
        _validate_uuid(self.target_uuid, "target-uuid")
        _validate_vm_name(self.registration_shell_name, "registration-shell-name")
        _validate_vm_name(self.target_name, "target-name")
        if not 1 <= self.command_timeout_seconds <= 60:
            raise RecoveryControlError("command-timeout-seconds-out-of-range")

    def common_json(self, phase: str) -> dict[str, object]:
        return {
            "asset_retirement_delete_manifest_sha256": (
                self.asset_retirement_delete_manifest_sha256
            ),
            "attempt_id": self.attempt_id,
            "command_timeout_seconds": self.command_timeout_seconds,
            "default_target_package_path_sha256": clone_bindings.sha256_text(
                str(self.default_target_package_path)
            ),
            "expected_inventory_sha256": self.expected_inventory_sha256,
            "expected_repository_head": self.expected_repository_head,
            "expected_vm_count": self.expected_vm_count,
            "format": EVIDENCE_FORMAT,
            "operator_asset_root_sha256": clone_bindings.sha256_text(
                str(self.operator_asset_root)
            ),
            "partial_clone_manifest_sha256": (
                self.partial_clone_manifest_sha256
            ),
            "partial_clone_root_sha256": clone_bindings.sha256_text(
                str(self.partial_clone_root)
            ),
            "phase": phase,
            "predecessor_failure_manifest_sha256": (
                self.predecessor_failure_manifest_sha256
            ),
            "registration_shell_manifest_sha256": (
                self.registration_shell_manifest_sha256
            ),
            "target_name": self.target_name,
            "target_package_path_sha256": clone_bindings.sha256_text(
                str(self.target_package_path)
            ),
            "target_uuid": self.target_uuid,
            "utm_documents_root_sha256": clone_bindings.sha256_text(
                str(self.utm_documents_root)
            ),
        }


@dataclass(frozen=True)
class MovePrepareRequest(RecoveryRequestBase):
    authorized_move_prepare: bool
    authorized_one_inventory_query: bool
    authorized_emit_one_utm_ui_move: bool
    authorized_no_move_materialize_start_clone_delete_retry_guest_or_transaction: bool

    def validate(self) -> None:
        self.validate_common()
        move = case_contract.EXPECTED_CLONE_FRONT_DOOR[
            "partial_recovery"
        ]["move"]
        _require_phase_identity(
            self,
            str(move["prepare_attempt_id"]),
            str(move["prepare_evidence_relative_path"]),
            "move-prepare",
        )
        no_mutation = getattr(
            self,
            (
                "authorized_no_move_materialize_start_clone_delete_retry_"
                "guest_or_transaction"
            ),
        )
        if not all(
            (
                self.authorized_move_prepare,
                self.authorized_one_inventory_query,
                self.authorized_emit_one_utm_ui_move,
                no_mutation,
            )
        ):
            raise RecoveryControlError("move-prepare-authorization-required")

    def as_json(self) -> dict[str, object]:
        return {
            **self.common_json("move-prepare"),
            "authorization": {
                "emit_one_utm_ui_move": True,
                "move_prepare": True,
                (
                    "no_move_materialize_start_clone_delete_retry_guest_or_"
                    "transaction"
                ): True,
                "one_inventory_query": True,
            },
        }


@dataclass(frozen=True)
class MoveAdoptRequest(RecoveryRequestBase):
    move_prepare_root: Path
    move_prepare_manifest_sha256: str
    authorized_adopt_one_utm_ui_move_result: bool
    authorized_two_inventory_queries: bool
    authorized_no_materialize_start_clone_delete_retry_guest_or_transaction: bool

    def validate(self) -> None:
        self.validate_common()
        move = case_contract.EXPECTED_CLONE_FRONT_DOOR[
            "partial_recovery"
        ]["move"]
        _require_phase_identity(
            self,
            str(move["adopt_attempt_id"]),
            str(move["adopt_evidence_relative_path"]),
            "move-adopt",
        )
        expected_prepare = self.operator_asset_root / str(
            move["prepare_evidence_relative_path"]
        )
        if self.move_prepare_root != expected_prepare:
            raise RecoveryControlError("move-prepare-root-drift")
        _validate_evidence_input(
            self.move_prepare_root,
            self.move_prepare_manifest_sha256,
            self.repository_root,
            self.output_root,
            "move-prepare",
        )
        no_materialization = (
            self.authorized_no_materialize_start_clone_delete_retry_guest_or_transaction
        )
        if not all(
            (
                self.authorized_adopt_one_utm_ui_move_result,
                self.authorized_two_inventory_queries,
                no_materialization,
            )
        ):
            raise RecoveryControlError("move-adopt-authorization-required")

    def as_json(self) -> dict[str, object]:
        return {
            **self.common_json("move-adopt"),
            "authorization": {
                "adopt_one_utm_ui_move_result": True,
                "no_materialize_start_clone_delete_retry_guest_or_transaction": True,
                "two_inventory_queries": True,
            },
            "move_prepare_manifest_sha256": (
                self.move_prepare_manifest_sha256
            ),
            "move_prepare_root_sha256": clone_bindings.sha256_text(
                str(self.move_prepare_root)
            ),
        }


@dataclass(frozen=True)
class MaterializeRequest(RecoveryRequestBase):
    move_prepare_root: Path
    move_prepare_manifest_sha256: str
    move_adopt_root: Path
    move_adopt_manifest_sha256: str
    clonefile_timeout_seconds: int
    authorized_materialize_s2_once: bool
    authorized_two_inventory_queries: bool
    authorized_two_clonefile_copies: bool
    authorized_two_atomic_replacements: bool
    authorized_no_move_start_clone_delete_retry_rollback_guest_or_transaction: bool

    def validate(self) -> None:
        self.validate_common()
        recovery = case_contract.EXPECTED_CLONE_FRONT_DOOR[
            "partial_recovery"
        ]
        move = recovery["move"]
        materialization = recovery["materialization"]
        _require_phase_identity(
            self,
            str(materialization["attempt_id"]),
            str(materialization["evidence_relative_path"]),
            "materialize",
        )
        if self.move_prepare_root != self.operator_asset_root / str(
            move["prepare_evidence_relative_path"]
        ):
            raise RecoveryControlError("move-prepare-root-drift")
        if self.move_adopt_root != self.operator_asset_root / str(
            move["adopt_evidence_relative_path"]
        ):
            raise RecoveryControlError("move-adopt-root-drift")
        _validate_evidence_input(
            self.move_prepare_root,
            self.move_prepare_manifest_sha256,
            self.repository_root,
            self.output_root,
            "move-prepare",
        )
        _validate_evidence_input(
            self.move_adopt_root,
            self.move_adopt_manifest_sha256,
            self.repository_root,
            self.output_root,
            "move-adopt",
        )
        if clone_control._paths_overlap(
            self.move_prepare_root, self.move_adopt_root
        ):
            raise RecoveryControlError("move-evidence-roots-overlap")
        if not 1 <= self.clonefile_timeout_seconds <= 300:
            raise RecoveryControlError("clonefile-timeout-seconds-out-of-range")
        no_other_mutation = getattr(
            self,
            (
                "authorized_no_move_start_clone_delete_retry_rollback_"
                "guest_or_transaction"
            ),
        )
        if not all(
            (
                self.authorized_materialize_s2_once,
                self.authorized_two_inventory_queries,
                self.authorized_two_clonefile_copies,
                self.authorized_two_atomic_replacements,
                no_other_mutation,
            )
        ):
            raise RecoveryControlError("materialize-authorization-required")

    def as_json(self) -> dict[str, object]:
        return {
            **self.common_json("materialize"),
            "authorization": {
                "materialize_s2_once": True,
                "no_move_start_clone_delete_retry_rollback_guest_or_transaction": True,
                "two_atomic_replacements": True,
                "two_clonefile_copies": True,
                "two_inventory_queries": True,
            },
            "clonefile_timeout_seconds": self.clonefile_timeout_seconds,
            "move_adopt_manifest_sha256": self.move_adopt_manifest_sha256,
            "move_adopt_root_sha256": clone_bindings.sha256_text(
                str(self.move_adopt_root)
            ),
            "move_prepare_manifest_sha256": (
                self.move_prepare_manifest_sha256
            ),
            "move_prepare_root_sha256": clone_bindings.sha256_text(
                str(self.move_prepare_root)
            ),
        }


@dataclass(frozen=True)
class RecoveryResult:
    outcome: str
    exit_code: int
    evidence_root: Path
    manifest_sha256: str
    inventory_query_invocations: int
    copy_invocations: int
    replacement_count: int


PrepareBindingValidator = Callable[[MovePrepareRequest], dict[str, object]]
AdoptBindingValidator = Callable[[MoveAdoptRequest], dict[str, object]]
MaterializeBindingValidator = Callable[[MaterializeRequest], dict[str, object]]


def run_move_prepare(
    request: MovePrepareRequest,
    *,
    runner: CommandRunner | None = None,
    binding_validator: PrepareBindingValidator | None = None,
) -> RecoveryResult:
    request.validate()
    writer = clone_control.EvidenceWriter.create(request.output_root)
    writer.write_json("request.json", request.as_json())
    command_runner = runner or clone_control.SubprocessCommandRunner()
    validate_bindings = binding_validator or bindings.validate_prepare_bindings
    stage = "binding-preflight"
    outcome = "precondition-rejected"
    reason = "not-run"
    queries = 0
    try:
        writer.write_json("binding-preflight.json", validate_bindings(request))
        source, registration = _validate_source_and_registration(request, writer)
        stage = "default-target-preflight"
        partial = bindings.validate_default_partial_target(
            request, registration, source
        )
        writer.write_json("default-target-preflight.json", partial.as_json())
        stage = "authorized-target-preflight"
        _write_and_require_absent(
            writer,
            "authorized-target-preflight.json",
            request.target_package_path,
        )
        stage = "source-default-authorized-device"
        _require_same_device(
            source,
            partial,
            request.target_package_path.parent,
        )
        writer.write_json(
            "source-default-authorized-device.json",
            {"format": EVIDENCE_FORMAT, "same_device": True},
        )
        handles_argv = bindings._lsof_argv(source, registration, partial)
        stage = "source-shell-default-handles-preflight"
        handles = command_runner.run(
            handles_argv, request.command_timeout_seconds
        )
        writer.write_json(
            "source-shell-default-handles-preflight.json", handles.as_json()
        )
        bindings._require_zero_handles(handles, handles_argv)
        stage = "utmctl-list-pre-move"
        queries = 1
        inventory = _run_inventory(
            command_runner,
            request,
            writer,
            "utmctl-list-pre-move.json",
            "inventory-pre-move.json",
        )
        del inventory
        stage = "source-shell-default-handles-post-list"
        handles_terminal = command_runner.run(
            handles_argv, request.command_timeout_seconds
        )
        writer.write_json(
            "source-shell-default-handles-post-list.json",
            handles_terminal.as_json(),
        )
        bindings._require_zero_handles(handles_terminal, handles_argv)
        _validate_prepare_terminal_state(
            request, writer, source, registration, partial
        )
        outcome = "move-ready"
        reason = "one-utm-ui-move-may-be-authorized-separately"
    except _RECOVERY_EXCEPTIONS as exc:
        reason = f"{stage}:{exc}"
    writer.write_json(
        "terminal.json",
        _terminal(
            request,
            outcome=outcome,
            reason=reason,
            inventory_queries=queries,
            copy_invocations=0,
            materialization_invocations=0,
            replacement_count=0,
            next_external_action=(
                "one-utm-ui-move-to-authorized-target-path"
                if outcome == "move-ready"
                else "none"
            ),
        ),
    )
    return _result(
        request,
        writer,
        outcome,
        EXIT_COMPLETE if outcome == "move-ready" else EXIT_PRECONDITION_REJECTED,
        queries,
        0,
        0,
    )


def run_move_adopt(
    request: MoveAdoptRequest,
    *,
    runner: CommandRunner | None = None,
    binding_validator: AdoptBindingValidator | None = None,
) -> RecoveryResult:
    request.validate()
    writer = clone_control.EvidenceWriter.create(request.output_root)
    writer.write_json("request.json", request.as_json())
    command_runner = runner or clone_control.SubprocessCommandRunner()
    validate_bindings = binding_validator or bindings.validate_adopt_bindings
    stage = "binding-preflight"
    outcome = "state-indeterminate"
    reason = "not-run"
    queries = 0
    try:
        writer.write_json("binding-preflight.json", validate_bindings(request))
        source, registration = _validate_source_and_registration(request, writer)
        stage = "default-target-preflight"
        _write_and_require_absent(
            writer,
            "default-target-preflight.json",
            request.default_target_package_path,
        )
        stage = "authorized-target-preflight"
        partial = bindings.validate_authorized_partial_target(
            request, registration, source
        )
        writer.write_json("authorized-target-preflight.json", partial.as_json())
        stage = "source-authorized-device"
        _require_same_device(source, partial, request.target_package_path.parent)
        writer.write_json(
            "source-authorized-device.json",
            {"format": EVIDENCE_FORMAT, "same_device": True},
        )
        handles_argv = bindings._lsof_argv(source, registration, partial)
        stage = "source-shell-authorized-handles-preflight"
        handles = command_runner.run(
            handles_argv, request.command_timeout_seconds
        )
        writer.write_json(
            "source-shell-authorized-handles-preflight.json", handles.as_json()
        )
        bindings._require_zero_handles(handles, handles_argv)
        stage = "utmctl-list-pre-adopt"
        queries = 1
        first = _run_inventory(
            command_runner,
            request,
            writer,
            "utmctl-list-pre-adopt.json",
            "inventory-pre-adopt.json",
        )
        stage = "authorized-target-post-list"
        after_list = bindings.validate_authorized_partial_target(
            request, registration, source
        )
        writer.write_json("authorized-target-post-list.json", after_list.as_json())
        if after_list != partial:
            raise RecoveryControlError("authorized-target-drift-after-list")
        stage = "utmctl-list-terminal"
        queries = 2
        terminal_inventory = _run_inventory(
            command_runner,
            request,
            writer,
            "utmctl-list-terminal.json",
            "inventory-terminal.json",
        )
        if terminal_inventory != first:
            raise RecoveryControlError("inventory-drift-between-adopt-queries")
        stage = "source-shell-authorized-handles-terminal"
        handles_terminal = command_runner.run(
            handles_argv, request.command_timeout_seconds
        )
        writer.write_json(
            "source-shell-authorized-handles-terminal.json",
            handles_terminal.as_json(),
        )
        bindings._require_zero_handles(handles_terminal, handles_argv)
        _validate_adopt_terminal_state(
            request, writer, source, registration, partial
        )
        outcome = "move-adopted"
        reason = "utm-ui-move-adopted-with-partial-bytes-unchanged"
    except _RECOVERY_EXCEPTIONS as exc:
        reason = f"{stage}:{exc}"
    writer.write_json(
        "terminal.json",
        _terminal(
            request,
            outcome=outcome,
            reason=reason,
            inventory_queries=queries,
            copy_invocations=0,
            materialization_invocations=0,
            replacement_count=0,
            next_external_action="none",
        ),
    )
    return _result(
        request,
        writer,
        outcome,
        EXIT_COMPLETE if outcome == "move-adopted" else EXIT_STATE_INDETERMINATE,
        queries,
        0,
        0,
    )


def run_materialize(
    request: MaterializeRequest,
    *,
    runner: CommandRunner | None = None,
    binding_validator: MaterializeBindingValidator | None = None,
) -> RecoveryResult:
    request.validate()
    writer = clone_control.EvidenceWriter.create(request.output_root)
    writer.write_json("request.json", request.as_json())
    command_runner = runner or clone_control.SubprocessCommandRunner()
    validate_bindings = (
        binding_validator or bindings.validate_materialize_bindings
    )
    stage = "binding-preflight"
    outcome = "precondition-rejected"
    reason = "not-run"
    queries = 0
    copies = 0
    replacements = 0
    try:
        writer.write_json("binding-preflight.json", validate_bindings(request))
        source, registration = _validate_source_and_registration(request, writer)
        stage = "default-target-preflight"
        _write_and_require_absent(
            writer,
            "default-target-preflight.json",
            request.default_target_package_path,
        )
        stage = "target-before-materialization"
        target = bindings.validate_authorized_partial_target(
            request, registration, source
        )
        writer.write_json("target-before-materialization.json", target.as_json())
        stage = "source-target-device"
        _require_same_device(source, target, request.target_package_path.parent)
        writer.write_json(
            "source-target-device.json",
            {"format": EVIDENCE_FORMAT, "same_device": True},
        )
        handles_argv = bindings._lsof_argv(source, registration, target)
        stage = "source-shell-target-handles-preflight"
        handles = command_runner.run(
            handles_argv, request.command_timeout_seconds
        )
        writer.write_json(
            "source-shell-target-handles-preflight.json", handles.as_json()
        )
        bindings._require_zero_handles(handles, handles_argv)
        stage = "utmctl-list-prematerialization"
        queries = 1
        pre = _run_inventory(
            command_runner,
            request,
            writer,
            "utmctl-list-prematerialization.json",
            "inventory-prematerialization.json",
        )
        stage = "target-post-list"
        target_post_list = bindings.validate_authorized_partial_target(
            request, registration, source
        )
        writer.write_json("target-post-list.json", target_post_list.as_json())
        if target_post_list != target:
            raise RecoveryControlError("target-drift-after-list")
        stage = "source-shell-target-handles-post-list"
        handles_post_list = command_runner.run(
            handles_argv, request.command_timeout_seconds
        )
        writer.write_json(
            "source-shell-target-handles-post-list.json",
            handles_post_list.as_json(),
        )
        bindings._require_zero_handles(handles_post_list, handles_argv)
        data = request.target_package_path / "Data"
        efi_incoming = data / f".efi_vars.fd.{request.attempt_id}.incoming"
        qcow2_incoming = data / (
            f".{source.qcow2_name}.{request.attempt_id}.incoming"
        )
        if any(
            path.exists() or path.is_symlink()
            for path in (efi_incoming, qcow2_incoming)
        ):
            raise RecoveryControlError("target-incoming-already-exists")
        stage = "clonefile-efi"
        copies = 1
        efi_copy = command_runner.run(
            ("/bin/cp", "-c", str(source.efi_path), str(efi_incoming)),
            request.clonefile_timeout_seconds,
        )
        writer.write_json("clonefile-efi.json", efi_copy.as_json())
        _require_silent_success(efi_copy, "clonefile-efi")
        clone_bindings._validate_regular_file(
            efi_incoming, 0o644, source.efi_sha256, "incoming-efi"
        )
        stage = "clonefile-qcow2"
        copies = 2
        qcow2_copy = command_runner.run(
            ("/bin/cp", "-c", str(source.qcow2_path), str(qcow2_incoming)),
            request.clonefile_timeout_seconds,
        )
        writer.write_json("clonefile-qcow2.json", qcow2_copy.as_json())
        _require_silent_success(qcow2_copy, "clonefile-qcow2")
        clone_bindings._validate_regular_file(
            qcow2_incoming, 0o644, source.qcow2_sha256, "incoming-qcow2"
        )
        stage = "source-shell-target-handles-pre-replacement"
        handles_pre_replace = command_runner.run(
            handles_argv, request.command_timeout_seconds
        )
        writer.write_json(
            "source-shell-target-handles-pre-replacement.json",
            handles_pre_replace.as_json(),
        )
        bindings._require_zero_handles(handles_pre_replace, handles_argv)
        stage = "replace-target-efi"
        efi_replace = command_runner.run(
            ("/bin/mv", "-f", str(efi_incoming), str(target.efi_path)),
            request.command_timeout_seconds,
        )
        writer.write_json("replace-target-efi.json", efi_replace.as_json())
        _require_silent_success(efi_replace, "replace-target-efi")
        replacements = 1
        _fsync_directory(data)
        stage = "source-shell-target-handles-between-replacements"
        between_argv = bindings._lsof_argv(source, registration, target)
        handles_between = command_runner.run(
            between_argv, request.command_timeout_seconds
        )
        writer.write_json(
            "source-shell-target-handles-between-replacements.json",
            handles_between.as_json(),
        )
        bindings._require_zero_handles(handles_between, between_argv)
        stage = "replace-target-qcow2"
        qcow2_replace = command_runner.run(
            ("/bin/mv", "-f", str(qcow2_incoming), str(target.qcow2_path)),
            request.command_timeout_seconds,
        )
        writer.write_json("replace-target-qcow2.json", qcow2_replace.as_json())
        _require_silent_success(qcow2_replace, "replace-target-qcow2")
        replacements = 2
        _fsync_directory(data)
        stage = "target-after-materialization"
        materialized = bindings.validate_materialized_target(
            request, registration, source
        )
        writer.write_json(
            "target-after-materialization.json", materialized.as_json()
        )
        if materialized.config_sha256 != target.config_sha256:
            raise RecoveryControlError("target-config-changed-during-materialization")
        stage = "utmctl-list-terminal"
        queries = 2
        terminal_inventory = _run_inventory(
            command_runner,
            request,
            writer,
            "utmctl-list-terminal.json",
            "inventory-terminal.json",
        )
        if terminal_inventory != pre:
            raise RecoveryControlError("inventory-drift-during-materialization")
        terminal_handles_argv = bindings._lsof_argv(
            source, registration, materialized
        )
        stage = "source-shell-target-handles-terminal"
        terminal_handles = command_runner.run(
            terminal_handles_argv, request.command_timeout_seconds
        )
        writer.write_json(
            "source-shell-target-handles-terminal.json",
            terminal_handles.as_json(),
        )
        bindings._require_zero_handles(
            terminal_handles, terminal_handles_argv
        )
        _validate_materialize_terminal_state(
            request, writer, source, registration, materialized
        )
        outcome = "materialized"
        reason = "authorized-target-has-exact-s2-efi-and-qcow2"
    except _RECOVERY_EXCEPTIONS as exc:
        reason = f"{stage}:{exc}"
        if copies or replacements:
            outcome = "state-indeterminate"
    writer.write_json(
        "terminal.json",
        _terminal(
            request,
            outcome=outcome,
            reason=reason,
            inventory_queries=queries,
            copy_invocations=copies,
            materialization_invocations=1 if copies else 0,
            replacement_count=replacements,
            next_external_action="none",
        ),
    )
    exit_code = (
        EXIT_COMPLETE
        if outcome == "materialized"
        else EXIT_STATE_INDETERMINATE
        if outcome == "state-indeterminate"
        else EXIT_PRECONDITION_REJECTED
    )
    return _result(
        request,
        writer,
        outcome,
        exit_code,
        queries,
        copies,
        replacements,
    )


def _validate_source_and_registration(
    request: RecoveryRequestBase, writer: clone_control.EvidenceWriter
) -> tuple[clone_bindings.BundleIdentity, clone_bindings.BundleIdentity]:
    source = clone_bindings.validate_source_snapshot(request)
    writer.write_json("source-snapshot-preflight.json", source.as_json())
    registration = clone_bindings.validate_registration_shell(request)
    writer.write_json(
        "registration-shell-preflight.json", registration.as_json()
    )
    return source, registration


def _run_inventory(
    runner: CommandRunner,
    request: RecoveryRequestBase,
    writer: clone_control.EvidenceWriter,
    observation_name: str,
    inventory_name: str,
) -> tuple[clone_control.RegisteredVm, ...]:
    observation = runner.run(
        ("utmctl", "list"), request.command_timeout_seconds
    )
    writer.write_json(observation_name, observation.as_json())
    inventory = clone_control.parse_utmctl_list(observation)
    writer.write_json(
        inventory_name, bindings.validate_live_inventory(inventory, request)
    )
    return inventory


def _validate_prepare_terminal_state(
    request: MovePrepareRequest,
    writer: clone_control.EvidenceWriter,
    source: clone_bindings.BundleIdentity,
    registration: clone_bindings.BundleIdentity,
    partial: clone_bindings.BundleIdentity,
) -> None:
    source_terminal = clone_bindings.validate_source_snapshot(request)
    registration_terminal = clone_bindings.validate_registration_shell(request)
    partial_terminal = bindings.validate_default_partial_target(
        request, registration, source
    )
    writer.write_json("source-snapshot-terminal.json", source_terminal.as_json())
    writer.write_json(
        "registration-shell-terminal.json", registration_terminal.as_json()
    )
    writer.write_json("default-target-terminal.json", partial_terminal.as_json())
    _write_and_require_absent(
        writer, "authorized-target-terminal.json", request.target_package_path
    )
    if (
        source_terminal != source
        or registration_terminal != registration
        or partial_terminal != partial
    ):
        raise RecoveryControlError("move-prepare-terminal-identity-drift")


def _validate_adopt_terminal_state(
    request: MoveAdoptRequest,
    writer: clone_control.EvidenceWriter,
    source: clone_bindings.BundleIdentity,
    registration: clone_bindings.BundleIdentity,
    partial: clone_bindings.BundleIdentity,
) -> None:
    source_terminal = clone_bindings.validate_source_snapshot(request)
    registration_terminal = clone_bindings.validate_registration_shell(request)
    default_terminal = _require_absent(request.default_target_package_path)
    partial_terminal = bindings.validate_authorized_partial_target(
        request, registration, source
    )
    writer.write_json("source-snapshot-terminal.json", source_terminal.as_json())
    writer.write_json(
        "registration-shell-terminal.json", registration_terminal.as_json()
    )
    writer.write_json("default-target-terminal.json", default_terminal)
    writer.write_json(
        "authorized-target-terminal.json", partial_terminal.as_json()
    )
    if (
        source_terminal != source
        or registration_terminal != registration
        or partial_terminal != partial
    ):
        raise RecoveryControlError("move-adopt-terminal-identity-drift")


def _validate_materialize_terminal_state(
    request: MaterializeRequest,
    writer: clone_control.EvidenceWriter,
    source: clone_bindings.BundleIdentity,
    registration: clone_bindings.BundleIdentity,
    materialized: clone_bindings.BundleIdentity,
) -> None:
    source_terminal = clone_bindings.validate_source_snapshot(request)
    registration_terminal = clone_bindings.validate_registration_shell(request)
    default_terminal = _require_absent(request.default_target_package_path)
    target_terminal = bindings.validate_materialized_target(
        request, registration, source
    )
    writer.write_json("source-snapshot-terminal.json", source_terminal.as_json())
    writer.write_json(
        "registration-shell-terminal.json", registration_terminal.as_json()
    )
    writer.write_json("default-target-terminal.json", default_terminal)
    writer.write_json("target-terminal.json", target_terminal.as_json())
    if (
        source_terminal != source
        or registration_terminal != registration
        or target_terminal != materialized
    ):
        raise RecoveryControlError("materialization-terminal-identity-drift")


def _write_and_require_absent(
    writer: clone_control.EvidenceWriter, name: str, path: Path
) -> None:
    evidence = _require_absent(path)
    writer.write_json(name, evidence)


def _require_absent(path: Path) -> dict[str, object]:
    evidence = clone_control.observe_target_package(path)
    if evidence["state"] != "absent":
        raise RecoveryControlError("package-must-be-absent")
    return evidence


def _require_same_device(
    source: clone_bindings.BundleIdentity,
    target: clone_bindings.BundleIdentity,
    destination_parent: Path,
) -> None:
    try:
        devices = {
            source.config_path.parent.lstat().st_dev,
            target.config_path.parent.lstat().st_dev,
            destination_parent.lstat().st_dev,
        }
    except OSError as exc:
        raise RecoveryControlError("same-device-check-unavailable") from exc
    if len(devices) != 1:
        raise RecoveryControlError("source-target-device-mismatch")


def _require_silent_success(
    observation: clone_control.CommandObservation, label: str
) -> None:
    bindings._require_silent_success(observation, label)


def _terminal(
    request: RecoveryRequestBase,
    *,
    outcome: str,
    reason: str,
    inventory_queries: int,
    copy_invocations: int,
    materialization_invocations: int,
    replacement_count: int,
    next_external_action: str,
) -> dict[str, object]:
    return {
        "automatic_delete": "not-performed",
        "automatic_retry": "not-performed",
        "automatic_rollback": "not-performed",
        "automatic_start": "not-performed",
        "clone_invocations": 0,
        "copy_invocations": copy_invocations,
        "format": EVIDENCE_FORMAT,
        "guest_exec": "not-performed",
        "inventory_query_invocations": inventory_queries,
        "materialization_invocations": materialization_invocations,
        "move_invocations": 0,
        "next_external_action": next_external_action,
        "operation_id": "not-generated",
        "outcome": outcome,
        "reason": reason,
        "replacement_count": replacement_count,
        "target_name": request.target_name,
        "target_uuid": request.target_uuid,
        "transaction": "not-performed",
    }


def _result(
    request: RecoveryRequestBase,
    writer: clone_control.EvidenceWriter,
    outcome: str,
    exit_code: int,
    queries: int,
    copies: int,
    replacements: int,
) -> RecoveryResult:
    return RecoveryResult(
        outcome=outcome,
        exit_code=exit_code,
        evidence_root=request.output_root,
        manifest_sha256=writer.write_manifest(),
        inventory_query_invocations=queries,
        copy_invocations=copies,
        replacement_count=replacements,
    )


def _validate_evidence_input(
    path: Path,
    manifest_sha256: str,
    repository_root: Path,
    output_root: Path,
    label: str,
) -> None:
    if not path.is_absolute() or ".." in path.parts:
        raise RecoveryControlError(f"{label}-root-must-be-absolute-normalized")
    if clone_control._path_is_within(path, repository_root):
        raise RecoveryControlError(f"{label}-root-must-be-outside-repository")
    if clone_control._paths_overlap(path, output_root):
        raise RecoveryControlError(f"{label}-root-overlaps-output")
    if not HEX_64.fullmatch(manifest_sha256):
        raise RecoveryControlError(f"{label}-manifest-invalid")


def _require_phase_identity(
    request: RecoveryRequestBase,
    expected_attempt_id: str,
    expected_output_relative_path: str,
    label: str,
) -> None:
    if request.attempt_id != expected_attempt_id:
        raise RecoveryControlError(f"{label}-attempt-id-drift")
    if request.output_root != (
        request.operator_asset_root / expected_output_relative_path
    ):
        raise RecoveryControlError(f"{label}-output-root-drift")


def _validate_uuid(value: str, label: str) -> None:
    try:
        canonical = str(uuid.UUID(value)).upper()
    except ValueError as exc:
        raise RecoveryControlError(f"{label}-invalid") from exc
    if canonical != value:
        raise RecoveryControlError(f"{label}-must-be-uppercase-canonical")


def _validate_vm_name(value: str, label: str) -> None:
    if (
        not value
        or len(value.encode("utf-8")) > 160
        or any(character in "\x00\r\n/" for character in value)
    ):
        raise RecoveryControlError(f"{label}-invalid")


def _fsync_directory(path: Path) -> None:
    descriptor = os.open(path, os.O_RDONLY)
    try:
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


_RECOVERY_EXCEPTIONS = (
    RecoveryControlError,
    bindings.RecoveryBindingError,
    clone_bindings.CloneBindingError,
    clone_control.CloneControlError,
    OSError,
)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Recover the frozen upgrade_quiesced partial clone through a "
            "separately authorized UTM UI move and exact S2 materialization."
        )
    )
    parser.add_argument(
        "command", choices=("move-prepare", "move-adopt", "materialize-once")
    )
    parser.add_argument("--repository-root", type=Path, required=True)
    parser.add_argument("--expected-repository-head", required=True)
    parser.add_argument("--output-root", type=Path, required=True)
    parser.add_argument("--attempt-id", required=True)
    parser.add_argument("--operator-asset-root", type=Path, required=True)
    parser.add_argument("--utm-documents-root", type=Path, required=True)
    parser.add_argument("--asset-retirement-delete-root", type=Path, required=True)
    parser.add_argument("--asset-retirement-delete-manifest-sha256", required=True)
    parser.add_argument("--predecessor-failure-root", type=Path, required=True)
    parser.add_argument("--predecessor-failure-manifest-sha256", required=True)
    parser.add_argument("--partial-clone-root", type=Path, required=True)
    parser.add_argument("--partial-clone-manifest-sha256", required=True)
    parser.add_argument("--source-snapshot-root", type=Path, required=True)
    parser.add_argument("--registration-shell-evidence-root", type=Path, required=True)
    parser.add_argument("--registration-shell-manifest-sha256", required=True)
    parser.add_argument("--registration-shell-uuid", required=True)
    parser.add_argument("--registration-shell-name", required=True)
    parser.add_argument("--registration-shell-package-path", type=Path, required=True)
    parser.add_argument("--target-uuid", required=True)
    parser.add_argument("--target-name", required=True)
    parser.add_argument("--default-target-package-path", type=Path, required=True)
    parser.add_argument("--target-package-path", type=Path, required=True)
    parser.add_argument("--expected-vm-count", type=int, required=True)
    parser.add_argument("--expected-inventory-sha256", required=True)
    parser.add_argument("--command-timeout-seconds", type=int, default=15)
    parser.add_argument("--move-prepare-root", type=Path)
    parser.add_argument("--move-prepare-manifest-sha256")
    parser.add_argument("--move-adopt-root", type=Path)
    parser.add_argument("--move-adopt-manifest-sha256")
    parser.add_argument("--clonefile-timeout-seconds", type=int, default=120)
    for flag in (
        "authorized-move-prepare",
        "authorized-one-inventory-query",
        "authorized-emit-one-utm-ui-move",
        "authorized-no-move-materialize-start-clone-delete-retry-guest-or-transaction",
        "authorized-adopt-one-utm-ui-move-result",
        "authorized-two-inventory-queries",
        "authorized-no-materialize-start-clone-delete-retry-guest-or-transaction",
        "authorized-materialize-s2-once",
        "authorized-two-clonefile-copies",
        "authorized-two-atomic-replacements",
        "authorized-no-move-start-clone-delete-retry-rollback-guest-or-transaction",
    ):
        parser.add_argument(f"--{flag}", action="store_true")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    common = {
        "repository_root": args.repository_root,
        "expected_repository_head": args.expected_repository_head,
        "output_root": args.output_root,
        "attempt_id": args.attempt_id,
        "operator_asset_root": args.operator_asset_root,
        "utm_documents_root": args.utm_documents_root,
        "asset_retirement_delete_root": args.asset_retirement_delete_root,
        "asset_retirement_delete_manifest_sha256": (
            args.asset_retirement_delete_manifest_sha256
        ),
        "predecessor_failure_root": args.predecessor_failure_root,
        "predecessor_failure_manifest_sha256": args.predecessor_failure_manifest_sha256,
        "partial_clone_root": args.partial_clone_root,
        "partial_clone_manifest_sha256": args.partial_clone_manifest_sha256,
        "source_snapshot_root": args.source_snapshot_root,
        "registration_shell_evidence_root": args.registration_shell_evidence_root,
        "registration_shell_manifest_sha256": args.registration_shell_manifest_sha256,
        "registration_shell_uuid": args.registration_shell_uuid,
        "registration_shell_name": args.registration_shell_name,
        "registration_shell_package_path": args.registration_shell_package_path,
        "target_uuid": args.target_uuid,
        "target_name": args.target_name,
        "default_target_package_path": args.default_target_package_path,
        "target_package_path": args.target_package_path,
        "expected_vm_count": args.expected_vm_count,
        "expected_inventory_sha256": args.expected_inventory_sha256,
        "command_timeout_seconds": args.command_timeout_seconds,
    }
    prepare_no_mutation = getattr(
        args,
        (
            "authorized_no_move_materialize_start_clone_delete_retry_"
            "guest_or_transaction"
        ),
    )
    adopt_no_materialization = (
        args.authorized_no_materialize_start_clone_delete_retry_guest_or_transaction
    )
    materialize_no_other_mutation = (
        args.authorized_no_move_start_clone_delete_retry_rollback_guest_or_transaction
    )
    try:
        if args.command == "move-prepare":
            result = run_move_prepare(
                MovePrepareRequest(
                    **common,
                    **{
                        "authorized_move_prepare": args.authorized_move_prepare,
                        "authorized_one_inventory_query": (
                            args.authorized_one_inventory_query
                        ),
                        "authorized_emit_one_utm_ui_move": (
                            args.authorized_emit_one_utm_ui_move
                        ),
                        (
                            "authorized_no_move_materialize_start_clone_"
                            "delete_retry_guest_or_transaction"
                        ): prepare_no_mutation,
                    },
                )
            )
        elif args.command == "move-adopt":
            if (
                args.move_prepare_root is None
                or args.move_prepare_manifest_sha256 is None
            ):
                raise RecoveryControlError("move-prepare-evidence-required")
            result = run_move_adopt(
                MoveAdoptRequest(
                    **common,
                    **{
                        "move_prepare_root": args.move_prepare_root,
                        "move_prepare_manifest_sha256": (
                            args.move_prepare_manifest_sha256
                        ),
                        "authorized_adopt_one_utm_ui_move_result": (
                            args.authorized_adopt_one_utm_ui_move_result
                        ),
                        "authorized_two_inventory_queries": (
                            args.authorized_two_inventory_queries
                        ),
                        (
                            "authorized_no_materialize_start_clone_delete_"
                            "retry_guest_or_transaction"
                        ): adopt_no_materialization,
                    },
                )
            )
        else:
            if (
                args.move_prepare_root is None
                or args.move_prepare_manifest_sha256 is None
                or args.move_adopt_root is None
                or args.move_adopt_manifest_sha256 is None
            ):
                raise RecoveryControlError("move-evidence-required")
            result = run_materialize(
                MaterializeRequest(
                    **common,
                    **{
                        "move_prepare_root": args.move_prepare_root,
                        "move_prepare_manifest_sha256": (
                            args.move_prepare_manifest_sha256
                        ),
                        "move_adopt_root": args.move_adopt_root,
                        "move_adopt_manifest_sha256": (
                            args.move_adopt_manifest_sha256
                        ),
                        "clonefile_timeout_seconds": (
                            args.clonefile_timeout_seconds
                        ),
                        "authorized_materialize_s2_once": (
                            args.authorized_materialize_s2_once
                        ),
                        "authorized_two_inventory_queries": (
                            args.authorized_two_inventory_queries
                        ),
                        "authorized_two_clonefile_copies": (
                            args.authorized_two_clonefile_copies
                        ),
                        "authorized_two_atomic_replacements": (
                            args.authorized_two_atomic_replacements
                        ),
                        (
                            "authorized_no_move_start_clone_delete_retry_"
                            "rollback_guest_or_transaction"
                        ): materialize_no_other_mutation,
                    },
                )
            )
    except RecoveryControlError as exc:
        print(f"partial_recovery_error={exc}", file=sys.stderr)
        return EXIT_PRECONDITION_REJECTED
    print(f"partial_recovery_outcome={result.outcome}")
    print(f"evidence_root={result.evidence_root}")
    print(f"files_sha256={result.manifest_sha256}")
    return result.exit_code


if __name__ == "__main__":
    raise SystemExit(main())
