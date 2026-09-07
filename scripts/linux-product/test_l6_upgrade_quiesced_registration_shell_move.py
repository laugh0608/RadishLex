#!/usr/bin/env python3
from __future__ import annotations

import hashlib
import json
import plistlib
import tempfile
import unittest
from pathlib import Path
from unittest import mock

import l6_upgrade_quiesced_case_contract as case_contract
import l6_upgrade_quiesced_registration_shell as shell_control
import l6_upgrade_quiesced_registration_shell_bindings as shell_bindings
import l6_upgrade_quiesced_registration_shell_move as control
import l6_upgrade_quiesced_registration_shell_move_bindings as bindings
import l6_utm_clone_once as clone_control


PEER_UUID = "11111111-1111-4111-8111-111111111111"
QCOW2_NAME = "EF62CC7C-DF55-4A83-8633-F4630CF3B234.qcow2"


class FakeRunner:
    def __init__(
        self,
        request: control.MoveRequestBase,
        *,
        handles_present: bool = False,
        running_peer: bool = False,
        update_lands: bool = True,
        update_disk_drift: bool = False,
        update_exit_code: int = 0,
    ) -> None:
        self.request = request
        self.handles_present = handles_present
        self.running_peer = running_peer
        self.update_lands = update_lands
        self.update_disk_drift = update_disk_drift
        self.update_exit_code = update_exit_code
        self.calls: list[tuple[str, ...]] = []

    def run(
        self, argv: tuple[str, ...], timeout_seconds: int
    ) -> clone_control.CommandObservation:
        del timeout_seconds
        self.calls.append(argv)
        if argv[0] == shell_control.LSOF_PATH:
            if self.handles_present:
                self.handles_present = False
                return observation(argv, stdout=b"p123\ncQEMULauncher\nf4\ntREG\n")
            return observation(argv, exit_code=1)
        if argv == ("utmctl", "list"):
            return observation(
                argv,
                stdout=inventory_bytes(
                    inventory(
                        peer_status=(
                            "started" if self.running_peer else "stopped"
                        )
                    )
                ),
            )
        if argv == shell_control._update_argv(
            self.request, bindings.REQUIRED_SHELL_UUID
        ):
            if self.update_lands:
                write_final_config(
                    self.request.registration_shell_package_path
                    / "config.plist"
                )
            if self.update_disk_drift:
                (
                    self.request.registration_shell_package_path
                    / "Data"
                    / QCOW2_NAME
                ).write_bytes(b"synthetic-disk-drift")
            return observation(
                argv,
                exit_code=self.update_exit_code,
                stdout=(
                    f"{bindings.REQUIRED_SHELL_UUID}\n".encode("ascii")
                    if self.update_exit_code == 0
                    else b""
                ),
                stderr=(
                    b""
                    if self.update_exit_code == 0
                    else b"synthetic update failure\n"
                ),
            )
        raise AssertionError(f"unexpected command: {argv}")


