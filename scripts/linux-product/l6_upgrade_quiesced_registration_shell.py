#!/usr/bin/env python3
from __future__ import annotations

import argparse
import re
import sys
import uuid
from dataclasses import dataclass
from pathlib import Path
from typing import Callable, Protocol

import l6_upgrade_quiesced_case_contract as case_contract
import l6_upgrade_quiesced_registration_shell_bindings as bindings
import l6_utm_clone_once as clone_control


EVIDENCE_FORMAT = bindings.CONTROL_FORMAT
EXIT_FROZEN = 0
EXIT_FAILED_CLOSED_ABSENT = 10
EXIT_PRECONDITION_REJECTED = 11
EXIT_STATE_INDETERMINATE = 12
OSASCRIPT_PATH = "/usr/bin/osascript"
LSOF_PATH = "/usr/sbin/lsof"
HEX_40 = re.compile(r"[0-9a-f]{40}")
HEX_64 = re.compile(r"[0-9a-f]{64}")
SAFE_ATTEMPT_ID = re.compile(r"[a-z0-9][a-z0-9-]{0,95}")


class RegistrationShellError(ValueError):
    pass


class _TerminalOutcome(Exception):
    pass


@dataclass(frozen=True)
class RegistrationShellRequest:
    repository_root: Path
    expected_repository_head: str
    output_root: Path
    attempt_id: str
    operator_asset_root: Path
    source_snapshot_root: Path
    registration_shell_evidence_root: Path
    registration_shell_name: str
    registration_shell_package_path: Path
    expected_vm_count: int
    expected_precreate_inventory_sha256: str
    create_timeout_seconds: int
    update_timeout_seconds: int
    command_timeout_seconds: int
    authorized_upgrade_quiesced_registration_shell: bool
    authorized_one_create_new_registration_only_shell: bool
    authorized_one_stopped_configuration_update: bool
    authorized_no_clone_start_guest_delete_retry_or_transaction: bool

    def validate(self) -> None:
        paths = (
            (self.repository_root, "repository-root"),
            (self.output_root, "output-root"),
            (self.operator_asset_root, "operator-asset-root"),
            (self.source_snapshot_root, "source-snapshot-root"),
            (
                self.registration_shell_evidence_root,
                "registration-shell-evidence-root",
            ),
            (
                self.registration_shell_package_path,
                "registration-shell-package-path",
            ),
        )
        for path, label in paths:
            if not path.is_absolute() or ".." in path.parts:
                raise RegistrationShellError(
                    f"{label}-must-be-absolute-normalized"
                )
        if clone_control._path_is_within(
            self.output_root, self.repository_root
        ) or clone_control._path_is_within(
            self.registration_shell_evidence_root,
            self.repository_root,
        ):
            raise RegistrationShellError(
                "evidence-roots-must-be-outside-repository"
            )
        protected = (
            (self.source_snapshot_root, "source-snapshot"),
            (
                self.registration_shell_package_path,
                "registration-shell-package",
            ),
        )
        for evidence_root, evidence_label in (
            (self.output_root, "output-root"),
            (
                self.registration_shell_evidence_root,
                "registration-shell-evidence-root",
            ),
        ):
            for protected_path, protected_label in protected:
                if clone_control._paths_overlap(
                    evidence_root, protected_path
                ):
                    raise RegistrationShellError(
                        f"{evidence_label}-overlaps-{protected_label}"
                    )
        if clone_control._paths_overlap(
            self.output_root, self.registration_shell_evidence_root
        ):
            raise RegistrationShellError("evidence-roots-overlap")
        expected_snapshot = self.operator_asset_root / str(
            case_contract.EXPECTED_START["relative_path"]
        )
        if self.source_snapshot_root != expected_snapshot:
            raise RegistrationShellError(
                "source-snapshot-path-differs-from-case"
            )
        if (
            self.registration_shell_name != bindings.REQUIRED_SHELL_NAME
        ):
            raise RegistrationShellError(
                "registration-shell-name-differs-from-contract"
            )
        if self.registration_shell_package_path != (
            self.operator_asset_root
            / f"{self.registration_shell_name}.utm"
        ):
            raise RegistrationShellError(
                "registration-shell-package-path-differs-from-contract"
            )
        if not HEX_40.fullmatch(self.expected_repository_head):
            raise RegistrationShellError("expected-repository-head-invalid")
        if not HEX_64.fullmatch(
            self.expected_precreate_inventory_sha256
        ):
            raise RegistrationShellError(
                "expected-precreate-inventory-sha256-invalid"
            )
        if not SAFE_ATTEMPT_ID.fullmatch(self.attempt_id):
            raise RegistrationShellError("attempt-id-invalid")
        if not 1 <= self.expected_vm_count <= 128:
            raise RegistrationShellError("expected-vm-count-out-of-range")
        if not 1 <= self.create_timeout_seconds <= 300:
            raise RegistrationShellError(
                "create-timeout-seconds-out-of-range"
            )
        if not 1 <= self.update_timeout_seconds <= 300:
            raise RegistrationShellError(
                "update-timeout-seconds-out-of-range"
            )
        if not 1 <= self.command_timeout_seconds <= 60:
            raise RegistrationShellError(
                "command-timeout-seconds-out-of-range"
            )
        authorizations = (
            (
                self.authorized_upgrade_quiesced_registration_shell,
                "upgrade-quiesced-registration-shell-authorization-required",
            ),
            (
                self.authorized_one_create_new_registration_only_shell,
                "one-create-new-registration-only-shell-authorization-required",
            ),
            (
                self.authorized_one_stopped_configuration_update,
                "one-stopped-configuration-update-authorization-required",
            ),
            (
                self.authorized_no_clone_start_guest_delete_retry_or_transaction,
                "no-clone-start-guest-delete-retry-or-transaction-authorization-required",
            ),
        )
        for authorized, message in authorizations:
            if not authorized:
                raise RegistrationShellError(message)

    def as_json(self) -> dict[str, object]:
        return {
            "attempt_id": self.attempt_id,
            "authorization": {
                "no_clone_start_guest_delete_retry_or_transaction": True,
                "one_create_new_registration_only_shell": True,
                "one_stopped_configuration_update": True,
                "upgrade_quiesced_registration_shell": True,
            },
            "command_timeout_seconds": self.command_timeout_seconds,
            "create_timeout_seconds": self.create_timeout_seconds,
            "update_timeout_seconds": self.update_timeout_seconds,
            "expected_precreate_inventory_sha256": (
                self.expected_precreate_inventory_sha256
            ),
            "expected_repository_head": self.expected_repository_head,
            "expected_vm_count": self.expected_vm_count,
            "format": EVIDENCE_FORMAT,
            "operator_asset_root_sha256": bindings.sha256_text(
                str(self.operator_asset_root)
            ),
            "registration_shell_evidence_root_sha256": (
                bindings.sha256_text(
                    str(self.registration_shell_evidence_root)
                )
            ),
            "registration_shell_name": self.registration_shell_name,
            "registration_shell_package_path_sha256": (
                bindings.sha256_text(
                    str(self.registration_shell_package_path)
                )
            ),
            "source_snapshot_identity": str(
                case_contract.EXPECTED_START["identity"]
            ),
        }


