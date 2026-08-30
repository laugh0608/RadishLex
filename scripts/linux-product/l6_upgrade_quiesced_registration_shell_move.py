#!/usr/bin/env python3
from __future__ import annotations

import argparse
import re
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Callable, Protocol

import l6_upgrade_quiesced_case_contract as case_contract
import l6_upgrade_quiesced_registration_shell as shell_control
import l6_upgrade_quiesced_registration_shell_bindings as shell_bindings
import l6_upgrade_quiesced_registration_shell_move_bindings as bindings
import l6_utm_clone_once as clone_control


EVIDENCE_FORMAT = bindings.EVIDENCE_FORMAT
EXIT_FROZEN = 0
EXIT_PRECONDITION_REJECTED = 11
EXIT_STATE_INDETERMINATE = 12
HEX_40 = re.compile(r"[0-9a-f]{40}")
HEX_64 = re.compile(r"[0-9a-f]{64}")
SAFE_ATTEMPT_ID = re.compile(r"[a-z0-9][a-z0-9-]{0,95}")


class ShellMoveControlError(ValueError):
    pass


class CommandRunner(Protocol):
    def run(
        self, argv: tuple[str, ...], timeout_seconds: int
    ) -> clone_control.CommandObservation: ...


@dataclass(frozen=True)
class MoveRequestBase:
    repository_root: Path
    expected_repository_head: str
    output_root: Path
    attempt_id: str
    prior_v4_control_root: Path
    prior_v4_manifest_sha256: str
    operator_asset_root: Path
    source_snapshot_root: Path
    default_shell_package_path: Path
    registration_shell_package_path: Path
    registration_shell_evidence_root: Path
    registration_shell_name: str
    registration_shell_uuid: str
    expected_vm_count: int
    expected_inventory_sha256: str
    command_timeout_seconds: int

    def validate_common(self) -> None:
        paths = (
            (self.repository_root, "repository-root"),
            (self.output_root, "output-root"),
            (self.prior_v4_control_root, "prior-v4-control-root"),
            (self.operator_asset_root, "operator-asset-root"),
            (self.source_snapshot_root, "source-snapshot-root"),
            (self.default_shell_package_path, "default-shell-package-path"),
            (
                self.registration_shell_package_path,
                "registration-shell-package-path",
            ),
            (
                self.registration_shell_evidence_root,
                "registration-shell-evidence-root",
            ),
        )
        for path, label in paths:
            if not path.is_absolute() or ".." in path.parts:
                raise ShellMoveControlError(
                    f"{label}-must-be-absolute-normalized"
                )
        outside_repository = (
            (self.output_root, "output-root"),
            (self.prior_v4_control_root, "prior-v4-control-root"),
            (
                self.registration_shell_evidence_root,
                "registration-shell-evidence-root",
            ),
        )
        for path, label in outside_repository:
            if clone_control._path_is_within(path, self.repository_root):
                raise ShellMoveControlError(
                    f"{label}-must-be-outside-repository"
                )
        protected = (
            (self.prior_v4_control_root, "prior-v4-control-root"),
            (self.source_snapshot_root, "source-snapshot-root"),
            (self.default_shell_package_path, "default-shell-package"),
            (
                self.registration_shell_package_path,
                "registration-shell-package",
            ),
        )
        evidence_roots = (
            (self.output_root, "output-root"),
            (
                self.registration_shell_evidence_root,
                "registration-shell-evidence-root",
            ),
        )
        for evidence, evidence_label in evidence_roots:
            for protected_path, protected_label in protected:
                if clone_control._paths_overlap(evidence, protected_path):
                    raise ShellMoveControlError(
                        f"{evidence_label}-overlaps-{protected_label}"
                    )
        if clone_control._paths_overlap(
            self.output_root, self.registration_shell_evidence_root
        ):
            raise ShellMoveControlError("evidence-roots-overlap")
        expected_source = self.operator_asset_root / str(
            case_contract.EXPECTED_START["relative_path"]
        )
        expected_default = bindings.DEFAULT_UTM_STORAGE_ROOT / (
            f"{shell_bindings.REQUIRED_SHELL_NAME}.utm"
        )
        expected_external = self.operator_asset_root / (
            f"{shell_bindings.REQUIRED_SHELL_NAME}.utm"
        )
        if self.source_snapshot_root != expected_source:
            raise ShellMoveControlError("source-snapshot-path-drift")
        if self.default_shell_package_path != expected_default:
            raise ShellMoveControlError("default-shell-path-drift")
        if self.registration_shell_package_path != expected_external:
            raise ShellMoveControlError("external-shell-path-drift")
        if self.default_shell_package_path == (
            self.registration_shell_package_path
        ):
            raise ShellMoveControlError("default-and-external-shell-overlap")
        if self.registration_shell_name != shell_bindings.REQUIRED_SHELL_NAME:
            raise ShellMoveControlError("registration-shell-name-drift")
        if self.registration_shell_uuid != bindings.REQUIRED_SHELL_UUID:
            raise ShellMoveControlError("registration-shell-uuid-drift")
        if (
            self.prior_v4_manifest_sha256
            != bindings.REQUIRED_V4_CONTROL_MANIFEST_SHA256
        ):
            raise ShellMoveControlError("prior-v4-manifest-drift")
        if not HEX_40.fullmatch(self.expected_repository_head):
            raise ShellMoveControlError("expected-repository-head-invalid")
        if not HEX_64.fullmatch(self.expected_inventory_sha256):
            raise ShellMoveControlError("expected-inventory-sha256-invalid")
        if not SAFE_ATTEMPT_ID.fullmatch(self.attempt_id):
            raise ShellMoveControlError("attempt-id-invalid")
        if self.expected_vm_count != bindings.REQUIRED_INVENTORY_COUNT:
            raise ShellMoveControlError("expected-vm-count-drift")
        if (
            self.expected_inventory_sha256
            != bindings.REQUIRED_INVENTORY_SHA256
        ):
            raise ShellMoveControlError("expected-inventory-drift")
        if not 1 <= self.command_timeout_seconds <= 60:
            raise ShellMoveControlError(
                "command-timeout-seconds-out-of-range"
            )

    def common_json(self, phase: str) -> dict[str, object]:
        return {
            "attempt_id": self.attempt_id,
            "command_timeout_seconds": self.command_timeout_seconds,
            "default_shell_package_path_sha256": shell_bindings.sha256_text(
                str(self.default_shell_package_path)
            ),
            "expected_inventory_sha256": self.expected_inventory_sha256,
            "expected_repository_head": self.expected_repository_head,
            "expected_vm_count": self.expected_vm_count,
            "format": EVIDENCE_FORMAT,
            "operator_asset_root_sha256": shell_bindings.sha256_text(
                str(self.operator_asset_root)
            ),
            "phase": phase,
            "prior_v4_manifest_sha256": self.prior_v4_manifest_sha256,
            "registration_shell_evidence_root_sha256": (
                shell_bindings.sha256_text(
                    str(self.registration_shell_evidence_root)
                )
            ),
            "registration_shell_name": self.registration_shell_name,
            "registration_shell_package_path_sha256": (
                shell_bindings.sha256_text(
                    str(self.registration_shell_package_path)
                )
            ),
            "registration_shell_uuid": self.registration_shell_uuid,
            "source_snapshot_identity": str(
                case_contract.EXPECTED_START["identity"]
            ),
        }


