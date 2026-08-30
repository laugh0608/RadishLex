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
import l6_upgrade_quiesced_registration_shell as control
import l6_upgrade_quiesced_registration_shell_bindings as bindings
import l6_utm_clone_once as clone_control


SHELL_UUID = "AAAAAAAA-AAAA-4AAA-8AAA-AAAAAAAAAAAA"
SOURCE_UUID = "BBBBBBBB-BBBB-4BBB-8BBB-BBBBBBBBBBBB"
PEER_UUID = "11111111-1111-4111-8111-111111111111"
SECOND_PEER_UUID = "22222222-2222-4222-8222-222222222222"
QCOW2_NAME = "FFF05A20-E829-493C-8F40-B40884425A3F.qcow2"


class FakeRunner:
    def __init__(
        self,
        request: control.RegistrationShellRequest,
        *,
        running_peer: bool = False,
        source_handles_present: bool = False,
        create_lands: bool = True,
        create_exit_code: int = 0,
        create_stderr: bytes = b"",
        create_stdout: bytes | None = None,
        shell_network: object = None,
        terminal_inventory_drift: bool = False,
    ) -> None:
        self.request = request
        self.running_peer = running_peer
        self.source_handles_present = source_handles_present
        self.create_lands = create_lands
        self.create_exit_code = create_exit_code
        self.create_stderr = create_stderr
        self.create_stdout = (
            f"{SHELL_UUID}\n".encode("ascii")
            if create_stdout is None
            else create_stdout
        )
        self.shell_network = [] if shell_network is None else shell_network
        self.terminal_inventory_drift = terminal_inventory_drift
        self.calls: list[tuple[str, ...]] = []
        self.list_calls = 0

    def run(
        self, argv: tuple[str, ...], timeout_seconds: int
    ) -> clone_control.CommandObservation:
        del timeout_seconds
        self.calls.append(argv)
        if argv[0] == control.LSOF_PATH:
            if self.source_handles_present:
                self.source_handles_present = False
                return observation(
                    argv,
                    stdout=b"p123\ncQEMULauncher\nf4\ntREG\n",
                )
            return observation(argv, exit_code=1)
        if argv == ("utmctl", "list"):
            self.list_calls += 1
            if self.list_calls == 1 or not self.request.registration_shell_package_path.exists():
                return observation(
                    argv,
                    stdout=baseline_list(
                        peer_status=(
                            "started" if self.running_peer else "stopped"
                        )
                    ),
                )
            return observation(
                argv,
                stdout=created_list(
                    peer_status=(
                        "started"
                        if self.terminal_inventory_drift
                        and self.list_calls >= 3
                        else "stopped"
                    )
                ),
            )
        if argv == control._create_argv(self.request):
            if self.create_lands:
                make_bundle(
                    self.request.registration_shell_package_path,
                    mode=0o755,
                    name=self.request.registration_shell_name,
                    vm_uuid=SHELL_UUID,
                    network=self.shell_network,
                    notes=bindings.REQUIRED_SHELL_NOTES,
                    registration_shell=True,
                )
            return observation(
                argv,
                exit_code=self.create_exit_code,
                stdout=self.create_stdout,
                stderr=self.create_stderr,
            )
        raise AssertionError(f"unexpected command: {argv}")