@dataclass(frozen=True)
class RegistrationShellResult:
    outcome: str
    exit_code: int
    evidence_root: Path
    manifest_sha256: str
    create_invocations: int
    update_invocations: int
    registration_shell_uuid: str | None
    registration_shell_manifest_sha256: str | None


class CommandRunner(Protocol):
    def run(
        self, argv: tuple[str, ...], timeout_seconds: int
    ) -> clone_control.CommandObservation: ...


BindingValidator = Callable[
    [RegistrationShellRequest], dict[str, object]
]


def run_registration_shell_once(
    request: RegistrationShellRequest,
    *,
    runner: CommandRunner | None = None,
    binding_validator: BindingValidator | None = None,
) -> RegistrationShellResult:
    request.validate()
    writer = clone_control.EvidenceWriter.create(request.output_root)
    writer.write_json("request.json", request.as_json())
    command_runner = runner or clone_control.SubprocessCommandRunner()
    validate_bindings = (
        binding_validator or bindings.validate_registration_shell_bindings
    )

    stage = "binding-preflight"
    outcome = "precondition-rejected"
    exit_code = EXIT_PRECONDITION_REJECTED
    reason = "not-run"
    create_invocations = 0
    update_invocations = 0
    create_observation: clone_control.CommandObservation | None = None
    update_observation: clone_control.CommandObservation | None = None
    shell_uuid: str | None = None
    shell_manifest_sha256: str | None = None
    shell_package_state = "not-observed"

    try:
        writer.write_json(
            "binding-preflight.json", validate_bindings(request)
        )

        stage = "source-snapshot-preflight"
        source = bindings.validate_source_snapshot(request)
        writer.write_json(
            "source-snapshot-preflight.json", source.as_json()
        )

        stage = "registration-shell-package-precreate"
        shell_precreate = clone_control.observe_target_package(
            request.registration_shell_package_path
        )
        writer.write_json(
            "registration-shell-package-precreate.json", shell_precreate
        )
        if shell_precreate["state"] != "absent":
            raise RegistrationShellError(
                "registration-shell-package-not-absent"
            )

        stage = "registration-shell-evidence-precreate"
        shell_evidence_precreate = clone_control.observe_target_package(
            request.registration_shell_evidence_root
        )
        writer.write_json(
            "registration-shell-evidence-precreate.json",
            shell_evidence_precreate,
        )
        if shell_evidence_precreate["state"] != "absent":
            raise RegistrationShellError(
                "registration-shell-evidence-root-not-absent"
            )

        stage = "source-and-shell-parent-device"
        if source.efi_path.stat().st_dev != (
            request.registration_shell_package_path.parent.stat().st_dev
        ):
            raise RegistrationShellError(
                "source-and-registration-shell-parent-not-on-same-device"
            )
        writer.write_json(
            "source-and-shell-parent-device.json",
            {
                "format": EVIDENCE_FORMAT,
                "same_device": True,
            },
        )

        stage = "source-handles-precreate"
        source_lsof_argv = _lsof_argv(source)
        source_lsof = command_runner.run(
            source_lsof_argv, request.command_timeout_seconds
        )
        writer.write_json(
            "source-handles-precreate.json", source_lsof.as_json()
        )
        _require_zero_handles(source_lsof, source_lsof_argv)

        stage = "utmctl-list-precreate"
        precreate_list = command_runner.run(
            ("utmctl", "list"), request.command_timeout_seconds
        )
        writer.write_json(
            "utmctl-list-precreate.json", precreate_list.as_json()
        )
        precreate_vms = clone_control.parse_utmctl_list(precreate_list)
        _validate_precreate_inventory(precreate_vms, request)
        writer.write_json(
            "inventory-precreate.json", _inventory_evidence(precreate_vms)
        )

        stage = "create-new-registration-shell"
        create_invocations = 1
        create_observation = command_runner.run(
            _create_argv(request), request.create_timeout_seconds
        )
        writer.write_json(
            "create-new-registration-shell.json",
            create_observation.as_json(),
        )

        stage = "utmctl-list-postcreate"
        postcreate_list = command_runner.run(
            ("utmctl", "list"), request.command_timeout_seconds
        )
        writer.write_json(
            "utmctl-list-postcreate.json", postcreate_list.as_json()
        )
        postcreate_vms = clone_control.parse_utmctl_list(postcreate_list)
        writer.write_json(
            "inventory-postcreate.json", _inventory_evidence(postcreate_vms)
        )

        stage = "registration-shell-package-postcreate"
        shell_postcreate = clone_control.observe_target_package(
            request.registration_shell_package_path
        )
        shell_package_state = str(shell_postcreate["state"])
        writer.write_json(
            "registration-shell-package-postcreate.json", shell_postcreate
        )

        if _create_failed_closed_absent(
            create_observation,
            precreate_vms,
            postcreate_vms,
            shell_package_state,
        ):
            outcome = "failed-closed-absent"
            exit_code = EXIT_FAILED_CLOSED_ABSENT
            reason = "create-finished-without-registry-or-package-change"
            raise _TerminalOutcome

        shell_uuid = _parse_created_uuid(create_observation)
        _validate_created_inventory(
            precreate_vms, postcreate_vms, request, shell_uuid
        )
        if shell_package_state != "directory":
            raise RegistrationShellError(
                "registration-shell-package-not-created"
            )

        stage = "registration-shell-bundle-preupdate"
        partial_shell = bindings.validate_registration_shell_before_update(
            request, shell_uuid
        )
        writer.write_json(
            "registration-shell-bundle-preupdate.json",
            partial_shell.as_json(),
        )

        stage = "update-registration-shell-configuration"
        update_invocations = 1
        update_observation = command_runner.run(
            _update_argv(request, shell_uuid),
            request.update_timeout_seconds,
        )
        writer.write_json(
            "update-registration-shell-configuration.json",
            update_observation.as_json(),
        )

        stage = "utmctl-list-postupdate"
        postupdate_list = command_runner.run(
            ("utmctl", "list"), request.command_timeout_seconds
        )
        writer.write_json(
            "utmctl-list-postupdate.json", postupdate_list.as_json()
        )
        postupdate_vms = clone_control.parse_utmctl_list(postupdate_list)
        writer.write_json(
            "inventory-postupdate.json", _inventory_evidence(postupdate_vms)
        )

        stage = "registration-shell-package-postupdate"
        shell_postupdate = clone_control.observe_target_package(
            request.registration_shell_package_path
        )
        shell_package_state = str(shell_postupdate["state"])
        writer.write_json(
            "registration-shell-package-postupdate.json", shell_postupdate
        )
        writer.write_json(
            "registration-shell-bundle-postupdate-observation.json",
            _observe_shell_bundle(request.registration_shell_package_path),
        )

        stage = "update-registration-shell-configuration-result"
        updated_uuid = _parse_updated_uuid(update_observation)
        if updated_uuid != shell_uuid:
            raise RegistrationShellError(
                "update-command-uuid-differs-from-created-shell"
            )
        _validate_created_inventory(
            precreate_vms, postupdate_vms, request, shell_uuid
        )
        if shell_package_state != "directory":
            raise RegistrationShellError(
                "registration-shell-package-not-preserved-after-update"
            )

        stage = "registration-shell-bundle-postupdate"
        shell = bindings.validate_registration_shell_bundle(
            request, shell_uuid
        )
        writer.write_json(
            "registration-shell-bundle-postupdate.json", shell.as_json()
        )
        if (
            shell.efi_sha256 != partial_shell.efi_sha256
            or shell.qcow2_name != partial_shell.qcow2_name
            or shell.qcow2_sha256 != partial_shell.qcow2_sha256
        ):
            raise RegistrationShellError(
                "registration-shell-disk-drift-during-update"
            )
        if source.efi_path.stat().st_dev != shell.efi_path.stat().st_dev:
            raise RegistrationShellError(
                "source-and-registration-shell-not-on-same-device"
            )

        stage = "source-and-shell-handles-terminal"
        terminal_lsof_argv = _lsof_argv(source, shell)
        terminal_lsof = command_runner.run(
            terminal_lsof_argv, request.command_timeout_seconds
        )
        writer.write_json(
            "source-and-shell-handles-terminal.json",
            terminal_lsof.as_json(),
        )
        _require_zero_handles(terminal_lsof, terminal_lsof_argv)

        stage = "source-snapshot-terminal"
        source_terminal = bindings.validate_source_snapshot(request)
        writer.write_json(
            "source-snapshot-terminal.json", source_terminal.as_json()
        )
        if source_terminal != source:
            raise RegistrationShellError("source-snapshot-drift")

        stage = "utmctl-list-terminal"
        terminal_list = command_runner.run(
            ("utmctl", "list"), request.command_timeout_seconds
        )
        writer.write_json(
            "utmctl-list-terminal.json", terminal_list.as_json()
        )
        terminal_vms = clone_control.parse_utmctl_list(terminal_list)
        writer.write_json(
            "inventory-terminal.json", _inventory_evidence(terminal_vms)
        )
        if clone_control.canonical_inventory_sha256(terminal_vms) != (
            clone_control.canonical_inventory_sha256(postupdate_vms)
        ):
            raise RegistrationShellError("terminal-inventory-drift")
        _validate_created_inventory(
            precreate_vms, terminal_vms, request, shell_uuid
        )

        stage = "registration-shell-evidence-freeze"
        shell_writer = clone_control.EvidenceWriter.create(
            request.registration_shell_evidence_root
        )
        shell_writer.write_json(
            "registration-shell.json",
            bindings.expected_shell_evidence(request, shell),
        )
        shell_manifest_sha256 = shell_writer.write_manifest()

        outcome = "frozen"
        exit_code = EXIT_FROZEN
        reason = (
            "create-new-then-update-stopped-networkless-shell-frozen"
        )
    except _TerminalOutcome:
        pass
    except (
        RegistrationShellError,
        bindings.RegistrationShellBindingError,
        clone_control.CloneControlError,
        OSError,
    ) as exc:
        reason = f"{stage}:{exc}"
        if create_invocations == 1:
            outcome = "state-indeterminate"
            exit_code = EXIT_STATE_INDETERMINATE

    terminal = {
        "automatic_delete": "not-performed",
        "automatic_retry": "not-performed",
        "automatic_rollback": "not-performed",
        "automatic_start": "not-performed",
        "clone_invocations": 0,
        "create_command_exit_code": (
            create_observation.exit_code if create_observation else None
        ),
        "create_command_timed_out": (
            create_observation.timed_out if create_observation else False
        ),
        "create_invocations": create_invocations,
        "file_pull_invocations": 0,
        "file_push_invocations": 0,
        "format": EVIDENCE_FORMAT,
        "guest_exec_invocations": 0,
        "input_transfer": "not-performed",
        "operation_id": "not-generated",
        "outcome": outcome,
        "reason": reason,
        "registration_shell_manifest_sha256": shell_manifest_sha256,
        "registration_shell_name": request.registration_shell_name,
        "registration_shell_package_state": shell_package_state,
        "registration_shell_uuid": shell_uuid,
        "transaction": "not-performed",
        "update_command_exit_code": (
            update_observation.exit_code if update_observation else None
        ),
        "update_command_timed_out": (
            update_observation.timed_out if update_observation else False
        ),
        "update_invocations": update_invocations,
    }
    writer.write_json("terminal.json", terminal)
    manifest_sha256 = writer.write_manifest()
    return RegistrationShellResult(
        outcome=outcome,
        exit_code=exit_code,
        evidence_root=request.output_root,
        manifest_sha256=manifest_sha256,
        create_invocations=create_invocations,
        update_invocations=update_invocations,
        registration_shell_uuid=shell_uuid,
        registration_shell_manifest_sha256=shell_manifest_sha256,
    )


