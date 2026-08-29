#!/usr/bin/env python3
from __future__ import annotations

import argparse
import os
import plistlib
import re
import stat
import sys
import uuid
from dataclasses import dataclass
from pathlib import Path
from typing import Callable, Protocol

import l6_upgrade_quiesced_case_contract as case_contract
import l6_upgrade_quiesced_clone_bindings as clone_bindings
import l6_utm_clone_once as clone_control


EVIDENCE_FORMAT = clone_bindings.EVIDENCE_FORMAT
EXIT_PREPARED = 0
EXIT_FAILED_CLOSED_ABSENT = 10
EXIT_PRECONDITION_REJECTED = 11
EXIT_STATE_INDETERMINATE = 12
HEX_40 = re.compile(r"[0-9a-f]{40}")
HEX_64 = re.compile(r"[0-9a-f]{64}")
SAFE_ATTEMPT_ID = re.compile(r"[a-z0-9][a-z0-9-]{0,95}")


class CloneFrontDoorError(ValueError):
    pass


class _TerminalOutcome(Exception):
    pass


@dataclass(frozen=True)
class CloneFrontDoorRequest:
    repository_root: Path
    expected_repository_head: str
    output_root: Path
    attempt_id: str
    operator_asset_root: Path
    source_snapshot_root: Path
    registration_shell_evidence_root: Path
    registration_shell_manifest_sha256: str
    registration_shell_uuid: str
    registration_shell_name: str
    registration_shell_package_path: Path
    target_name: str
    target_package_path: Path
    expected_vm_count: int
    expected_preclone_inventory_sha256: str
    clone_timeout_seconds: int
    command_timeout_seconds: int
    authorized_upgrade_quiesced_clone: bool
    authorized_dedicated_registration_shell: bool
    authorized_no_automatic_retry_delete_start_or_rollback: bool

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
            (self.target_package_path, "target-package-path"),
        )
        for path, label in paths:
            if not path.is_absolute() or ".." in path.parts:
                raise CloneFrontDoorError(
                    f"{label}-must-be-absolute-normalized"
                )
        if clone_control._path_is_within(
            self.output_root, self.repository_root
        ):
            raise CloneFrontDoorError("output-root-must-be-outside-repository")
        for protected, label in (
            (self.source_snapshot_root, "source-snapshot"),
            (self.registration_shell_evidence_root, "shell-evidence"),
            (self.registration_shell_package_path, "registration-shell"),
            (self.target_package_path, "target-package"),
        ):
            if clone_control._paths_overlap(self.output_root, protected):
                raise CloneFrontDoorError(f"output-root-overlaps-{label}")
        expected_snapshot = self.operator_asset_root / str(
            case_contract.EXPECTED_START["relative_path"]
        )
        if self.source_snapshot_root != expected_snapshot:
            raise CloneFrontDoorError("source-snapshot-path-differs-from-case")
        if not HEX_40.fullmatch(self.expected_repository_head):
            raise CloneFrontDoorError("expected-repository-head-invalid")
        for digest, label in (
            (
                self.registration_shell_manifest_sha256,
                "registration-shell-manifest-sha256",
            ),
            (
                self.expected_preclone_inventory_sha256,
                "expected-preclone-inventory-sha256",
            ),
        ):
            if not HEX_64.fullmatch(digest):
                raise CloneFrontDoorError(f"{label}-invalid")
        if not SAFE_ATTEMPT_ID.fullmatch(self.attempt_id):
            raise CloneFrontDoorError("attempt-id-invalid")
        _validate_uuid(
            self.registration_shell_uuid, "registration-shell-uuid"
        )
        _validate_vm_name(
            self.registration_shell_name, "registration-shell-name"
        )
        _validate_vm_name(self.target_name, "target-name")
        if self.registration_shell_name == self.target_name:
            raise CloneFrontDoorError(
                "registration-shell-and-target-name-match"
            )
        if self.registration_shell_package_path.name != (
            f"{self.registration_shell_name}.utm"
        ):
            raise CloneFrontDoorError(
                "registration-shell-package-name-mismatch"
            )
        if self.target_package_path.name != f"{self.target_name}.utm":
            raise CloneFrontDoorError("target-package-name-mismatch")
        if (
            self.registration_shell_package_path.parent
            != self.target_package_path.parent
        ):
            raise CloneFrontDoorError(
                "registration-shell-and-target-parent-differ"
            )
        if not 1 <= self.expected_vm_count <= 128:
            raise CloneFrontDoorError("expected-vm-count-out-of-range")
        if not 1 <= self.clone_timeout_seconds <= 300:
            raise CloneFrontDoorError("clone-timeout-seconds-out-of-range")
        if not 1 <= self.command_timeout_seconds <= 60:
            raise CloneFrontDoorError("command-timeout-seconds-out-of-range")
        if not self.authorized_upgrade_quiesced_clone:
            raise CloneFrontDoorError(
                "upgrade-quiesced-clone-authorization-required"
            )
        if not self.authorized_dedicated_registration_shell:
            raise CloneFrontDoorError(
                "dedicated-registration-shell-authorization-required"
            )
        if not self.authorized_no_automatic_retry_delete_start_or_rollback:
            raise CloneFrontDoorError(
                "no-automatic-retry-delete-start-or-rollback-authorization-required"
            )

    def as_json(self) -> dict[str, object]:
        return {
            "attempt_id": self.attempt_id,
            "authorization": {
                "dedicated_registration_shell": True,
                "no_automatic_retry_delete_start_or_rollback": True,
                "upgrade_quiesced_clone": True,
            },
            "clone_timeout_seconds": self.clone_timeout_seconds,
            "command_timeout_seconds": self.command_timeout_seconds,
            "expected_preclone_inventory_sha256": (
                self.expected_preclone_inventory_sha256
            ),
            "expected_repository_head": self.expected_repository_head,
            "expected_vm_count": self.expected_vm_count,
            "format": EVIDENCE_FORMAT,
            "operator_asset_root_sha256": _sha256_text(
                str(self.operator_asset_root)
            ),
            "registration_shell_evidence_root_sha256": _sha256_text(
                str(self.registration_shell_evidence_root)
            ),
            "registration_shell_manifest_sha256": (
                self.registration_shell_manifest_sha256
            ),
            "registration_shell_name": self.registration_shell_name,
            "registration_shell_package_path_sha256": _sha256_text(
                str(self.registration_shell_package_path)
            ),
            "registration_shell_uuid": self.registration_shell_uuid,
            "source_snapshot_path_sha256": _sha256_text(
                str(self.source_snapshot_root)
            ),
            "target_name": self.target_name,
            "target_package_path_sha256": _sha256_text(
                str(self.target_package_path)
            ),
        }