@dataclass(frozen=True)
class MovePrepareRequest(MoveRequestBase):
    authorized_upgrade_quiesced_registration_shell_move_prepare: bool
    authorized_one_pre_move_inventory_query: bool
    authorized_next_external_action_one_utm_ui_move: bool
    authorized_no_move_update_start_clone_delete_retry_guest_or_transaction: (
        bool
    )

    def validate(self) -> None:
        self.validate_common()
        required = (
            self.authorized_upgrade_quiesced_registration_shell_move_prepare,
            self.authorized_one_pre_move_inventory_query,
            self.authorized_next_external_action_one_utm_ui_move,
            (
                self.authorized_no_move_update_start_clone_delete_retry_guest_or_transaction
            ),
        )
        if not all(required):
            raise ShellMoveControlError("prepare-authorization-required")

    def as_json(self) -> dict[str, object]:
        return {
            **self.common_json("prepare"),
            "authorization": {
                "next_external_action_one_utm_ui_move": True,
                "no_move_update_start_clone_delete_retry_guest_or_transaction": True,
                "one_pre_move_inventory_query": True,
                "upgrade_quiesced_registration_shell_move_prepare": True,
            },
        }


@dataclass(frozen=True)
class MoveCompleteRequest(MoveRequestBase):
    prepare_root: Path
    prepare_manifest_sha256: str
    update_timeout_seconds: int
    authorized_adopt_one_utm_ui_move_result: bool
    authorized_one_stopped_configuration_update: bool
    authorized_two_inventory_queries: bool
    authorized_no_start_clone_delete_retry_guest_or_transaction: bool

    def validate(self) -> None:
        self.validate_common()
        if (
            not self.prepare_root.is_absolute()
            or ".." in self.prepare_root.parts
        ):
            raise ShellMoveControlError(
                "prepare-root-must-be-absolute-normalized"
            )
        if clone_control._path_is_within(
            self.prepare_root, self.repository_root
        ):
            raise ShellMoveControlError(
                "prepare-root-must-be-outside-repository"
            )
        for path, label in (
            (self.output_root, "output-root"),
            (
                self.registration_shell_evidence_root,
                "registration-shell-evidence-root",
            ),
            (self.source_snapshot_root, "source-snapshot-root"),
            (self.default_shell_package_path, "default-shell-package"),
            (
                self.registration_shell_package_path,
                "registration-shell-package",
            ),
        ):
            if clone_control._paths_overlap(self.prepare_root, path):
                raise ShellMoveControlError(f"prepare-root-overlaps-{label}")
        if not HEX_64.fullmatch(self.prepare_manifest_sha256):
            raise ShellMoveControlError("prepare-manifest-sha256-invalid")
        if not 1 <= self.update_timeout_seconds <= 300:
            raise ShellMoveControlError(
                "update-timeout-seconds-out-of-range"
            )
        required = (
            self.authorized_adopt_one_utm_ui_move_result,
            self.authorized_one_stopped_configuration_update,
            self.authorized_two_inventory_queries,
            self.authorized_no_start_clone_delete_retry_guest_or_transaction,
        )
        if not all(required):
            raise ShellMoveControlError("complete-authorization-required")

    def as_json(self) -> dict[str, object]:
        return {
            **self.common_json("complete"),
            "authorization": {
                "adopt_one_utm_ui_move_result": True,
                "no_start_clone_delete_retry_guest_or_transaction": True,
                "one_stopped_configuration_update": True,
                "two_inventory_queries": True,
            },
            "prepare_manifest_sha256": self.prepare_manifest_sha256,
            "prepare_root_sha256": shell_bindings.sha256_text(
                str(self.prepare_root)
            ),
            "update_timeout_seconds": self.update_timeout_seconds,
        }