def _validate_precreate_inventory(
    registered: tuple[clone_control.RegisteredVm, ...],
    request: RegistrationShellRequest,
) -> None:
    if len(registered) != request.expected_vm_count:
        raise RegistrationShellError("precreate-vm-count-mismatch")
    if any(item.status != "stopped" for item in registered):
        raise RegistrationShellError("precreate-vm-not-all-stopped")
    if any(
        item.name == request.registration_shell_name
        for item in registered
    ):
        raise RegistrationShellError(
            "registration-shell-name-already-registered"
        )
    if clone_control.canonical_inventory_sha256(registered) != (
        request.expected_precreate_inventory_sha256
    ):
        raise RegistrationShellError(
            "precreate-inventory-sha256-mismatch"
        )


def _validate_created_inventory(
    baseline: tuple[clone_control.RegisteredVm, ...],
    terminal: tuple[clone_control.RegisteredVm, ...],
    request: RegistrationShellRequest,
    shell_uuid: str,
) -> None:
    if len(terminal) != len(baseline) + 1:
        raise RegistrationShellError("created-inventory-count-mismatch")
    baseline_by_uuid = {item.uuid: item for item in baseline}
    terminal_by_uuid = {item.uuid: item for item in terminal}
    for vm_uuid, item in baseline_by_uuid.items():
        if terminal_by_uuid.get(vm_uuid) != item:
            raise RegistrationShellError("baseline-inventory-drift")
    created = [
        item for item in terminal if item.uuid not in baseline_by_uuid
    ]
    if created != [
        clone_control.RegisteredVm(
            shell_uuid, "stopped", request.registration_shell_name
        )
    ]:
        raise RegistrationShellError(
            "registration-shell-registry-identity-invalid"
        )
    if any(item.status != "stopped" for item in terminal):
        raise RegistrationShellError("terminal-vm-not-all-stopped")