@dataclass(frozen=True)
class CloneFrontDoorResult:
    outcome: str
    exit_code: int
    evidence_root: Path
    manifest_sha256: str
    clone_invocations: int
    replacement_count: int
    target_uuid: str | None


class CommandRunner(Protocol):
    def run(
        self, argv: tuple[str, ...], timeout_seconds: int
    ) -> clone_control.CommandObservation: ...


BundleIdentity = clone_bindings.BundleIdentity
BindingValidator = Callable[[CloneFrontDoorRequest], dict[str, object]]


def run_clone_front_door(
    request: CloneFrontDoorRequest,
    *,
    runner: CommandRunner | None = None,
    binding_validator: BindingValidator | None = None,
) -> CloneFrontDoorResult:
    request.validate()
    writer = clone_control.EvidenceWriter.create(request.output_root)
    writer.write_json("request.json", request.as_json())
    command_runner = runner or clone_control.SubprocessCommandRunner()
    validate_bindings = (
        binding_validator or validate_clone_front_door_bindings
    )

    stage = "binding-preflight"
    outcome = "precondition-rejected"
    exit_code = EXIT_PRECONDITION_REJECTED
    reason = "not-run"
    clone_invocations = 0
    replacement_count = 0
    target_uuid: str | None = None
    clone_observation: clone_control.CommandObservation | None = None
    target_state = "not-observed"

    try:
        writer.write_json(
            "binding-preflight.json", validate_bindings(request)
        )

        stage = "source-snapshot-preflight"
        source = validate_source_snapshot(request)
        writer.write_json(
            "source-snapshot-preflight.json", source.as_json()
        )

        stage = "registration-shell-preflight"
        registration = validate_registration_shell(request)
        writer.write_json(
            "registration-shell-preflight.json", registration.as_json()
        )
        if source.efi_path.stat().st_dev != registration.efi_path.stat().st_dev:
            raise CloneFrontDoorError(
                "source-and-registration-shell-not-on-same-clone-device"
            )

        stage = "target-package-preclone"
        target_preclone = clone_control.observe_target_package(
            request.target_package_path
        )
        writer.write_json("target-package-preclone.json", target_preclone)
        if target_preclone["state"] != "absent":
            raise CloneFrontDoorError(
                "target-package-not-absent-before-clone"
            )

        stage = "source-and-shell-handles-preflight"
        preclone_lsof_argv = _lsof_argv(source, registration)
        preclone_lsof = command_runner.run(
            preclone_lsof_argv, request.command_timeout_seconds
        )
        writer.write_json(
            "source-and-shell-handles-preflight.json",
            preclone_lsof.as_json(),
        )
        _require_zero_handles(preclone_lsof, preclone_lsof_argv)

        stage = "utmctl-list-preclone"
        preclone_list = command_runner.run(
            ("utmctl", "list"), request.command_timeout_seconds
        )
        writer.write_json(
            "utmctl-list-preclone.json", preclone_list.as_json()
        )
        preclone_vms = clone_control.parse_utmctl_list(preclone_list)
        _validate_preclone_inventory(preclone_vms, request)

        stage = "utmctl-clone"
        clone_invocations = 1
        clone_observation = command_runner.run(
            _clone_argv(request), request.clone_timeout_seconds
        )
        writer.write_json("utmctl-clone.json", clone_observation.as_json())

        stage = "utmctl-list-postclone"
        postclone_list = command_runner.run(
            ("utmctl", "list"), request.command_timeout_seconds
        )
        writer.write_json(
            "utmctl-list-postclone.json", postclone_list.as_json()
        )
        postclone_vms = clone_control.parse_utmctl_list(postclone_list)

        stage = "target-package-postclone"
        target_package = clone_control.observe_target_package(
            request.target_package_path
        )
        target_state = str(target_package["state"])
        writer.write_json("target-package-postclone.json", target_package)

        if clone_control.clone_failed_closed_absent(
            clone_observation, preclone_vms, postclone_vms, target_state
        ):
            outcome = "failed-closed-absent"
            exit_code = EXIT_FAILED_CLOSED_ABSENT
            reason = "clone-finished-without-registry-or-package-change"
            raise _TerminalOutcome
        if not clone_control.clone_created(
            clone_observation,
            preclone_vms,
            postclone_vms,
            target_state,
            _as_clone_request(request),
        ):
            raise CloneFrontDoorError("clone-postconditions-indeterminate")
        baseline_uuids = {item.uuid for item in preclone_vms}
        target_uuid = next(
            item.uuid
            for item in postclone_vms
            if item.uuid not in baseline_uuids
        )

        stage = "target-registration-shell-identity"
        target_before = validate_target_bundle(
            request, registration, target_uuid, materialized=False
        )
        writer.write_json(
            "target-before-materialization.json", target_before.as_json()
        )

        stage = "all-handles-prematerialization"
        pre_materialize_lsof_argv = _lsof_argv(
            source, registration, target_before
        )
        pre_materialize_lsof = command_runner.run(
            pre_materialize_lsof_argv, request.command_timeout_seconds
        )
        writer.write_json(
            "all-handles-prematerialization.json",
            pre_materialize_lsof.as_json(),
        )
        _require_zero_handles(
            pre_materialize_lsof, pre_materialize_lsof_argv
        )

        data_root = request.target_package_path / "Data"
        efi_incoming = data_root / (
            f".efi_vars.fd.{request.attempt_id}.incoming"
        )
        qcow2_incoming = data_root / (
            f".{source.qcow2_name}.{request.attempt_id}.incoming"
        )
        for incoming in (efi_incoming, qcow2_incoming):
            if incoming.exists() or incoming.is_symlink():
                raise CloneFrontDoorError("target-incoming-already-exists")

        stage = "clone-source-members"
        for source_path, incoming_path, label in (
            (source.efi_path, efi_incoming, "efi"),
            (source.qcow2_path, qcow2_incoming, "qcow2"),
        ):
            observation = command_runner.run(
                ("/bin/cp", "-c", str(source_path), str(incoming_path)),
                request.clone_timeout_seconds,
            )
            writer.write_json(
                f"clonefile-{label}.json", observation.as_json()
            )
            _require_silent_success(observation, f"clonefile-{label}")
            expected_hash = (
                source.efi_sha256
                if label == "efi"
                else source.qcow2_sha256
            )
            _validate_regular_file(
                incoming_path, 0o644, expected_hash, label
            )

        stage = "replace-target-efi"
        efi_replace = command_runner.run(
            (
                "/bin/mv",
                "-f",
                str(efi_incoming),
                str(target_before.efi_path),
            ),
            request.command_timeout_seconds,
        )
        writer.write_json(
            "replace-target-efi.json", efi_replace.as_json()
        )
        _require_silent_success(efi_replace, "replace-target-efi")
        replacement_count = 1
        _fsync_directory(data_root)

        stage = "replace-target-qcow2"
        qcow2_replace = command_runner.run(
            (
                "/bin/mv",
                "-f",
                str(qcow2_incoming),
                str(target_before.qcow2_path),
            ),
            request.command_timeout_seconds,
        )
        writer.write_json(
            "replace-target-qcow2.json", qcow2_replace.as_json()
        )
        _require_silent_success(qcow2_replace, "replace-target-qcow2")
        replacement_count = 2
        _fsync_directory(data_root)

        stage = "target-materialized-identity"
        target_after = validate_target_bundle(
            request,
            registration,
            target_uuid,
            materialized=True,
            source=source,
        )
        writer.write_json(
            "target-after-materialization.json", target_after.as_json()
        )
        if _sha256_file(target_after.qcow2_path) != source.qcow2_sha256:
            raise CloneFrontDoorError("target-qcow2-second-hash-mismatch")

        stage = "source-shell-target-postflight"
        source_after = validate_source_snapshot(request)
        registration_after = validate_registration_shell(request)
        if source_after != source or registration_after != registration:
            raise CloneFrontDoorError("source-or-registration-shell-drift")
        post_lsof_argv = _lsof_argv(
            source_after, registration_after, target_after
        )
        post_lsof = command_runner.run(
            post_lsof_argv, request.command_timeout_seconds
        )
        writer.write_json(
            "all-handles-postflight.json", post_lsof.as_json()
        )
        _require_zero_handles(post_lsof, post_lsof_argv)

        stage = "utmctl-list-terminal"
        terminal_list = command_runner.run(
            ("utmctl", "list"), request.command_timeout_seconds
        )
        writer.write_json(
            "utmctl-list-terminal.json", terminal_list.as_json()
        )
        terminal_vms = clone_control.parse_utmctl_list(terminal_list)
        if clone_control.canonical_inventory_sha256(terminal_vms) != (
            clone_control.canonical_inventory_sha256(postclone_vms)
        ):
            raise CloneFrontDoorError("terminal-inventory-drift")
        if any(item.status != "stopped" for item in terminal_vms):
            raise CloneFrontDoorError("terminal-vm-not-all-stopped")

        outcome = "prepared"
        exit_code = EXIT_PREPARED
        reason = "registered-target-materialized-from-immutable-s2"
    except _TerminalOutcome:
        pass
    except (
        CloneFrontDoorError,
        clone_bindings.CloneBindingError,
        clone_control.CloneControlError,
        OSError,
    ) as exc:
        reason = f"{stage}:{exc}"
        if clone_invocations == 1:
            outcome = "state-indeterminate"
            exit_code = EXIT_STATE_INDETERMINATE

    terminal = {
        "automatic_delete": "not-performed",
        "automatic_retry": "not-performed",
        "automatic_rollback": "not-performed",
        "automatic_start": "not-performed",
        "clone_command_exit_code": (
            clone_observation.exit_code if clone_observation else None
        ),
        "clone_command_timed_out": (
            clone_observation.timed_out if clone_observation else False
        ),
        "clone_invocations": clone_invocations,
        "format": EVIDENCE_FORMAT,
        "guest_exec": "not-performed",
        "input_transfer": "not-performed",
        "operation_id": "not-generated",
        "outcome": outcome,
        "reason": reason,
        "replacement_count": replacement_count,
        "target_name": request.target_name,
        "target_package_state": target_state,
        "target_uuid": target_uuid,
        "transaction": "not-performed",
    }
    writer.write_json("terminal.json", terminal)
    manifest_sha256 = writer.write_manifest()
    return CloneFrontDoorResult(
        outcome=outcome,
        exit_code=exit_code,
        evidence_root=request.output_root,
        manifest_sha256=manifest_sha256,
        clone_invocations=clone_invocations,
        replacement_count=replacement_count,
        target_uuid=target_uuid,
    )