@dataclass(frozen=True)
class ShellMoveResult:
    outcome: str
    exit_code: int
    evidence_root: Path
    manifest_sha256: str
    inventory_query_invocations: int
    update_invocations: int
    registration_shell_manifest_sha256: str | None


PrepareBindingValidator = Callable[
    [MovePrepareRequest], dict[str, object]
]
CompleteBindingValidator = Callable[
    [MoveCompleteRequest], dict[str, object]
]


def run_prepare_once(
    request: MovePrepareRequest,
    *,
    runner: CommandRunner | None = None,
    binding_validator: PrepareBindingValidator | None = None,
) -> ShellMoveResult:
    request.validate()
    writer = clone_control.EvidenceWriter.create(request.output_root)
    writer.write_json("request.json", request.as_json())
    command_runner = runner or clone_control.SubprocessCommandRunner()
    validate_bindings = binding_validator or bindings.validate_prepare_bindings

    stage = "binding-preflight"
    reason = "not-run"
    inventory_queries = 0
    outcome = "precondition-rejected"
    exit_code = EXIT_PRECONDITION_REJECTED
    try:
        writer.write_json(
            "binding-preflight.json", validate_bindings(request)
        )

        stage = "source-snapshot-preflight"
        source = shell_bindings.validate_source_snapshot(request)
        writer.write_json("source-snapshot-preflight.json", source.as_json())

        stage = "default-shell-preflight"
        default_shell = bindings.validate_default_partial_shell(request)
        writer.write_json(
            "default-shell-preflight.json", default_shell.as_json()
        )

        stage = "external-shell-preflight"
        external = clone_control.observe_target_package(
            request.registration_shell_package_path
        )
        writer.write_json("external-shell-preflight.json", external)
        if external["state"] != "absent":
            raise ShellMoveControlError("external-shell-must-be-absent")

        stage = "shell-evidence-preflight"
        shell_evidence = clone_control.observe_target_package(
            request.registration_shell_evidence_root
        )
        writer.write_json("shell-evidence-preflight.json", shell_evidence)
        if shell_evidence["state"] != "absent":
            raise ShellMoveControlError("shell-evidence-must-be-absent")

        stage = "source-default-target-device"
        _require_same_device(
            source,
            default_shell,
            request.registration_shell_package_path.parent,
        )
        writer.write_json(
            "source-default-target-device.json",
            {"format": EVIDENCE_FORMAT, "same_device": True},
        )

        stage = "source-and-default-handles-preflight"
        handles_argv = shell_control._lsof_argv(source, default_shell)
        handles = command_runner.run(
            handles_argv, request.command_timeout_seconds
        )
        writer.write_json(
            "source-and-default-handles-preflight.json", handles.as_json()
        )
        shell_control._require_zero_handles(handles, handles_argv)

        stage = "utmctl-list-pre-move"
        inventory_queries = 1
        inventory_observation = command_runner.run(
            ("utmctl", "list"), request.command_timeout_seconds
        )
        writer.write_json(
            "utmctl-list-pre-move.json", inventory_observation.as_json()
        )
        inventory = clone_control.parse_utmctl_list(inventory_observation)
        _validate_inventory(inventory, request)
        writer.write_json(
            "inventory-pre-move.json",
            shell_control._inventory_evidence(inventory),
        )

        stage = "source-and-default-handles-post-list"
        terminal_handles = command_runner.run(
            handles_argv, request.command_timeout_seconds
        )
        writer.write_json(
            "source-and-default-handles-post-list.json",
            terminal_handles.as_json(),
        )
        shell_control._require_zero_handles(terminal_handles, handles_argv)

        stage = "source-snapshot-terminal"
        source_terminal = shell_bindings.validate_source_snapshot(request)
        writer.write_json(
            "source-snapshot-terminal.json", source_terminal.as_json()
        )
        if source_terminal != source:
            raise ShellMoveControlError("source-snapshot-drift")

        stage = "default-shell-terminal"
        default_terminal = bindings.validate_default_partial_shell(request)
        writer.write_json(
            "default-shell-terminal.json", default_terminal.as_json()
        )
        if default_terminal != default_shell:
            raise ShellMoveControlError("default-shell-drift")

        stage = "external-shell-terminal"
        external_terminal = clone_control.observe_target_package(
            request.registration_shell_package_path
        )
        writer.write_json("external-shell-terminal.json", external_terminal)
        if external_terminal["state"] != "absent":
            raise ShellMoveControlError("external-shell-state-drift")

        outcome = "move-ready"
        exit_code = EXIT_FROZEN
        reason = "one-utm-ui-move-may-be-authorized-separately"
    except (
        ShellMoveControlError,
        bindings.ShellMoveBindingError,
        shell_bindings.RegistrationShellBindingError,
        shell_control.RegistrationShellError,
        clone_control.CloneControlError,
        OSError,
    ) as exc:
        reason = f"{stage}:{exc}"

    writer.write_json(
        "terminal.json",
        _terminal(
            request,
            outcome=outcome,
            reason=reason,
            inventory_queries=inventory_queries,
            update_invocations=0,
            update_observation=None,
            shell_manifest=None,
            next_external_action=(
                "one-utm-ui-move-to-authorized-package-path"
                if outcome == "move-ready"
                else "none"
            ),
        ),
    )
    manifest = writer.write_manifest()
    return ShellMoveResult(
        outcome=outcome,
        exit_code=exit_code,
        evidence_root=request.output_root,
        manifest_sha256=manifest,
        inventory_query_invocations=inventory_queries,
        update_invocations=0,
        registration_shell_manifest_sha256=None,
    )