class LinuxL6RegistrationShellMoveTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        self.operator_root = self.root / "VirtualMachines"
        self.operator_root.mkdir(mode=0o755)
        self.default_root = self.root / "UTM-Documents"
        self.default_root.mkdir(mode=0o755)
        self.source = self.operator_root / str(
            case_contract.EXPECTED_START["relative_path"]
        )
        self.default_shell = self.default_root / (
            f"{shell_bindings.REQUIRED_SHELL_NAME}.utm"
        )
        self.external_shell = self.operator_root / (
            f"{shell_bindings.REQUIRED_SHELL_NAME}.utm"
        )
        self.shell_evidence = self.root / "shell-evidence"
        make_source(self.source)
        make_partial_shell(self.default_shell)

        local_evidence = self.source / "local-snapshot.evidence.json"
        local_evidence.write_bytes(b'{"synthetic":"s2"}\n')
        local_evidence.chmod(0o600)
        source_expected = {
            "config_sha256": sha256(self.source / "config.plist"),
            "efi_sha256": sha256(self.source / "Data/efi_vars.fd"),
            "local_evidence_sha256": sha256(local_evidence),
            "qcow2_sha256": sha256(self.source / "Data/source.qcow2"),
        }
        partial_expected = {
            "config_sha256": sha256(self.default_shell / "config.plist"),
            "efi_sha256": sha256(self.default_shell / "Data/efi_vars.fd"),
            "qcow2_name": QCOW2_NAME,
            "qcow2_sha256": sha256(
                self.default_shell / "Data" / QCOW2_NAME
            ),
        }
        self.inventory = inventory()
        self.inventory_sha256 = clone_control.canonical_inventory_sha256(
            self.inventory
        )
        self.patches = (
            mock.patch.dict(case_contract.EXPECTED_START, source_expected),
            mock.patch.object(
                bindings, "DEFAULT_UTM_STORAGE_ROOT", self.default_root
            ),
            mock.patch.object(
                bindings, "REQUIRED_INVENTORY_COUNT", len(self.inventory)
            ),
            mock.patch.object(
                bindings,
                "REQUIRED_INVENTORY_SHA256",
                self.inventory_sha256,
            ),
            mock.patch.dict(bindings.EXPECTED_V4_PARTIAL, partial_expected),
        )
        for patch in self.patches:
            patch.start()

    def tearDown(self) -> None:
        for patch in reversed(self.patches):
            patch.stop()
        self.temporary.cleanup()

    def test_prepare_is_read_only_and_emits_move_ready_permit(self) -> None:
        request = self.prepare_request()
        runner = FakeRunner(request)

        result = control.run_prepare_once(
            request, runner=runner, binding_validator=valid_prepare_binding
        )

        self.assertEqual(result.outcome, "move-ready")
        self.assertEqual(result.inventory_query_invocations, 1)
        self.assertEqual(result.update_invocations, 0)
        self.assertTrue(self.default_shell.exists())
        self.assertFalse(self.external_shell.exists())
        self.assertEqual(runner.calls.count(("utmctl", "list")), 1)
        self.assertFalse(
            any(
                call[0] == shell_control.OSASCRIPT_PATH
                for call in runner.calls
            )
        )
        terminal = read_json(request.output_root / "terminal.json")
        self.assertEqual(
            terminal["next_external_action"],
            "one-utm-ui-move-to-authorized-package-path",
        )
        self.assertEqual(
            manifest_names(request.output_root), bindings.PREPARE_MEMBERS
        )

    def test_prepare_rejects_open_handle_without_move_or_update(self) -> None:
        request = self.prepare_request(attempt_id="prepare-open-handle")
        runner = FakeRunner(request, handles_present=True)

        result = control.run_prepare_once(
            request, runner=runner, binding_validator=valid_prepare_binding
        )

        self.assertEqual(result.outcome, "precondition-rejected")
        self.assertEqual(result.inventory_query_invocations, 0)
        self.assertEqual(result.update_invocations, 0)
        self.assertTrue(self.default_shell.exists())
        self.assertFalse(self.external_shell.exists())

    def test_prepare_rejects_running_inventory(self) -> None:
        request = self.prepare_request(attempt_id="prepare-running-peer")
        runner = FakeRunner(request, running_peer=True)

        result = control.run_prepare_once(
            request, runner=runner, binding_validator=valid_prepare_binding
        )

        self.assertEqual(result.outcome, "precondition-rejected")
        self.assertEqual(result.inventory_query_invocations, 1)
        self.assertTrue(self.default_shell.exists())

    def test_complete_adopts_move_updates_once_and_freezes(self) -> None:
        prepare, prepare_result = self.successful_prepare()
        self.default_shell.rename(self.external_shell)
        request = self.complete_request(
            prepare,
            prepare_result,
            expected_repository_head="b" * 40,
        )
        runner = FakeRunner(request)

        self.assertNotEqual(
            prepare.expected_repository_head,
            request.expected_repository_head,
        )
        self.assertEqual(
            bindings.validate_prepare_evidence(request)["outcome"],
            "move-ready",
        )

        result = control.run_complete_once(
            request, runner=runner, binding_validator=valid_complete_binding
        )

        self.assertEqual(result.outcome, "frozen")
        self.assertEqual(result.inventory_query_invocations, 2)
        self.assertEqual(result.update_invocations, 1)
        self.assertFalse(self.default_shell.exists())
        self.assertEqual(runner.calls.count(("utmctl", "list")), 2)
        self.assertEqual(
            runner.calls.count(
                shell_control._update_argv(
                    request, bindings.REQUIRED_SHELL_UUID
                )
            ),
            1,
        )
        shell = shell_bindings.validate_registration_shell_bundle(
            request, bindings.REQUIRED_SHELL_UUID
        )
        self.assertEqual(shell.network, [])
        self.assertEqual(
            read_json(self.shell_evidence / "registration-shell.json"),
            shell_bindings.expected_shell_evidence(request, shell),
        )
        self.assertEqual(
            manifest_names(request.output_root), bindings.COMPLETE_MEMBERS
        )

    def test_complete_without_ui_move_never_updates(self) -> None:
        prepare, prepare_result = self.successful_prepare()
        request = self.complete_request(
            prepare, prepare_result, attempt_id="complete-without-move"
        )
        runner = FakeRunner(request)

        result = control.run_complete_once(
            request, runner=runner, binding_validator=valid_complete_binding
        )

        self.assertEqual(result.outcome, "state-indeterminate")
        self.assertEqual(result.inventory_query_invocations, 0)
        self.assertEqual(result.update_invocations, 0)
        self.assertEqual(runner.calls, [])
        self.assertTrue(self.default_shell.exists())

    def test_complete_rejects_moved_disk_drift_before_update(self) -> None:
        prepare, prepare_result = self.successful_prepare()
        self.default_shell.rename(self.external_shell)
        (self.external_shell / "Data" / QCOW2_NAME).write_bytes(b"drift")
        request = self.complete_request(
            prepare, prepare_result, attempt_id="complete-preupdate-drift"
        )
        runner = FakeRunner(request)

        result = control.run_complete_once(
            request, runner=runner, binding_validator=valid_complete_binding
        )

        self.assertEqual(result.outcome, "state-indeterminate")
        self.assertEqual(result.update_invocations, 0)
        self.assertEqual(runner.calls, [])
        self.assertFalse(self.shell_evidence.exists())

    def test_complete_update_failure_has_no_cleanup_or_retry(self) -> None:
        prepare, prepare_result = self.successful_prepare()
        self.default_shell.rename(self.external_shell)
        request = self.complete_request(
            prepare, prepare_result, attempt_id="complete-update-failure"
        )
        runner = FakeRunner(
            request, update_lands=False, update_exit_code=1
        )

        result = control.run_complete_once(
            request, runner=runner, binding_validator=valid_complete_binding
        )

        self.assertEqual(result.outcome, "state-indeterminate")
        self.assertEqual(result.update_invocations, 1)
        self.assertEqual(result.inventory_query_invocations, 2)
        self.assertTrue(self.external_shell.exists())
        self.assertFalse(self.shell_evidence.exists())
        terminal = read_json(request.output_root / "terminal.json")
        self.assertEqual(terminal["automatic_delete"], "not-performed")
        self.assertEqual(terminal["automatic_retry"], "not-performed")
        self.assertEqual(terminal["automatic_rollback"], "not-performed")

    def test_missing_authorization_fails_before_evidence_creation(self) -> None:
        request = self.prepare_request(
            attempt_id="prepare-missing-auth",
            authorized_emit_next_external_action_one_utm_ui_move=False,
        )

        with self.assertRaisesRegex(
            control.ShellMoveControlError, "prepare-authorization-required"
        ):
            control.run_prepare_once(request)

        self.assertFalse(request.output_root.exists())

    def successful_prepare(
        self,
    ) -> tuple[control.MovePrepareRequest, control.ShellMoveResult]:
        request = self.prepare_request()
        result = control.run_prepare_once(
            request,
            runner=FakeRunner(request),
            binding_validator=valid_prepare_binding,
        )
        self.assertEqual(result.outcome, "move-ready")
        return request, result

    def prepare_request(
        self, **overrides: object
    ) -> control.MovePrepareRequest:
        attempt_id = str(overrides.get("attempt_id", "prepare-move"))
        values = self.common_values(attempt_id)
        values.update(
            {
                "authorized_upgrade_quiesced_registration_shell_move_prepare": (
                    True
                ),
                "authorized_one_pre_move_inventory_query": True,
                "authorized_emit_next_external_action_one_utm_ui_move": True,
                "authorized_no_move_update_start_clone_delete_retry_guest_or_transaction": (
                    True
                ),
            }
        )
        values.update(overrides)
        return control.MovePrepareRequest(**values)  # type: ignore[arg-type]

    def complete_request(
        self,
        prepare: control.MovePrepareRequest,
        result: control.ShellMoveResult,
        **overrides: object,
    ) -> control.MoveCompleteRequest:
        attempt_id = str(overrides.get("attempt_id", "complete-move"))
        values = self.common_values(attempt_id)
        values.update(
            {
                "prepare_root": prepare.output_root,
                "prepare_manifest_sha256": result.manifest_sha256,
                "update_timeout_seconds": 30,
                "authorized_adopt_one_utm_ui_move_result": True,
                "authorized_one_stopped_configuration_update": True,
                "authorized_two_inventory_queries": True,
                "authorized_no_start_clone_delete_retry_guest_or_transaction": True,
            }
        )
        values.update(overrides)
        return control.MoveCompleteRequest(**values)  # type: ignore[arg-type]

    def common_values(self, attempt_id: str) -> dict[str, object]:
        return {
            "repository_root": Path(__file__).resolve().parents[2],
            "expected_repository_head": "a" * 40,
            "output_root": self.root / attempt_id,
            "attempt_id": attempt_id,
            "prior_v4_control_root": self.root / "frozen-v4-control",
            "prior_v4_manifest_sha256": (
                bindings.REQUIRED_V4_CONTROL_MANIFEST_SHA256
            ),
            "operator_asset_root": self.operator_root,
            "source_snapshot_root": self.source,
            "default_shell_package_path": self.default_shell,
            "registration_shell_package_path": self.external_shell,
            "registration_shell_evidence_root": self.shell_evidence,
            "registration_shell_name": shell_bindings.REQUIRED_SHELL_NAME,
            "registration_shell_uuid": bindings.REQUIRED_SHELL_UUID,
            "expected_vm_count": len(self.inventory),
            "expected_inventory_sha256": self.inventory_sha256,
            "command_timeout_seconds": 5,
        }