def _create_failed_closed_absent(
    observation: clone_control.CommandObservation,
    baseline: tuple[clone_control.RegisteredVm, ...],
    terminal: tuple[clone_control.RegisteredVm, ...],
    package_state: str,
) -> bool:
    return (
        not observation.timed_out
        and observation.exit_code is not None
        and package_state == "absent"
        and clone_control.canonical_inventory_sha256(baseline)
        == clone_control.canonical_inventory_sha256(terminal)
    )


def _parse_created_uuid(
    observation: clone_control.CommandObservation,
) -> str:
    return _parse_command_uuid(observation, "create")


def _parse_updated_uuid(
    observation: clone_control.CommandObservation,
) -> str:
    return _parse_command_uuid(observation, "update")


def _parse_command_uuid(
    observation: clone_control.CommandObservation,
    action: str,
) -> str:
    if (
        observation.timed_out
        or observation.exit_code != 0
        or observation.stderr.total_bytes != 0
        or observation.stdout.truncated
    ):
        raise RegistrationShellError(
            f"{action}-command-not-silent-success"
        )
    try:
        value = observation.stdout.prefix.decode("ascii").strip()
        canonical = str(uuid.UUID(value)).upper()
    except (UnicodeDecodeError, ValueError) as exc:
        raise RegistrationShellError(
            f"{action}-command-uuid-output-invalid"
        ) from exc
    if value != canonical:
        raise RegistrationShellError(
            f"{action}-command-uuid-output-not-canonical"
        )
    return canonical