def run_complete_once(
    request: MoveCompleteRequest,
    *,
    runner: CommandRunner | None = None,
    binding_validator: CompleteBindingValidator | None = None,
) -> ShellMoveResult:
    request.validate()
    writer = clone_control.EvidenceWriter.create(request.output_root)
    writer.write_json("request.json", request.as_json())
    command_runner = runner or clone_control.SubprocessCommandRunner()
    validate_bindings = binding_validator or bindings.validate_complete_bindings

    stage = "binding-preflight"
    reason = "not-run"
    inventory_queries = 0
    update_invocations = 0
    update_observation: clone_control.CommandObservation | None = None
    shell_manifest: str | None = None
    outcome = "state-indeterminate"
    exit_code = EXIT_STATE_INDETERMINATE
    try:
        writer.write_json(
            "binding-preflight.json", validate_bindings(request)
        )

        stage = "source-snapshot-preflight"
        source = shell_bindings.validate_source_snapshot(request)
        writer.write_json("source-snapshot-preflight.json", source.as_json())

        stage = "default-shell-preupdate"
        default = clone_control.observe_target_package(
            request.default_shell_package_path
        )
        writer.write_json("default-shell-preupdate.json", default)
        if default["state"] != "absent":
            raise ShellMoveControlError(
                "default-shell-must-be-absent-after-move"
            )

        stage = "external-shell-preupdate"
        partial = bindings.validate_external_partial_shell(request)
        writer.write_json("external-shell-preupdate.json", partial.as_json())

        stage = "shell-evidence-preupdate"
        evidence = clone_control.observe_target_package(
            request.registration_shell_evidence_root
        )
        writer.write_json("shell-evidence-preupdate.json", evidence)
        if evidence["state"] != "absent":
            raise ShellMoveControlError("shell-evidence-must-be-absent")

        stage = "source-and-shell-parent-device"
        _require_same_device(
            source,
            partial,
            request.registration_shell_package_path.parent,
        )
        writer.write_json(
            "source-and-shell-parent-device.json",
            {"format": EVIDENCE_FORMAT, "same_device": True},
        )

        stage = "source-and-shell-handles-preupdate"
        handles_argv = shell_control._lsof_argv(source, partial)
        handles = command_runner.run(
            handles_argv, request.command_timeout_seconds
        )
        writer.write_json(
            "source-and-shell-handles-preupdate.json", handles.as_json()
        )
        shell_control._require_zero_handles(handles, handles_argv)

        stage = "utmctl-list-preupdate"
        inventory_queries = 1
        preupdate_observation = command_runner.run(
            ("utmctl", "list"), request.command_timeout_seconds
        )
        writer.write_json(
            "utmctl-list-preupdate.json", preupdate_observation.as_json()
        )
        preupdate = clone_control.parse_utmctl_list(preupdate_observation)
        _validate_inventory(preupdate, request)
        writer.write_json(
            "inventory-preupdate.json",
            shell_control._inventory_evidence(preupdate),
        )

        stage = "update-registration-shell-configuration"
        update_invocations = 1
        update_observation = command_runner.run(
            shell_control._update_argv(
                request, request.registration_shell_uuid
            ),
            request.update_timeout_seconds,
        )
        writer.write_json(
            "update-registration-shell-configuration.json",
            update_observation.as_json(),
        )

        stage = "utmctl-list-postupdate"
        inventory_queries = 2
        postupdate_observation = command_runner.run(
            ("utmctl", "list"), request.command_timeout_seconds
        )
        writer.write_json(
            "utmctl-list-postupdate.json", postupdate_observation.as_json()
        )
        postupdate = clone_control.parse_utmctl_list(postupdate_observation)
        _validate_inventory(postupdate, request)
        writer.write_json(
            "inventory-postupdate.json",
            shell_control._inventory_evidence(postupdate),
        )
        if shell_control._parse_updated_uuid(update_observation) != (
            request.registration_shell_uuid
        ):
            raise ShellMoveControlError("update-command-uuid-drift")

        stage = "external-shell-postupdate"
        shell = bindings.validate_final_shell(request)
        writer.write_json("external-shell-postupdate.json", shell.as_json())
        _require_same_disk_identity(partial, shell)

        stage = "source-and-shell-handles-terminal"
        terminal_handles_argv = shell_control._lsof_argv(source, shell)
        terminal_handles = command_runner.run(
            terminal_handles_argv, request.command_timeout_seconds
        )
        writer.write_json(
            "source-and-shell-handles-terminal.json",
            terminal_handles.as_json(),
        )
        shell_control._require_zero_handles(
            terminal_handles, terminal_handles_argv
        )

        stage = "source-snapshot-terminal"
        source_terminal = shell_bindings.validate_source_snapshot(request)
        writer.write_json(
            "source-snapshot-terminal.json", source_terminal.as_json()
        )
        if source_terminal != source:
            raise ShellMoveControlError("source-snapshot-drift")

        stage = "default-shell-terminal"
        default_terminal = clone_control.observe_target_package(
            request.default_shell_package_path
        )
        writer.write_json("default-shell-terminal.json", default_terminal)
        if default_terminal["state"] != "absent":
            raise ShellMoveControlError("default-shell-state-drift")

        stage = "external-shell-terminal"
        shell_terminal = bindings.validate_final_shell(request)
        writer.write_json(
            "external-shell-terminal.json", shell_terminal.as_json()
        )
        if shell_terminal != shell:
            raise ShellMoveControlError("external-shell-terminal-drift")

        stage = "registration-shell-evidence-freeze"
        shell_writer = clone_control.EvidenceWriter.create(
            request.registration_shell_evidence_root
        )
        shell_writer.write_json(
            "registration-shell.json",
            shell_bindings.expected_shell_evidence(request, shell),
        )
        shell_manifest = shell_writer.write_manifest()
        writer.write_json(
            "registration-shell-evidence-freeze.json",
            {
                "format": EVIDENCE_FORMAT,
                "manifest_sha256": shell_manifest,
                "state": "frozen",
            },
        )

        outcome = "frozen"
        exit_code = EXIT_FROZEN
        reason = "moved-registration-shell-updated-networkless-and-frozen"
    except (
        ShellMoveControlError,
        bindings.ShellMoveBindingError,
        shell_bindings.RegistrationShellBindingError,
        shell_control.RegistrationShellError,
        clone_control.CloneControlError,
        OSError,
    ) as exc:
        reason = f"{stage}:{exc}"

    writer.write_json(
        "terminal.json",
        _terminal(
            request,
            outcome=outcome,
            reason=reason,
            inventory_queries=inventory_queries,
            update_invocations=update_invocations,
            update_observation=update_observation,
            shell_manifest=shell_manifest,
            next_external_action="none",
        ),
    )
    manifest = writer.write_manifest()
    return ShellMoveResult(
        outcome=outcome,
        exit_code=exit_code,
        evidence_root=request.output_root,
        manifest_sha256=manifest,
        inventory_query_invocations=inventory_queries,
        update_invocations=update_invocations,
        registration_shell_manifest_sha256=shell_manifest,
    )


