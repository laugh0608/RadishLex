#!/usr/bin/env python3
from __future__ import annotations

import hashlib
import json
import os
import plistlib
import shutil
import tempfile
import unittest
from dataclasses import replace
from pathlib import Path
from unittest import mock

import l6_upgrade_quiesced_case_contract as case_contract
import l6_upgrade_quiesced_clone_bindings as clone_bindings
import l6_upgrade_quiesced_clone_front_door as front_door
import l6_utm_clone_once as clone_control


SHELL_UUID = "AAAAAAAA-AAAA-4AAA-8AAA-AAAAAAAAAAAA"
SHELL_NAME = "RadishLex-Dedicated-Upgrade-Quiesced-Shell"
SOURCE_UUID = "BBBBBBBB-BBBB-4BBB-8BBB-BBBBBBBBBBBB"
TARGET_UUID = "CCCCCCCC-CCCC-4CCC-8CCC-CCCCCCCCCCCC"
TARGET_NAME = "RadishLex-Debian13-ARM64-L6-d75818f-upgrade-quiesced"
PEER_UUID = "11111111-1111-4111-8111-111111111111"
QCOW2_NAME = "FFF05A20-E829-493C-8F40-B40884425A3F.qcow2"


class FakeRunner:
    def __init__(
        self,
        request: front_door.CloneFrontDoorRequest,
        *,
        running_peer: bool = False,
        clone_lands: bool = True,
        clone_stderr: bytes = b"",
        handles_present: bool = False,
        target_config_drift: bool = False,
        fail_qcow2_replace: bool = False,
        terminal_inventory_drift: bool = False,
    ) -> None:
        self.request = request
        self.running_peer = running_peer
        self.clone_lands = clone_lands
        self.clone_stderr = clone_stderr
        self.handles_present = handles_present
        self.target_config_drift = target_config_drift
        self.fail_qcow2_replace = fail_qcow2_replace
        self.terminal_inventory_drift = terminal_inventory_drift
        self.calls: list[tuple[str, ...]] = []
        self.list_calls = 0

    def run(
        self, argv: tuple[str, ...], timeout_seconds: int
    ) -> clone_control.CommandObservation:
        del timeout_seconds
        self.calls.append(argv)
        if argv[0] == "/usr/sbin/lsof":
            if self.handles_present:
                self.handles_present = False
                return observation(
                    argv,
                    stdout=b"p123\ncQEMULauncher\nf4\ntREG\nntarget\n",
                )
            return observation(argv, exit_code=1)
        if argv == ("utmctl", "list"):
            self.list_calls += 1
            if self.list_calls == 1 or not self.clone_lands:
                return observation(
                    argv,
                    stdout=preclone_list(
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
        if argv == front_door._clone_argv(self.request):
            if self.clone_lands:
                self._create_target()
            return observation(argv, stderr=self.clone_stderr)
        if argv[:2] == ("/bin/cp", "-c"):
            shutil.copy2(argv[2], argv[3])
            return observation(argv)
        if argv[:2] == ("/bin/mv", "-f"):
            if self.fail_qcow2_replace and argv[2].endswith(
                f".{self.request.attempt_id}.incoming"
            ) and QCOW2_NAME in argv[2]:
                return observation(argv, exit_code=1, stderr=b"synthetic\n")
            os.replace(argv[2], argv[3])
            return observation(argv)
        raise AssertionError(f"unexpected command: {argv}")

    def _create_target(self) -> None:
        shutil.copytree(
            self.request.registration_shell_package_path,
            self.request.target_package_path,
        )
        config_path = self.request.target_package_path / "config.plist"
        config = plistlib.loads(config_path.read_bytes())
        config["Information"]["Name"] = self.request.target_name
        config["Information"]["UUID"] = TARGET_UUID
        if self.target_config_drift:
            config["Sharing"]["ClipboardSharing"] = False
        write_plist(config_path, config)


class LinuxL6UpgradeQuiescedCloneFrontDoorTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        self.operator_asset_root = self.root / "assets"
        self.utm_documents_root = self.root / "utm-documents"
        self.utm_documents_root.mkdir(mode=0o700)
        self.retirement_root = self.operator_asset_root / str(
            case_contract.EXPECTED_CLONE_FRONT_DOOR[
                "preclone_baseline"
            ]["delete_evidence_relative_path"]
        )
        self.source = self.operator_asset_root / str(
            case_contract.EXPECTED_START["relative_path"]
        )
        self.packages = self.root / "packages"
        self.packages.mkdir(mode=0o755, parents=True)
        self.shell = self.packages / f"{SHELL_NAME}.utm"
        self.target = self.packages / f"{TARGET_NAME}.utm"
        self.shell_evidence = self.root / "shell-evidence"
        self.shell_evidence.mkdir(mode=0o700)
        make_bundle(
            self.source,
            mode=0o700,
            name="RadishLex-S2-Source",
            vm_uuid=SOURCE_UUID,
            network=[{"Mode": "Emulated"}],
            efi=b"immutable-s2-efi",
            qcow2=b"immutable-s2-qcow2",
        )
        local_evidence = self.source / "local-snapshot.evidence.json"
        local_evidence.write_bytes(b'{"synthetic":"s2"}\n')
        local_evidence.chmod(0o600)
        make_bundle(
            self.shell,
            mode=0o755,
            name=SHELL_NAME,
            vm_uuid=SHELL_UUID,
            network=[],
            efi=b"dedicated-shell-efi",
            qcow2=b"dedicated-shell-qcow2",
        )
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
        self.baseline_patch = mock.patch.dict(
            case_contract.EXPECTED_CLONE_FRONT_DOOR[
                "preclone_baseline"
            ],
            {
                "delete_manifest_sha256": "e" * 64,
                "inventory_sha256": inventory_sha256(preclone_list()),
                "registered_vm_count": 2,
            },
        )
        self.baseline_patch.start()

    def tearDown(self) -> None:
        self.baseline_patch.stop()
        self.expected_patch.stop()
        self.temporary.cleanup()

    def test_prepared_requires_one_clone_and_two_s2_replacements(self) -> None:
        request = self.request()
        runner = FakeRunner(request)

        result = front_door.run_clone_front_door(
            request, runner=runner, binding_validator=valid_binding
        )

        self.assertEqual(result.outcome, "prepared")
        self.assertEqual(result.exit_code, front_door.EXIT_PREPARED)
        self.assertEqual(result.clone_invocations, 1)
        self.assertEqual(result.replacement_count, 2)
        self.assertEqual(result.target_uuid, TARGET_UUID)
        self.assertEqual(
            sha256(self.target / "Data/efi_vars.fd"),
            sha256(self.source / "Data/efi_vars.fd"),
        )
        self.assertEqual(
            sha256(self.target / "Data" / QCOW2_NAME),
            sha256(self.source / "Data" / QCOW2_NAME),
        )
        target_config = plistlib.loads(
            (self.target / "config.plist").read_bytes()
        )
        self.assertEqual(target_config["Network"], [])
        self.assertEqual(target_config["Information"]["UUID"], TARGET_UUID)
        self.assertEqual(runner.calls.count(front_door._clone_argv(request)), 1)
        self.assertFalse(
            any(
                call[:2]
                in {
                    ("utmctl", "start"),
                    ("utmctl", "delete"),
                    ("utmctl", "exec"),
                }
                for call in runner.calls
            )
        )
        assert_manifest_valid(request.output_root)

    def test_clone_without_landing_fails_closed_absent(self) -> None:
        request = self.request(attempt_id="clone-absent")
        runner = FakeRunner(
            request,
            clone_lands=False,
            clone_stderr=b"OSStatus error -1712\n",
        )

        result = front_door.run_clone_front_door(
            request, runner=runner, binding_validator=valid_binding
        )

        self.assertEqual(result.outcome, "failed-closed-absent")
        self.assertEqual(result.replacement_count, 0)
        self.assertFalse(self.target.exists())
        self.assertFalse(any(call[0] == "/bin/cp" for call in runner.calls))

    def test_running_peer_rejects_before_clone(self) -> None:
        request = self.request(attempt_id="running-peer")
        runner = FakeRunner(request, running_peer=True)

        result = front_door.run_clone_front_door(
            request, runner=runner, binding_validator=valid_binding
        )

        self.assertEqual(result.outcome, "precondition-rejected")
        self.assertEqual(result.clone_invocations, 0)
        self.assertNotIn(front_door._clone_argv(request), runner.calls)

    def test_source_drift_rejects_before_any_host_command(self) -> None:
        request = self.request(attempt_id="source-drift")
        (self.source / "Data/efi_vars.fd").write_bytes(b"drift")
        runner = FakeRunner(request)

        result = front_door.run_clone_front_door(
            request, runner=runner, binding_validator=valid_binding
        )

        self.assertEqual(result.outcome, "precondition-rejected")
        self.assertEqual(runner.calls, [])

    def test_open_handle_rejects_before_clone(self) -> None:
        request = self.request(attempt_id="open-handle")
        runner = FakeRunner(request, handles_present=True)

        result = front_door.run_clone_front_door(
            request, runner=runner, binding_validator=valid_binding
        )

        self.assertEqual(result.outcome, "precondition-rejected")
        self.assertEqual(result.clone_invocations, 0)

    def test_target_config_drift_is_indeterminate_without_replacement(self) -> None:
        request = self.request(attempt_id="config-drift")
        runner = FakeRunner(request, target_config_drift=True)

        result = front_door.run_clone_front_door(
            request, runner=runner, binding_validator=valid_binding
        )

        self.assertEqual(result.outcome, "state-indeterminate")
        self.assertEqual(result.clone_invocations, 1)
        self.assertEqual(result.replacement_count, 0)
        self.assertFalse(any(call[0] == "/bin/cp" for call in runner.calls))

    def test_second_replace_failure_preserves_partial_target(self) -> None:
        request = self.request(attempt_id="partial-target")
        runner = FakeRunner(request, fail_qcow2_replace=True)

        result = front_door.run_clone_front_door(
            request, runner=runner, binding_validator=valid_binding
        )

        self.assertEqual(result.outcome, "state-indeterminate")
        self.assertEqual(result.replacement_count, 1)
        self.assertEqual(
            sha256(self.target / "Data/efi_vars.fd"),
            sha256(self.source / "Data/efi_vars.fd"),
        )
        self.assertNotEqual(
            sha256(self.target / "Data" / QCOW2_NAME),
            sha256(self.source / "Data" / QCOW2_NAME),
        )
        terminal = read_json(request.output_root / "terminal.json")
        self.assertEqual(terminal["automatic_rollback"], "not-performed")
        self.assertEqual(terminal["automatic_delete"], "not-performed")

    def test_terminal_inventory_drift_is_indeterminate_after_materialization(
        self,
    ) -> None:
        request = self.request(attempt_id="terminal-drift")
        runner = FakeRunner(request, terminal_inventory_drift=True)

        result = front_door.run_clone_front_door(
            request, runner=runner, binding_validator=valid_binding
        )

        self.assertEqual(result.outcome, "state-indeterminate")
        self.assertEqual(result.replacement_count, 2)

    def test_authorizations_and_dedicated_package_parent_are_required(
        self,
    ) -> None:
        for override in (
            {"authorized_upgrade_quiesced_clone": False},
            {"authorized_dedicated_registration_shell": False},
            {
                "authorized_no_automatic_retry_delete_start_or_rollback": (
                    False
                )
            },
        ):
            with self.subTest(override=override):
                with self.assertRaises(front_door.CloneFrontDoorError):
                    self.request(**override).validate()

        other_parent = self.root / "other"
        other_parent.mkdir()
        with self.assertRaises(front_door.CloneFrontDoorError):
            self.request(
                target_package_path=other_parent / f"{TARGET_NAME}.utm"
            ).validate()

        for override in (
            {"expected_vm_count": 8},
            {"expected_preclone_inventory_sha256": "0" * 64},
            {"asset_retirement_delete_manifest_sha256": "0" * 64},
            {
                "asset_retirement_delete_root": (
                    self.operator_asset_root / "unbound-delete-root"
                )
            },
        ):
            with self.subTest(retirement_override=override):
                with self.assertRaises(front_door.CloneFrontDoorError):
                    self.request(**override).validate()

    def test_dedicated_shell_evidence_binds_role_and_disk_identity(self) -> None:
        request = self.request(attempt_id="shell-binding")
        request = self.write_retirement_evidence(request)
        shell = clone_bindings._read_bundle(self.shell, root_mode=0o755)
        evidence = {
            "case_profile": "debian13-arm64-upgrade-quiesced-crash-v1",
            "config_sha256": shell.config_sha256,
            "dedicated_to_case": True,
            "efi_sha256": shell.efi_sha256,
            "format": clone_bindings.SHELL_EVIDENCE_FORMAT,
            "guest_state_reuse": False,
            "network": [],
            "package_path_sha256": clone_bindings.sha256_text(
                str(self.shell)
            ),
            "qcow2_name": QCOW2_NAME,
            "qcow2_sha256": shell.qcow2_sha256,
            "source_terminal_reuse": False,
            "vm_name": SHELL_NAME,
            "vm_uuid": SHELL_UUID,
        }
        bound_request = self.write_shell_evidence(request, evidence)
        git_output = lambda _root, arguments: (  # noqa: E731
            b"a" * 40 + b"\n" if arguments[0] == "rev-parse" else b""
        )
        with (
            mock.patch.object(clone_bindings, "_run_git", side_effect=git_output),
            mock.patch.object(
                clone_bindings.case_contract,
                "validate_repository_contract",
            ),
            mock.patch.object(
                clone_bindings,
                "_validate_control_identity",
                return_value="d" * 64,
            ),
        ):
            result = clone_bindings.validate_clone_front_door_bindings(
                bound_request
            )
            self.assertTrue(result["dedicated_registration_shell"])
            self.assertEqual(
                result["asset_retirement_delete_entries_verified"], 18
            )
            self.assertEqual(result["post_retirement_vm_count"], 2)

            resurrected = (
                self.utm_documents_root
                / "RadishLex-Debian13-ARM64-L6-1ebbdab.utm"
            )
            resurrected.mkdir()
            with self.assertRaises(clone_bindings.CloneBindingError):
                clone_bindings.validate_clone_front_door_bindings(
                    bound_request
                )
            resurrected.rmdir()

            evidence["source_terminal_reuse"] = True
            drifted_request = self.write_shell_evidence(
                bound_request, evidence, rewrite=True
            )
            with self.assertRaises(clone_bindings.CloneBindingError):
                clone_bindings.validate_clone_front_door_bindings(
                    drifted_request
                )

            terminal_path = self.retirement_root / "terminal.json"
            terminal = json.loads(terminal_path.read_text(encoding="utf-8"))
            terminal["outcome"] = "prepared"
            terminal_path.write_text(
                json.dumps(terminal, sort_keys=True) + "\n",
                encoding="utf-8",
            )
            terminal_path.chmod(0o600)
            semantic_request = self.refresh_retirement_manifest(
                bound_request
            )
            with self.assertRaisesRegex(
                clone_bindings.CloneBindingError, "terminal"
            ):
                clone_bindings.validate_clone_front_door_bindings(
                    semantic_request
                )

    def request(
        self, **overrides: object
    ) -> front_door.CloneFrontDoorRequest:
        values: dict[str, object] = {
            "repository_root": self.root / "repo",
            "expected_repository_head": "a" * 40,
            "output_root": self.root
            / f"evidence-{overrides.get('attempt_id', 'prepared')}",
            "attempt_id": "upgrade-quiesced-synthetic",
            "operator_asset_root": self.operator_asset_root,
            "utm_documents_root": self.utm_documents_root,
            "asset_retirement_delete_root": self.retirement_root,
            "asset_retirement_delete_manifest_sha256": "e" * 64,
            "source_snapshot_root": self.source,
            "registration_shell_evidence_root": self.shell_evidence,
            "registration_shell_manifest_sha256": "b" * 64,
            "registration_shell_uuid": SHELL_UUID,
            "registration_shell_name": SHELL_NAME,
            "registration_shell_package_path": self.shell,
            "target_name": TARGET_NAME,
            "target_package_path": self.target,
            "expected_vm_count": 2,
            "expected_preclone_inventory_sha256": inventory_sha256(
                preclone_list()
            ),
            "clone_timeout_seconds": 120,
            "command_timeout_seconds": 15,
            "authorized_upgrade_quiesced_clone": True,
            "authorized_dedicated_registration_shell": True,
            "authorized_no_automatic_retry_delete_start_or_rollback": True,
        }
        values.update(overrides)
        return front_door.CloneFrontDoorRequest(**values)  # type: ignore[arg-type]

    def write_retirement_evidence(
        self, request: front_door.CloneFrontDoorRequest
    ) -> front_door.CloneFrontDoorRequest:
        self.retirement_root.mkdir(mode=0o700)
        values: dict[str, object] = {
            name: {"synthetic": name}
            for name in clone_bindings.ASSET_RETIREMENT_DELETE_FILES
        }
        baseline = case_contract.EXPECTED_CLONE_FRONT_DOOR[
            "preclone_baseline"
        ]
        values["request.json"] = {
            "batch_id": baseline["delete_batch_id"],
            "expected_repository_head": baseline[
                "delete_repository_head"
            ],
            "format": clone_bindings.ASSET_RETIREMENT_DELETE_FORMAT,
            "operator_asset_root_sha256": clone_bindings.sha256_text(
                str(self.operator_asset_root)
            ),
            "utm_documents_root_sha256": clone_bindings.sha256_text(
                str(self.utm_documents_root)
            ),
        }
        values["terminal.json"] = clone_bindings.EXPECTED_DELETE_TERMINAL
        values["delete-03-post-list.json"] = observation(
            ("utmctl", "list"), stdout=preclone_list()
        ).as_json()
        for index in range(1, 4):
            values[f"delete-{index:02d}-package.json"] = {
                "format": clone_bindings.ASSET_RETIREMENT_DELETE_FORMAT,
                "state": "absent",
            }
        for name in clone_bindings.ASSET_RETIREMENT_DELETE_FILES:
            path = self.retirement_root / name
            path.write_text(
                json.dumps(values[name], sort_keys=True) + "\n",
                encoding="utf-8",
            )
            path.chmod(0o600)
        return self.refresh_retirement_manifest(request)

    def refresh_retirement_manifest(
        self, request: front_door.CloneFrontDoorRequest
    ) -> front_door.CloneFrontDoorRequest:
        manifest = self.retirement_root / "files.sha256"
        manifest.write_text(
            "".join(
                f"{sha256(self.retirement_root / name)}  {name}\n"
                for name in clone_bindings.ASSET_RETIREMENT_DELETE_FILES
            ),
            encoding="ascii",
        )
        manifest.chmod(0o600)
        manifest_sha256 = sha256(manifest)
        baseline = case_contract.EXPECTED_CLONE_FRONT_DOOR[
            "preclone_baseline"
        ]
        baseline["delete_manifest_sha256"] = manifest_sha256
        return replace(
            request,
            asset_retirement_delete_manifest_sha256=manifest_sha256,
        )

    def write_shell_evidence(
        self,
        request: front_door.CloneFrontDoorRequest,
        evidence: dict[str, object],
        *,
        rewrite: bool = False,
    ) -> front_door.CloneFrontDoorRequest:
        evidence_path = self.shell_evidence / "registration-shell.json"
        if rewrite:
            evidence_path.unlink()
            (self.shell_evidence / "files.sha256").unlink()
        evidence_path.write_text(
            json.dumps(evidence, sort_keys=True) + "\n", encoding="utf-8"
        )
        evidence_path.chmod(0o600)
        manifest = self.shell_evidence / "files.sha256"
        manifest.write_text(
            f"{sha256(evidence_path)}  registration-shell.json\n",
            encoding="ascii",
        )
        manifest.chmod(0o600)
        return replace(
            request,
            registration_shell_manifest_sha256=sha256(manifest),
        )


def make_bundle(
    root: Path,
    *,
    mode: int,
    name: str,
    vm_uuid: str,
    network: list[dict[str, object]],
    efi: bytes,
    qcow2: bytes,
) -> None:
    data = root / "Data"
    data.mkdir(parents=True, mode=mode)
    root.chmod(mode)
    data.chmod(mode)
    config = {
        "Backend": "QEMU",
        "ConfigurationVersion": 4,
        "Drive": [
            {
                "ImageName": QCOW2_NAME,
                "ImageType": "Disk",
                "Interface": "VirtIO",
            }
        ],
        "Information": {"Name": name, "UUID": vm_uuid},
        "Network": network,
        "Sharing": {"ClipboardSharing": True},
        "System": {"Architecture": "aarch64", "Target": "virt"},
    }
    write_plist(root / "config.plist", config)
    (data / "efi_vars.fd").write_bytes(efi)
    (data / QCOW2_NAME).write_bytes(qcow2)
    (data / "efi_vars.fd").chmod(0o644)
    (data / QCOW2_NAME).chmod(0o644)


def write_plist(path: Path, value: object) -> None:
    path.write_bytes(
        plistlib.dumps(value, fmt=plistlib.FMT_XML, sort_keys=True)
    )
    path.chmod(0o644)


def valid_binding(
    request: front_door.CloneFrontDoorRequest,
) -> dict[str, object]:
    return {
        "asset_retirement_delete_entries_verified": 18,
        "asset_retirement_delete_manifest_sha256": (
            request.asset_retirement_delete_manifest_sha256
        ),
        "case_contract": "validated",
        "control_sha256": "d" * 64,
        "dedicated_registration_shell": True,
        "deleted_packages_absent": 3,
        "format": front_door.EVIDENCE_FORMAT,
        "post_retirement_inventory_sha256": (
            request.expected_preclone_inventory_sha256
        ),
        "post_retirement_vm_count": request.expected_vm_count,
        "registration_shell_entries_verified": 1,
        "registration_shell_manifest_sha256": (
            request.registration_shell_manifest_sha256
        ),
        "repository_clean": True,
        "repository_head": request.expected_repository_head,
    }


def observation(
    argv: tuple[str, ...],
    *,
    exit_code: int | None = 0,
    stdout: bytes = b"",
    stderr: bytes = b"",
) -> clone_control.CommandObservation:
    return clone_control.CommandObservation.from_bytes(
        argv, exit_code=exit_code, stdout=stdout, stderr=stderr
    )


def preclone_list(*, peer_status: str = "stopped") -> bytes:
    return (
        "UUID Status Name\n"
        f"{SHELL_UUID} stopped {SHELL_NAME}\n"
        f"{PEER_UUID} {peer_status} Synthetic-Peer\n"
    ).encode("utf-8")


def created_list(*, peer_status: str = "stopped") -> bytes:
    return (
        "UUID Status Name\n"
        f"{SHELL_UUID} stopped {SHELL_NAME}\n"
        f"{PEER_UUID} {peer_status} Synthetic-Peer\n"
        f"{TARGET_UUID} stopped {TARGET_NAME}\n"
    ).encode("utf-8")


def inventory_sha256(value: bytes) -> str:
    parsed = clone_control.parse_utmctl_list(
        observation(("utmctl", "list"), stdout=value)
    )
    return clone_control.canonical_inventory_sha256(parsed)


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def read_json(path: Path) -> dict[str, object]:
    return json.loads(path.read_text(encoding="utf-8"))


def assert_manifest_valid(root: Path) -> None:
    lines = (root / "files.sha256").read_text(encoding="ascii").splitlines()
    for line in lines:
        expected_hash, name = line.split("  ", maxsplit=1)
        assert sha256(root / name) == expected_hash


if __name__ == "__main__":
    unittest.main()