class LinuxL6UpgradeQuiescedRegistrationShellTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        self.operator_asset_root = self.root / "VirtualMachines"
        self.operator_asset_root.mkdir(mode=0o755)
        self.source = self.operator_asset_root / str(
            case_contract.EXPECTED_START["relative_path"]
        )
        self.shell = self.operator_asset_root / (
            f"{bindings.REQUIRED_SHELL_NAME}.utm"
        )
        self.shell_evidence = self.root / "registration-shell-evidence"
        make_bundle(
            self.source,
            mode=0o700,
            name="RadishLex-S2-Source",
            vm_uuid=SOURCE_UUID,
            network=[{"Mode": "Emulated"}],
            notes="synthetic immutable S2",
            registration_shell=False,
        )
        local_evidence = self.source / "local-snapshot.evidence.json"
        local_evidence.write_bytes(b'{"synthetic":"s2"}\n')
        local_evidence.chmod(0o600)
        expected = {
            "config_sha256": sha256(self.source / "config.plist"),
            "efi_sha256": sha256(self.source / "Data/efi_vars.fd"),
            "local_evidence_sha256": sha256(local_evidence),
            "qcow2_sha256": sha256(
                self.source / "Data" / QCOW2_NAME
            ),
        }
        self.expected_patch = mock.patch.dict(
            case_contract.EXPECTED_START, expected
        )
        self.expected_patch.start()

    def tearDown(self) -> None:
        self.expected_patch.stop()
        self.temporary.cleanup()

    def test_frozen_shell_is_create_new_stopped_and_single_member(self) -> None:
        request = self.request()
        runner = FakeRunner(request)

        result = control.run_registration_shell_once(
            request, runner=runner, binding_validator=valid_binding
        )

        self.assertEqual(result.outcome, "frozen")
        self.assertEqual(result.exit_code, control.EXIT_FROZEN)
        self.assertEqual(result.create_invocations, 1)
        self.assertEqual(result.registration_shell_uuid, SHELL_UUID)
        self.assertEqual(
            runner.calls.count(control._create_argv(request)), 1
        )
        self.assertEqual(runner.calls.count(("utmctl", "list")), 3)
        self.assertFalse(
            any(
                call[:2]
                in {
                    ("utmctl", "clone"),
                    ("utmctl", "delete"),
                    ("utmctl", "exec"),
                    ("utmctl", "start"),
                }
                for call in runner.calls
            )
        )
        shell = bindings.validate_registration_shell_bundle(
            request, SHELL_UUID
        )
        self.assertEqual(shell.network, [])
        self.assertEqual(
            read_json(self.shell_evidence / "registration-shell.json"),
            bindings.expected_shell_evidence(request, shell),
        )
        manifest_lines = (
            self.shell_evidence / "files.sha256"
        ).read_text(encoding="ascii").splitlines()
        self.assertEqual(len(manifest_lines), 1)
        self.assertTrue(
            manifest_lines[0].endswith("  registration-shell.json")
        )
        self.assertEqual(
            clone_control._verify_sha256_manifest(
                self.shell_evidence,
                self.shell_evidence / "files.sha256",
            ),
            1,
        )
        terminal = read_json(request.output_root / "terminal.json")
        self.assertEqual(terminal["outcome"], "frozen")
        self.assertEqual(terminal["automatic_start"], "not-performed")
        self.assertEqual(terminal["clone_invocations"], 0)
        self.assertEqual(terminal["guest_exec_invocations"], 0)
        self.assertEqual(terminal["transaction"], "not-performed")
        assert_manifest_valid(request.output_root)

    def test_running_peer_rejects_before_create(self) -> None:
        request = self.request(attempt_id="running-peer")
        runner = FakeRunner(request, running_peer=True)

        result = control.run_registration_shell_once(
            request, runner=runner, binding_validator=valid_binding
        )

        self.assertEqual(result.outcome, "precondition-rejected")
        self.assertEqual(result.create_invocations, 0)
        self.assertNotIn(control._create_argv(request), runner.calls)
        self.assertFalse(self.shell.exists())

    def test_source_handle_rejects_before_create(self) -> None:
        request = self.request(attempt_id="source-handle")
        runner = FakeRunner(request, source_handles_present=True)

        result = control.run_registration_shell_once(
            request, runner=runner, binding_validator=valid_binding
        )

        self.assertEqual(result.outcome, "precondition-rejected")
        self.assertEqual(result.create_invocations, 0)
        self.assertNotIn(control._create_argv(request), runner.calls)

    def test_existing_shell_package_rejects_without_host_command(self) -> None:
        request = self.request(attempt_id="existing-package")
        make_bundle(
            self.shell,
            mode=0o755,
            name=bindings.REQUIRED_SHELL_NAME,
            vm_uuid=SHELL_UUID,
            network=[],
            notes=bindings.REQUIRED_SHELL_NOTES,
            registration_shell=True,
        )
        runner = FakeRunner(request)

        result = control.run_registration_shell_once(
            request, runner=runner, binding_validator=valid_binding
        )

        self.assertEqual(result.outcome, "precondition-rejected")
        self.assertEqual(runner.calls, [])

    def test_create_failure_with_no_landing_is_failed_closed_absent(self) -> None:
        request = self.request(attempt_id="create-absent")
        runner = FakeRunner(
            request,
            create_lands=False,
            create_exit_code=1,
            create_stdout=b"",
            create_stderr=b"synthetic create failure\n",
        )

        result = control.run_registration_shell_once(
            request, runner=runner, binding_validator=valid_binding
        )

        self.assertEqual(result.outcome, "failed-closed-absent")
        self.assertEqual(result.create_invocations, 1)
        self.assertFalse(self.shell.exists())
        self.assertFalse(self.shell_evidence.exists())

    def test_networked_shell_landing_is_state_indeterminate(self) -> None:
        request = self.request(attempt_id="network-drift")
        runner = FakeRunner(
            request, shell_network=[{"Mode": "Emulated"}]
        )

        result = control.run_registration_shell_once(
            request, runner=runner, binding_validator=valid_binding
        )

        self.assertEqual(result.outcome, "state-indeterminate")
        self.assertTrue(self.shell.exists())
        self.assertFalse(self.shell_evidence.exists())
        self.assertFalse(
            any(call[:2] == ("utmctl", "delete") for call in runner.calls)
        )

    def test_invalid_uuid_output_after_landing_is_state_indeterminate(self) -> None:
        request = self.request(attempt_id="uuid-output-drift")
        runner = FakeRunner(request, create_stdout=b"not-a-uuid\n")

        result = control.run_registration_shell_once(
            request, runner=runner, binding_validator=valid_binding
        )

        self.assertEqual(result.outcome, "state-indeterminate")
        self.assertTrue(self.shell.exists())
        self.assertFalse(self.shell_evidence.exists())

    def test_terminal_inventory_drift_does_not_freeze_shell(self) -> None:
        request = self.request(attempt_id="terminal-drift")
        runner = FakeRunner(request, terminal_inventory_drift=True)

        result = control.run_registration_shell_once(
            request, runner=runner, binding_validator=valid_binding
        )

        self.assertEqual(result.outcome, "state-indeterminate")
        self.assertFalse(self.shell_evidence.exists())

    def test_missing_authorization_fails_before_output_creation(self) -> None:
        request = self.request(
            attempt_id="missing-authorization",
            authorized_one_create_new_registration_only_shell=False,
        )

        with self.assertRaisesRegex(
            control.RegistrationShellError,
            "one-create-new-registration-only-shell-authorization-required",
        ):
            control.run_registration_shell_once(request)

        self.assertFalse(request.output_root.exists())

    def test_create_transport_source_is_exact_and_has_no_start(self) -> None:
        source = (
            Path(__file__).with_name(
                "l6_upgrade_quiesced_registration_shell.applescript"
            )
        ).read_bytes()

        self.assertEqual(source, bindings.CREATE_SOURCE)
        self.assertIn(b"make new virtual machine", source)
        self.assertIn(
            b"configuration:{class:qemu configuration, name:shellName",
            source,
        )
        self.assertIn(b"network interfaces:{}", source)
        self.assertNotIn(b"set shellConfiguration", source)
        self.assertNotIn(b"start shellMachine", source)
        self.assertNotIn(b"duplicate", source)

    def request(
        self, **overrides: object
    ) -> control.RegistrationShellRequest:
        attempt_id = str(overrides.get("attempt_id", "synthetic-shell"))
        baseline = baseline_vms()
        values: dict[str, object] = {
            "repository_root": Path(__file__).resolve().parents[2],
            "expected_repository_head": "a" * 40,
            "output_root": self.root / f"control-{attempt_id}",
            "attempt_id": attempt_id,
            "operator_asset_root": self.operator_asset_root,
            "source_snapshot_root": self.source,
            "registration_shell_evidence_root": self.shell_evidence,
            "registration_shell_name": bindings.REQUIRED_SHELL_NAME,
            "registration_shell_package_path": self.shell,
            "expected_vm_count": len(baseline),
            "expected_precreate_inventory_sha256": (
                clone_control.canonical_inventory_sha256(baseline)
            ),
            "create_timeout_seconds": 30,
            "command_timeout_seconds": 5,
            "authorized_upgrade_quiesced_registration_shell": True,
            "authorized_one_create_new_registration_only_shell": True,
            "authorized_no_clone_start_guest_delete_retry_or_transaction": (
                True
            ),
        }
        values.update(overrides)
        return control.RegistrationShellRequest(**values)  # type: ignore[arg-type]