def inventory(
    *, peer_status: str = "stopped"
) -> tuple[clone_control.RegisteredVm, ...]:
    return (
        clone_control.RegisteredVm(PEER_UUID, peer_status, "Peer-One"),
        clone_control.RegisteredVm(
            bindings.REQUIRED_SHELL_UUID,
            "stopped",
            shell_bindings.REQUIRED_SHELL_NAME,
        ),
    )


def inventory_bytes(vms: tuple[clone_control.RegisteredVm, ...]) -> bytes:
    lines = ["UUID Status Name"]
    lines.extend(f"{item.uuid} {item.status} {item.name}" for item in vms)
    return ("\n".join(lines) + "\n").encode("utf-8")


def make_source(root: Path) -> None:
    data = root / "Data"
    data.mkdir(mode=0o700, parents=True)
    root.chmod(0o700)
    config = base_config("source.qcow2")
    config["Information"] = {
        "Name": "RadishLex-S2-Source",
        "UUID": "BBBBBBBB-BBBB-4BBB-8BBB-BBBBBBBBBBBB",
    }
    write_plist(root / "config.plist", config)
    (data / "efi_vars.fd").write_bytes(b"immutable-s2-efi")
    (data / "source.qcow2").write_bytes(b"immutable-s2-qcow2")
    chmod_bundle_files(root, "source.qcow2")