def _validate_inventory(
    inventory: tuple[clone_control.RegisteredVm, ...],
    request: MoveRequestBase,
) -> None:
    if len(inventory) != request.expected_vm_count:
        raise ShellMoveControlError("inventory-count-drift")
    if any(item.status != "stopped" for item in inventory):
        raise ShellMoveControlError("inventory-not-all-stopped")
    if clone_control.canonical_inventory_sha256(inventory) != (
        request.expected_inventory_sha256
    ):
        raise ShellMoveControlError("inventory-sha256-drift")
    shell = [
        item
        for item in inventory
        if item.uuid == request.registration_shell_uuid
        or item.name == request.registration_shell_name
    ]
    if shell != [
        clone_control.RegisteredVm(
            request.registration_shell_uuid,
            "stopped",
            request.registration_shell_name,
        )
    ]:
        raise ShellMoveControlError("registration-shell-inventory-drift")


def _require_same_device(
    source: shell_bindings.BundleIdentity,
    shell: shell_bindings.BundleIdentity,
    target_parent: Path,
) -> None:
    devices = {
        source.efi_path.stat().st_dev,
        shell.efi_path.stat().st_dev,
        target_parent.stat().st_dev,
    }
    if len(devices) != 1:
        raise ShellMoveControlError("source-shell-target-not-on-same-device")