def validate_clone_front_door_bindings(
    request: CloneFrontDoorRequest,
) -> dict[str, object]:
    return clone_bindings.validate_clone_front_door_bindings(request)


def validate_source_snapshot(
    request: CloneFrontDoorRequest,
) -> BundleIdentity:
    identity = _read_bundle(request.source_snapshot_root, root_mode=0o700)
    expected = case_contract.EXPECTED_START
    if (
        identity.config_sha256 != expected["config_sha256"]
        or identity.efi_sha256 != expected["efi_sha256"]
        or identity.qcow2_sha256 != expected["qcow2_sha256"]
    ):
        raise CloneFrontDoorError("source-snapshot-disk-identity-drift")
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


def validate_registration_shell(
    request: CloneFrontDoorRequest,
) -> BundleIdentity:
    identity = _read_bundle(
        request.registration_shell_package_path, root_mode=0o755
    )
    if (
        identity.uuid != request.registration_shell_uuid
        or identity.name != request.registration_shell_name
        or identity.network != []
    ):
        raise CloneFrontDoorError(
            "registration-shell-runtime-identity-drift"
        )
    return identity


def validate_target_bundle(
    request: CloneFrontDoorRequest,
    registration: BundleIdentity,
    target_uuid: str,
    *,
    materialized: bool,
    source: BundleIdentity | None = None,
) -> BundleIdentity:
    identity = _read_bundle(request.target_package_path, root_mode=0o755)
    if identity.name != request.target_name or identity.uuid != target_uuid:
        raise CloneFrontDoorError("target-name-or-uuid-drift")
    if identity.network != []:
        raise CloneFrontDoorError("target-network-must-be-empty")
    if _normalized_plist(registration.config_path) != _normalized_plist(
        identity.config_path
    ):
        raise CloneFrontDoorError(
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
        raise CloneFrontDoorError("target-materialization-state-mismatch")
    return identity


def _read_bundle(root: Path, *, root_mode: int) -> BundleIdentity:
    _validate_directory(root, root_mode, "bundle-root")
    data = root / "Data"
    _validate_directory(data, root_mode, "bundle-data")
    config = root / "config.plist"
    _validate_regular_file(config, 0o644, None, "config")
    try:
        value = plistlib.loads(config.read_bytes())
    except (OSError, plistlib.InvalidFileException) as exc:
        raise CloneFrontDoorError("config-plist-invalid") from exc
    if not isinstance(value, dict):
        raise CloneFrontDoorError("config-plist-root-invalid")
    information = value.get("Information")
    drives = value.get("Drive")
    if not isinstance(information, dict) or not isinstance(drives, list):
        raise CloneFrontDoorError("config-required-fields-invalid")
    disks = [
        item
        for item in drives
        if isinstance(item, dict) and item.get("ImageType") == "Disk"
    ]
    if len(disks) != 1 or disks[0].get("Interface") != "VirtIO":
        raise CloneFrontDoorError("config-disk-contract-invalid")
    qcow2_name = disks[0].get("ImageName")
    if (
        not isinstance(qcow2_name, str)
        or Path(qcow2_name).name != qcow2_name
        or not qcow2_name.endswith(".qcow2")
    ):
        raise CloneFrontDoorError("config-qcow2-name-invalid")
    name = information.get("Name")
    vm_uuid = information.get("UUID")
    if not isinstance(name, str) or not isinstance(vm_uuid, str):
        raise CloneFrontDoorError("config-information-invalid")
    _validate_uuid(vm_uuid, "config-uuid")
    efi = data / "efi_vars.fd"
    qcow2 = data / qcow2_name
    _validate_regular_file(efi, 0o644, None, "efi")
    _validate_regular_file(qcow2, 0o644, None, "qcow2")
    return BundleIdentity(
        config_path=config,
        efi_path=efi,
        qcow2_path=qcow2,
        config_sha256=_sha256_file(config),
        efi_sha256=_sha256_file(efi),
        qcow2_sha256=_sha256_file(qcow2),
        qcow2_name=qcow2_name,
        name=name,
        uuid=vm_uuid,
        network=value.get("Network"),
    )


def _validate_preclone_inventory(
    registered: tuple[clone_control.RegisteredVm, ...],
    request: CloneFrontDoorRequest,
) -> None:
    if len(registered) != request.expected_vm_count:
        raise CloneFrontDoorError("preclone-vm-count-mismatch")
    if any(item.status != "stopped" for item in registered):
        raise CloneFrontDoorError("preclone-vm-not-all-stopped")
    shell = next(
        (
            item
            for item in registered
            if item.uuid == request.registration_shell_uuid
        ),
        None,
    )
    if shell is None or shell.name != request.registration_shell_name:
        raise CloneFrontDoorError(
            "registration-shell-not-uniquely-registered"
        )
    if any(item.name == request.target_name for item in registered):
        raise CloneFrontDoorError("target-name-already-registered")
    if clone_control.canonical_inventory_sha256(registered) != (
        request.expected_preclone_inventory_sha256
    ):
        raise CloneFrontDoorError("preclone-inventory-sha256-mismatch")


def _as_clone_request(
    request: CloneFrontDoorRequest,
) -> clone_control.CloneRequest:
    return clone_control.CloneRequest(
        repository_root=request.repository_root,
        expected_repository_head=request.expected_repository_head,
        prior_failure_root=request.registration_shell_evidence_root,
        prior_failure_manifest_sha256=(
            request.registration_shell_manifest_sha256
        ),
        output_root=request.output_root,
        attempt_id=request.attempt_id,
        source_uuid=request.registration_shell_uuid,
        source_name=request.registration_shell_name,
        target_name=request.target_name,
        target_package_path=request.target_package_path,
        expected_vm_count=request.expected_vm_count,
        expected_preclone_inventory_sha256=(
            request.expected_preclone_inventory_sha256
        ),
        clone_timeout_seconds=request.clone_timeout_seconds,
        command_timeout_seconds=request.command_timeout_seconds,
        authorized_l6_vm_clone=True,
        authorized_no_automatic_retry_delete_or_start=True,
    )


def _clone_argv(request: CloneFrontDoorRequest) -> tuple[str, ...]:
    return (
        "utmctl",
        "clone",
        request.registration_shell_uuid,
        "--name",
        request.target_name,
    )


def _lsof_argv(*identities: BundleIdentity) -> tuple[str, ...]:
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
        raise CloneFrontDoorError("lsof-argv-drift")
    if (
        observation.timed_out
        or observation.exit_code != 1
        or observation.stdout.total_bytes != 0
        or observation.stderr.total_bytes != 0
        or observation.stdout.truncated
        or observation.stderr.truncated
    ):
        raise CloneFrontDoorError(
            "disk-open-handles-or-lsof-indeterminate"
        )


def _require_silent_success(
    observation: clone_control.CommandObservation, label: str
) -> None:
    if (
        observation.timed_out
        or observation.exit_code != 0
        or observation.stdout.total_bytes != 0
        or observation.stderr.total_bytes != 0
    ):
        raise CloneFrontDoorError(f"{label}-failed")


def _normalized_plist(path: Path) -> object:
    try:
        value = plistlib.loads(path.read_bytes())
    except (OSError, plistlib.InvalidFileException) as exc:
        raise CloneFrontDoorError("config-plist-invalid") from exc
    if (
        not isinstance(value, dict)
        or not isinstance(value.get("Information"), dict)
    ):
        raise CloneFrontDoorError("config-information-invalid")
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
        raise CloneFrontDoorError(f"{label}-unavailable") from exc
    if (
        not stat.S_ISDIR(metadata.st_mode)
        or stat.S_ISLNK(metadata.st_mode)
        or stat.S_IMODE(metadata.st_mode) != mode
        or metadata.st_uid != os.getuid()
        or metadata.st_gid != os.getgid()
    ):
        raise CloneFrontDoorError(f"{label}-identity-invalid")


def _validate_regular_file(
    path: Path,
    mode: int,
    expected_sha256: str | None,
    label: str,
) -> None:
    try:
        metadata = path.lstat()
    except OSError as exc:
        raise CloneFrontDoorError(f"{label}-unavailable") from exc
    if (
        not stat.S_ISREG(metadata.st_mode)
        or stat.S_ISLNK(metadata.st_mode)
        or stat.S_IMODE(metadata.st_mode) != mode
        or metadata.st_uid != os.getuid()
        or metadata.st_gid != os.getgid()
        or metadata.st_nlink != 1
    ):
        raise CloneFrontDoorError(f"{label}-identity-invalid")
    if expected_sha256 is not None and _sha256_file(path) != expected_sha256:
        raise CloneFrontDoorError(f"{label}-sha256-mismatch")


_sha256_file = clone_bindings.sha256_file
_sha256_text = clone_bindings.sha256_text


def _validate_uuid(value: str, label: str) -> None:
    try:
        canonical = str(uuid.UUID(value)).upper()
    except ValueError as exc:
        raise CloneFrontDoorError(f"{label}-invalid") from exc
    if canonical != value:
        raise CloneFrontDoorError(f"{label}-must-be-uppercase-canonical")


def _validate_vm_name(value: str, label: str) -> None:
    if (
        not value
        or len(value.encode("utf-8")) > 160
        or any(character in "\x00\r\n/" for character in value)
    ):
        raise CloneFrontDoorError(f"{label}-invalid")


def _fsync_directory(path: Path) -> None:
    descriptor = os.open(path, os.O_RDONLY)
    try:
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Create one registered stopped target through a dedicated shell, "
            "then materialize only immutable S2 EFI/qcow2 bytes."
        )
    )
    parser.add_argument("command", choices=("clone-materialize-once",))
    parser.add_argument("--repository-root", type=Path, required=True)
    parser.add_argument("--expected-repository-head", required=True)
    parser.add_argument("--output-root", type=Path, required=True)
    parser.add_argument("--attempt-id", required=True)
    parser.add_argument("--operator-asset-root", type=Path, required=True)
    parser.add_argument("--source-snapshot-root", type=Path, required=True)
    parser.add_argument(
        "--registration-shell-evidence-root", type=Path, required=True
    )
    parser.add_argument("--registration-shell-manifest-sha256", required=True)
    parser.add_argument("--registration-shell-uuid", required=True)
    parser.add_argument("--registration-shell-name", required=True)
    parser.add_argument(
        "--registration-shell-package-path", type=Path, required=True
    )
    parser.add_argument("--target-name", required=True)
    parser.add_argument("--target-package-path", type=Path, required=True)
    parser.add_argument("--expected-vm-count", type=int, required=True)
    parser.add_argument("--expected-preclone-inventory-sha256", required=True)
    parser.add_argument("--clone-timeout-seconds", type=int, default=120)
    parser.add_argument("--command-timeout-seconds", type=int, default=15)
    parser.add_argument(
        "--authorized-upgrade-quiesced-clone", action="store_true"
    )
    parser.add_argument(
        "--authorized-dedicated-registration-shell", action="store_true"
    )
    parser.add_argument(
        "--authorized-no-automatic-retry-delete-start-or-rollback",
        action="store_true",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    request = CloneFrontDoorRequest(
        repository_root=args.repository_root,
        expected_repository_head=args.expected_repository_head,
        output_root=args.output_root,
        attempt_id=args.attempt_id,
        operator_asset_root=args.operator_asset_root,
        source_snapshot_root=args.source_snapshot_root,
        registration_shell_evidence_root=(
            args.registration_shell_evidence_root
        ),
        registration_shell_manifest_sha256=(
            args.registration_shell_manifest_sha256
        ),
        registration_shell_uuid=args.registration_shell_uuid,
        registration_shell_name=args.registration_shell_name,
        registration_shell_package_path=(
            args.registration_shell_package_path
        ),
        target_name=args.target_name,
        target_package_path=args.target_package_path,
        expected_vm_count=args.expected_vm_count,
        expected_preclone_inventory_sha256=(
            args.expected_preclone_inventory_sha256
        ),
        clone_timeout_seconds=args.clone_timeout_seconds,
        command_timeout_seconds=args.command_timeout_seconds,
        authorized_upgrade_quiesced_clone=(
            args.authorized_upgrade_quiesced_clone
        ),
        authorized_dedicated_registration_shell=(
            args.authorized_dedicated_registration_shell
        ),
        authorized_no_automatic_retry_delete_start_or_rollback=(
            args.authorized_no_automatic_retry_delete_start_or_rollback
        ),
    )
    try:
        result = run_clone_front_door(request)
    except CloneFrontDoorError as exc:
        print(
            f"l6_upgrade_quiesced_clone_front_door_error={exc}",
            file=sys.stderr,
        )
        return EXIT_PRECONDITION_REJECTED
    print(f"clone_front_door_outcome={result.outcome}")
    print(f"evidence_root={result.evidence_root}")
    print(f"files_sha256={result.manifest_sha256}")
    return result.exit_code


if __name__ == "__main__":
    raise SystemExit(main())