def make_partial_shell(root: Path) -> None:
    data = root / "Data"
    data.mkdir(mode=0o755, parents=True)
    root.chmod(0o755)
    config = base_config(QCOW2_NAME)
    config.update(
        {
            "Display": [],
            "Information": {
                "Name": shell_bindings.REQUIRED_SHELL_NAME,
                "Notes": "",
                "UUID": bindings.REQUIRED_SHELL_UUID,
            },
            "Network": [
                {
                    "Hardware": "virtio-net-pci",
                    "Mode": "Shared",
                    "PortForward": [],
                }
            ],
            "Serial": [{"Mode": "Ptty"}],
            "System": {
                "Architecture": "aarch64",
                "CPUCount": 0,
                "MemorySize": 512,
                "Target": "virt",
            },
        }
    )
    write_plist(root / "config.plist", config)
    (data / "efi_vars.fd").write_bytes(b"new-shell-efi")
    (data / QCOW2_NAME).write_bytes(b"new-empty-shell-qcow2")
    chmod_bundle_files(root, QCOW2_NAME)


def write_final_config(path: Path) -> None:
    config = base_config(QCOW2_NAME)
    config.update(
        {
            "Display": [
                {"DynamicResolution": True, "Hardware": "virtio-gpu-pci"}
            ],
            "Information": {
                "Icon": "linux",
                "Name": shell_bindings.REQUIRED_SHELL_NAME,
                "Notes": shell_bindings.REQUIRED_SHELL_NOTES,
                "UUID": bindings.REQUIRED_SHELL_UUID,
            },
            "Network": [],
            "Serial": [],
            "Sharing": {
                "ClipboardSharing": True,
                "DirectoryShareMode": "VirtFS",
            },
            "System": {
                "Architecture": "aarch64",
                "CPUCount": 0,
                "MemorySize": 4096,
                "Target": "virt",
            },
        }
    )
    write_plist(path, config)
    path.chmod(0o644)