def _require_same_disk_identity(
    before: shell_bindings.BundleIdentity,
    after: shell_bindings.BundleIdentity,
) -> None:
    if (
        before.efi_sha256 != after.efi_sha256
        or before.qcow2_name != after.qcow2_name
        or before.qcow2_sha256 != after.qcow2_sha256
        or before.name != after.name
        or before.uuid != after.uuid
    ):
        raise ShellMoveControlError("registration-shell-disk-identity-drift")


def _terminal(
    request: MoveRequestBase,
    *,
    outcome: str,
    reason: str,
    inventory_queries: int,
    update_invocations: int,
    update_observation: clone_control.CommandObservation | None,
    shell_manifest: str | None,
    next_external_action: str,
) -> dict[str, object]:
    return {
        "automatic_delete": "not-performed",
        "automatic_retry": "not-performed",
        "automatic_rollback": "not-performed",
        "automatic_start": "not-performed",
        "clone_invocations": 0,
        "create_invocations": 0,
        "file_pull_invocations": 0,
        "file_push_invocations": 0,
        "format": EVIDENCE_FORMAT,
        "guest_exec_invocations": 0,
        "input_transfer": "not-performed",
        "inventory_query_invocations": inventory_queries,
        "next_external_action": next_external_action,
        "operation_id": "not-generated",
        "outcome": outcome,
        "reason": reason,
        "registration_shell_manifest_sha256": shell_manifest,
        "registration_shell_name": request.registration_shell_name,
        "registration_shell_uuid": request.registration_shell_uuid,
        "transaction": "not-performed",
        "ui_move_invocations": 0,
        "update_command_exit_code": (
            update_observation.exit_code if update_observation else None
        ),
        "update_command_timed_out": (
            update_observation.timed_out if update_observation else False
        ),
        "update_invocations": update_invocations,
    }


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Prepare or complete the native-UTM-move recovery for the "
            "upgrade_quiesced registration shell."
        )
    )
    parser.add_argument(
        "command", choices=("prepare-move-once", "complete-move-once")
    )
    parser.add_argument("--repository-root", type=Path, required=True)
    parser.add_argument("--expected-repository-head", required=True)
    parser.add_argument("--output-root", type=Path, required=True)
    parser.add_argument("--attempt-id", required=True)
    parser.add_argument("--prior-v4-control-root", type=Path, required=True)
    parser.add_argument("--prior-v4-manifest-sha256", required=True)
    parser.add_argument("--operator-asset-root", type=Path, required=True)
    parser.add_argument("--source-snapshot-root", type=Path, required=True)
    parser.add_argument(
        "--default-shell-package-path", type=Path, required=True
    )
    parser.add_argument(
        "--registration-shell-package-path", type=Path, required=True
    )
    parser.add_argument(
        "--registration-shell-evidence-root", type=Path, required=True
    )
    parser.add_argument("--registration-shell-name", required=True)
    parser.add_argument("--registration-shell-uuid", required=True)
    parser.add_argument("--expected-vm-count", type=int, required=True)
    parser.add_argument("--expected-inventory-sha256", required=True)
    parser.add_argument(
        "--command-timeout-seconds", type=int, default=15
    )
    parser.add_argument("--prepare-root", type=Path)
    parser.add_argument("--prepare-manifest-sha256")
    parser.add_argument("--update-timeout-seconds", type=int, default=120)
    parser.add_argument(
        "--authorized-upgrade-quiesced-registration-shell-move-prepare",
        action="store_true",
    )
    parser.add_argument(
        "--authorized-one-pre-move-inventory-query", action="store_true"
    )
    parser.add_argument(
        "--authorized-next-external-action-one-utm-ui-move",
        action="store_true",
    )
    parser.add_argument(
        "--authorized-no-move-update-start-clone-delete-retry-guest-or-transaction",
        action="store_true",
    )
    parser.add_argument(
        "--authorized-adopt-one-utm-ui-move-result", action="store_true"
    )
    parser.add_argument(
        "--authorized-one-stopped-configuration-update", action="store_true"
    )
    parser.add_argument(
        "--authorized-two-inventory-queries", action="store_true"
    )
    parser.add_argument(
        "--authorized-no-start-clone-delete-retry-guest-or-transaction",
        action="store_true",
    )
    return parser.parse_args()