def baseline_vms(
    *, peer_status: str = "stopped"
) -> tuple[clone_control.RegisteredVm, ...]:
    return (
        clone_control.RegisteredVm(PEER_UUID, peer_status, "Peer-One"),
        clone_control.RegisteredVm(
            SECOND_PEER_UUID, "stopped", "Peer-Two"
        ),
    )


def baseline_list(*, peer_status: str = "stopped") -> bytes:
    return inventory_bytes(baseline_vms(peer_status=peer_status))


def created_list(*, peer_status: str = "stopped") -> bytes:
    return inventory_bytes(
        (
            *baseline_vms(peer_status=peer_status),
            clone_control.RegisteredVm(
                SHELL_UUID, "stopped", bindings.REQUIRED_SHELL_NAME
            ),
        )
    )


def inventory_bytes(
    vms: tuple[clone_control.RegisteredVm, ...],
) -> bytes:
    lines = ["UUID Status Name"]
    lines.extend(f"{item.uuid} {item.status} {item.name}" for item in vms)
    return ("\n".join(lines) + "\n").encode("utf-8")


def make_bundle(
    root: Path,
    *,
    mode: int,
    name: str,
    vm_uuid: str,
    network: object,
    notes: str,
    registration_shell: bool,
) -> None:
    data = root / "Data"
    data.mkdir(mode=mode, parents=True)
    root.chmod(mode)
    data.chmod(mode)
    config = {
        "Backend": "QEMU",
        "ConfigurationVersion": 4,
        "Display": [
            {
                "DynamicResolution": True,
                "Hardware": "virtio-gpu-pci",
            }
        ],
        "Drive": [
            {
                "Identifier": QCOW2_NAME.removesuffix(".qcow2"),
                "ImageName": QCOW2_NAME,
                "ImageType": "Disk",
                "Interface": "VirtIO",
                "InterfaceVersion": 1,
                "ReadOnly": False,
            }
        ],
        "Information": {
            "Icon": "linux",
            "Name": name,
            "Notes": notes,
            "UUID": vm_uuid,
        },
        "Network": network,
        "QEMU": {
            "AdditionalArguments": [],
            "Hypervisor": True,
            "UEFIBoot": True,
        },
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
    write_plist(root / "config.plist", config)
    (data / "efi_vars.fd").write_bytes(
        b"new-shell-efi" if registration_shell else b"immutable-s2-efi"
    )
    (data / QCOW2_NAME).write_bytes(
        b"new-empty-shell-qcow2"
        if registration_shell
        else b"immutable-s2-qcow2"
    )
    for path in (
        root / "config.plist",
        data / "efi_vars.fd",
        data / QCOW2_NAME,
    ):
        path.chmod(0o644)


def write_plist(path: Path, value: dict[str, object]) -> None:
    with path.open("wb") as output:
        plistlib.dump(value, output, sort_keys=True)


def valid_binding(
    request: control.RegistrationShellRequest,
) -> dict[str, object]:
    return {
        "binding_control_sha256": "b" * 64,
        "case_contract": "validated",
        "control_sha256": "c" * 64,
        "create_transport_sha256": "d" * 64,
        "expected_precreate_inventory_sha256": (
            request.expected_precreate_inventory_sha256
        ),
        "format": control.EVIDENCE_FORMAT,
        "repository_clean": True,
        "repository_head": request.expected_repository_head,
        "source_snapshot": {"synthetic": True},
        "utm_build": bindings.EXPECTED_UTM_BUILD,
        "utm_bundle_id": bindings.EXPECTED_UTM_BUNDLE_ID,
        "utm_info_plist_sha256": "e" * 64,
        "utm_sdef_sha256": "f" * 64,
        "utm_version": bindings.EXPECTED_UTM_VERSION,
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


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def read_json(path: Path) -> dict[str, object]:
    value = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(value, dict):
        raise AssertionError("expected JSON object")
    return value


def assert_manifest_valid(root: Path) -> None:
    manifest = root / "files.sha256"
    lines = manifest.read_text(encoding="ascii").splitlines()
    self_entries = [line for line in lines if line.endswith("  files.sha256")]
    if self_entries:
        raise AssertionError("manifest must not include itself")
    for line in lines:
        digest, name = line.split("  ", maxsplit=1)
        if sha256(root / name) != digest:
            raise AssertionError(f"manifest mismatch: {name}")


if __name__ == "__main__":
    unittest.main()