def _inventory_evidence(
    registered: tuple[clone_control.RegisteredVm, ...],
) -> dict[str, object]:
    return {
        "all_stopped": all(item.status == "stopped" for item in registered),
        "canonical_inventory_sha256": (
            clone_control.canonical_inventory_sha256(registered)
        ),
        "format": EVIDENCE_FORMAT,
        "registered_vm_count": len(registered),
    }


def _create_argv(
    request: RegistrationShellRequest,
) -> tuple[str, ...]:
    return (
        OSASCRIPT_PATH,
        str(request.repository_root / bindings.TRANSPORT_RELATIVE_PATH),
        "create",
        request.registration_shell_name,
    )


def _update_argv(
    request: RegistrationShellRequest,
    shell_uuid: str,
) -> tuple[str, ...]:
    return (
        OSASCRIPT_PATH,
        str(request.repository_root / bindings.TRANSPORT_RELATIVE_PATH),
        "update",
        shell_uuid,
        request.registration_shell_name,
    )


def _observe_shell_bundle(root: Path) -> dict[str, object]:
    try:
        identity = bindings.read_bundle(root, root_mode=0o755)
    except bindings.RegistrationShellBindingError as exc:
        return {
            "format": EVIDENCE_FORMAT,
            "reason": str(exc),
            "state": "invalid",
        }
    return {
        **identity.as_json(),
        "format": EVIDENCE_FORMAT,
        "state": "readable",
    }