def base_config(qcow2_name: str) -> dict[str, object]:
    return {
        "Backend": "QEMU",
        "ConfigurationVersion": 4,
        "Display": [],
        "Drive": [
            {
                "Identifier": qcow2_name.removesuffix(".qcow2"),
                "ImageName": qcow2_name,
                "ImageType": "Disk",
                "Interface": "VirtIO",
                "InterfaceVersion": 1,
                "ReadOnly": False,
            }
        ],
        "Information": {},
        "Network": [],
        "QEMU": {
            "AdditionalArguments": [],
            "Hypervisor": True,
            "UEFIBoot": True,
        },
        "Serial": [],
        "Sharing": {},
        "System": {
            "Architecture": "aarch64",
            "CPUCount": 0,
            "MemorySize": 4096,
            "Target": "virt",
        },
    }


def chmod_bundle_files(root: Path, qcow2_name: str) -> None:
    for path in (
        root / "config.plist",
        root / "Data/efi_vars.fd",
        root / "Data" / qcow2_name,
    ):
        path.chmod(0o644)


def valid_prepare_binding(
    request: control.MovePrepareRequest,
) -> dict[str, object]:
    return {
        "format": bindings.EVIDENCE_FORMAT,
        "phase": "prepare",
        "repository_head": request.expected_repository_head,
        "synthetic": True,
    }


def valid_complete_binding(
    request: control.MoveCompleteRequest,
) -> dict[str, object]:
    return {
        "format": bindings.EVIDENCE_FORMAT,
        "phase": "complete",
        "repository_head": request.expected_repository_head,
        "synthetic": True,
    }


def observation(
    argv: tuple[str, ...],
    *,
    exit_code: int = 0,
    stdout: bytes = b"",
    stderr: bytes = b"",
) -> clone_control.CommandObservation:
    return clone_control.CommandObservation.from_bytes(
        argv,
        exit_code=exit_code,
        stdout=stdout,
        stderr=stderr,
    )


def write_plist(path: Path, value: dict[str, object]) -> None:
    with path.open("wb") as output:
        plistlib.dump(value, output, sort_keys=True)


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def read_json(path: Path) -> dict[str, object]:
    value = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(value, dict):
        raise AssertionError("expected JSON object")
    return value


def manifest_names(root: Path) -> tuple[str, ...]:
    lines = (root / "files.sha256").read_text(encoding="ascii").splitlines()
    return tuple(line[66:] for line in lines)


if __name__ == "__main__":
    unittest.main()