def _common_kwargs(args: argparse.Namespace) -> dict[str, object]:
    return {
        "repository_root": args.repository_root,
        "expected_repository_head": args.expected_repository_head,
        "output_root": args.output_root,
        "attempt_id": args.attempt_id,
        "prior_v4_control_root": args.prior_v4_control_root,
        "prior_v4_manifest_sha256": args.prior_v4_manifest_sha256,
        "operator_asset_root": args.operator_asset_root,
        "source_snapshot_root": args.source_snapshot_root,
        "default_shell_package_path": args.default_shell_package_path,
        "registration_shell_package_path": (
            args.registration_shell_package_path
        ),
        "registration_shell_evidence_root": (
            args.registration_shell_evidence_root
        ),
        "registration_shell_name": args.registration_shell_name,
        "registration_shell_uuid": args.registration_shell_uuid,
        "expected_vm_count": args.expected_vm_count,
        "expected_inventory_sha256": args.expected_inventory_sha256,
        "command_timeout_seconds": args.command_timeout_seconds,
    }


def main() -> int:
    args = parse_args()
    common = _common_kwargs(args)
    try:
        if args.command == "prepare-move-once":
            request = MovePrepareRequest(
                **common,
                authorized_upgrade_quiesced_registration_shell_move_prepare=(
                    args.authorized_upgrade_quiesced_registration_shell_move_prepare
                ),
                authorized_one_pre_move_inventory_query=(
                    args.authorized_one_pre_move_inventory_query
                ),
                authorized_next_external_action_one_utm_ui_move=(
                    args.authorized_next_external_action_one_utm_ui_move
                ),
                authorized_no_move_update_start_clone_delete_retry_guest_or_transaction=(
                    args.authorized_no_move_update_start_clone_delete_retry_guest_or_transaction
                ),
            )
            result = run_prepare_once(request)
        else:
            if (
                args.prepare_root is None
                or args.prepare_manifest_sha256 is None
            ):
                raise ShellMoveControlError("complete-prepare-binding-required")
            request = MoveCompleteRequest(
                **common,
                prepare_root=args.prepare_root,
                prepare_manifest_sha256=args.prepare_manifest_sha256,
                update_timeout_seconds=args.update_timeout_seconds,
                authorized_adopt_one_utm_ui_move_result=(
                    args.authorized_adopt_one_utm_ui_move_result
                ),
                authorized_one_stopped_configuration_update=(
                    args.authorized_one_stopped_configuration_update
                ),
                authorized_two_inventory_queries=(
                    args.authorized_two_inventory_queries
                ),
                authorized_no_start_clone_delete_retry_guest_or_transaction=(
                    args.authorized_no_start_clone_delete_retry_guest_or_transaction
                ),
            )
            result = run_complete_once(request)
    except ShellMoveControlError as exc:
        print(f"registration_shell_move_error={exc}", file=sys.stderr)
        return EXIT_PRECONDITION_REJECTED
    print(f"registration_shell_move_outcome={result.outcome}")
    print(f"evidence_root={result.evidence_root}")
    print(f"files_sha256={result.manifest_sha256}")
    if result.registration_shell_manifest_sha256 is not None:
        print(
            "registration_shell_files_sha256="
            f"{result.registration_shell_manifest_sha256}"
        )
    return result.exit_code


if __name__ == "__main__":
    raise SystemExit(main())