def _lsof_argv(
    *identities: bindings.BundleIdentity,
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
    return (LSOF_PATH, "-n", "-P", "-F", "ctfn", *paths)


def _require_zero_handles(
    observation: clone_control.CommandObservation,
    expected_argv: tuple[str, ...],
) -> None:
    if observation.argv != expected_argv:
        raise RegistrationShellError("lsof-argv-drift")
    if (
        observation.timed_out
        or observation.exit_code != 1
        or observation.stdout.total_bytes != 0
        or observation.stderr.total_bytes != 0
        or observation.stdout.truncated
        or observation.stderr.truncated
    ):
        raise RegistrationShellError(
            "disk-open-handles-or-lsof-indeterminate"
        )


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Create, update, and freeze one dedicated stopped "
            "registration-only UTM shell for the upgrade_quiesced L6 case."
        )
    )
    parser.add_argument("command", choices=("create-freeze-once",))
    parser.add_argument("--repository-root", type=Path, required=True)
    parser.add_argument("--expected-repository-head", required=True)
    parser.add_argument("--output-root", type=Path, required=True)
    parser.add_argument("--attempt-id", required=True)
    parser.add_argument("--operator-asset-root", type=Path, required=True)
    parser.add_argument(
        "--source-snapshot-root", type=Path, required=True
    )
    parser.add_argument(
        "--registration-shell-evidence-root", type=Path, required=True
    )
    parser.add_argument("--registration-shell-name", required=True)
    parser.add_argument(
        "--registration-shell-package-path", type=Path, required=True
    )
    parser.add_argument("--expected-vm-count", type=int, required=True)
    parser.add_argument(
        "--expected-precreate-inventory-sha256", required=True
    )
    parser.add_argument(
        "--create-timeout-seconds", type=int, default=120
    )
    parser.add_argument(
        "--update-timeout-seconds", type=int, default=120
    )
    parser.add_argument(
        "--command-timeout-seconds", type=int, default=15
    )
    parser.add_argument(
        "--authorized-upgrade-quiesced-registration-shell",
        action="store_true",
    )
    parser.add_argument(
        "--authorized-one-create-new-registration-only-shell",
        action="store_true",
    )
    parser.add_argument(
        "--authorized-one-stopped-configuration-update",
        action="store_true",
    )
    parser.add_argument(
        "--authorized-no-clone-start-guest-delete-retry-or-transaction",
        action="store_true",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    request = RegistrationShellRequest(
        repository_root=args.repository_root,
        expected_repository_head=args.expected_repository_head,
        output_root=args.output_root,
        attempt_id=args.attempt_id,
        operator_asset_root=args.operator_asset_root,
        source_snapshot_root=args.source_snapshot_root,
        registration_shell_evidence_root=(
            args.registration_shell_evidence_root
        ),
        registration_shell_name=args.registration_shell_name,
        registration_shell_package_path=(
            args.registration_shell_package_path
        ),
        expected_vm_count=args.expected_vm_count,
        expected_precreate_inventory_sha256=(
            args.expected_precreate_inventory_sha256
        ),
        create_timeout_seconds=args.create_timeout_seconds,
        update_timeout_seconds=args.update_timeout_seconds,
        command_timeout_seconds=args.command_timeout_seconds,
        authorized_upgrade_quiesced_registration_shell=(
            args.authorized_upgrade_quiesced_registration_shell
        ),
        authorized_one_create_new_registration_only_shell=(
            args.authorized_one_create_new_registration_only_shell
        ),
        authorized_one_stopped_configuration_update=(
            args.authorized_one_stopped_configuration_update
        ),
        authorized_no_clone_start_guest_delete_retry_or_transaction=(
            args.authorized_no_clone_start_guest_delete_retry_or_transaction
        ),
    )
    try:
        result = run_registration_shell_once(request)
    except RegistrationShellError as exc:
        print(
            f"l6_upgrade_quiesced_registration_shell_error={exc}",
            file=sys.stderr,
        )
        return EXIT_PRECONDITION_REJECTED
    print(f"registration_shell_outcome={result.outcome}")
    print(f"evidence_root={result.evidence_root}")
    print(f"files_sha256={result.manifest_sha256}")
    if result.registration_shell_uuid is not None:
        print(
            "registration_shell_uuid="
            f"{result.registration_shell_uuid}"
        )
    if result.registration_shell_manifest_sha256 is not None:
        print(
            "registration_shell_files_sha256="
            f"{result.registration_shell_manifest_sha256}"
        )
    return result.exit_code


if __name__ == "__main__":
    raise SystemExit(main())
