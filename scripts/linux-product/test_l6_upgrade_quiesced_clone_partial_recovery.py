#!/usr/bin/env python3
from __future__ import annotations

import hashlib
import json
import plistlib
import shutil
import tempfile
import unittest
from dataclasses import replace
from pathlib import Path
from unittest import mock

import l6_upgrade_quiesced_case_contract as case_contract
import l6_upgrade_quiesced_clone_bindings as clone_bindings
import l6_upgrade_quiesced_clone_partial_recovery as control
import l6_upgrade_quiesced_clone_partial_recovery_bindings as bindings
import l6_utm_clone_once as clone_control


SHELL_UUID = "AAAAAAAA-AAAA-4AAA-8AAA-AAAAAAAAAAAA"
TARGET_UUID = "BBBBBBBB-BBBB-4BBB-8BBB-BBBBBBBBBBBB"
FOREIGN_UUID = "CCCCCCCC-CCCC-4CCC-8CCC-CCCCCCCCCCCC"
SHELL_NAME = "Synthetic-Registration-Shell"
TARGET_NAME = "Synthetic-Upgrade-Target"
PARTIAL_QCOW2 = "partial.qcow2"
SOURCE_QCOW2 = "source.qcow2"


class FakeRunner:
    def __init__(
        self,
        request: control.RecoveryRequestBase,
        *,
        running_foreign: bool = False,
        fail_qcow2_replace: bool = False,
        handle_on_call: int | None = None,
    ) -> None:
        self.request = request
        self.running_foreign = running_foreign
        self.fail_qcow2_replace = fail_qcow2_replace
        self.handle_on_call = handle_on_call
        self.handle_calls = 0
        self.calls: list[tuple[str, ...]] = []

    def run(
        self, argv: tuple[str, ...], timeout_seconds: int
    ) -> clone_control.CommandObservation:
        del timeout_seconds
        self.calls.append(argv)
        if argv[0] == "/usr/sbin/lsof":
            self.handle_calls += 1
            if self.handle_calls == self.handle_on_call:
                return observation(
                    argv,
                    stdout=b"p123\ncQEMULauncher\nf4\ntREG\n",
                )
            return observation(argv, exit_code=1)
        if argv == ("utmctl", "list"):
            current = inventory(
                foreign_status=(
                    "started" if self.running_foreign else "stopped"
                )
            )
            return observation(argv, stdout=inventory_bytes(current))
        if argv[:2] == ("/bin/cp", "-c"):
            shutil.copyfile(argv[2], argv[3])
            Path(argv[3]).chmod(0o644)
            return observation(argv)
        if argv[:2] == ("/bin/mv", "-f"):
            if self.fail_qcow2_replace and argv[3].endswith(PARTIAL_QCOW2):
                return observation(argv, exit_code=1, stderr=b"synthetic\n")
            Path(argv[2]).replace(argv[3])
            return observation(argv)
        raise AssertionError(f"unexpected command: {argv}")


class LinuxL6UpgradeQuiescedClonePartialRecoveryTests(
    unittest.TestCase
):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        self.repository = self.root / "repo"
        self.repository.mkdir()
        self.operator = self.root / "VirtualMachines"
        self.operator.mkdir(mode=0o755)
        self.documents = self.root / "UTM-Documents"
        self.documents.mkdir(mode=0o755)
        self.source = self.operator / "synthetic-s2"
        self.shell = self.operator / f"{SHELL_NAME}.utm"
        self.default_target = self.documents / f"{TARGET_NAME}.utm"
        self.target = self.operator / f"{TARGET_NAME}.utm"
        make_bundle(
            self.source,
            name="Synthetic-S2",
            vm_uuid="DDDDDDDD-DDDD-4DDD-8DDD-DDDDDDDDDDDD",
            qcow2_name=SOURCE_QCOW2,
            efi=b"source-efi",
            qcow2=b"source-qcow2",
            mode=0o700,
        )
        local = self.source / "local-snapshot.evidence.json"
        local.write_text('{"synthetic":"s2"}\n', encoding="utf-8")
        local.chmod(0o600)
        make_bundle(
            self.shell,
            name=SHELL_NAME,
            vm_uuid=SHELL_UUID,
            qcow2_name=PARTIAL_QCOW2,
            efi=b"partial-efi",
            qcow2=b"partial-qcow2",
        )
        make_bundle(
            self.default_target,
            name=TARGET_NAME,
            vm_uuid=TARGET_UUID,
            qcow2_name=PARTIAL_QCOW2,
            efi=b"partial-efi",
            qcow2=b"partial-qcow2",
        )
        self.retirement = self.operator / "retirement"
        self.predecessor = self.operator / "predecessor"
        self.partial_root = self.operator / "partial-clone"
        self.shell_evidence = self.operator / "shell-evidence"
        self.inventory = inventory()
        self.inventory_sha256 = clone_control.canonical_inventory_sha256(
            self.inventory
        )
        self.preclone = tuple(
            item for item in self.inventory if item.uuid != TARGET_UUID
        )
        self.preclone_sha256 = clone_control.canonical_inventory_sha256(
            self.preclone
        )
        source_expected = {
            "relative_path": self.source.name,
            "config_sha256": sha256(self.source / "config.plist"),
            "efi_sha256": sha256(self.source / "Data/efi_vars.fd"),
            "local_evidence_sha256": sha256(local),
            "qcow2_sha256": sha256(
                self.source / "Data" / SOURCE_QCOW2
            ),
        }
        baseline = {
            "delete_evidence_relative_path": self.retirement.name,
            "delete_manifest_sha256": "d" * 64,
            "inventory_sha256": "e" * 64,
            "managed_members": [{"name": SHELL_NAME, "uuid": SHELL_UUID}],
            "registered_vm_count": 1,
        }
        predecessor = {
            "evidence_relative_path": self.predecessor.name,
            "foreign_vm_count": 1,
            "manifest_sha256": "f" * 64,
            "outcome": "precondition-rejected",
        }
        partial = {
            "attempt_id": "synthetic-clone-v2",
            "clone_invocations": 1,
            "control_sha256": "1" * 64,
            "default_package": {
                "config_sha256": sha256(
                    self.default_target / "config.plist"
                ),
                "efi_sha256": sha256(
                    self.default_target / "Data/efi_vars.fd"
                ),
                "qcow2_name": PARTIAL_QCOW2,
                "qcow2_sha256": sha256(
                    self.default_target / "Data" / PARTIAL_QCOW2
                ),
            },
            "evidence_relative_path": self.partial_root.name,
            "foreign_vm_count": 1,
            "manifest_sha256": "2" * 64,
            "outcome": "state-indeterminate",
            "postclone_inventory_sha256": self.inventory_sha256,
            "postclone_managed_vm_count": 2,
            "postclone_registered_vm_count": 3,
            "preclone_inventory_sha256": self.preclone_sha256,
            "preclone_registered_vm_count": 2,
            "reason": (
                "target-package-postclone:"
                "clone-postconditions-indeterminate"
            ),
            "replacement_count": 0,
            "repository_head": "a" * 40,
            "target_name": TARGET_NAME,
            "target_uuid": TARGET_UUID,
        }
        self.patches = (
            mock.patch.dict(case_contract.EXPECTED_START, source_expected),
            mock.patch.dict(
                case_contract.EXPECTED_CLONE_FRONT_DOOR[
                    "preclone_baseline"
                ],
                baseline,
                clear=True,
            ),
            mock.patch.dict(
                case_contract.EXPECTED_CLONE_FRONT_DOOR[
                    "predecessor_failure"
                ],
                predecessor,
                clear=True,
            ),
            mock.patch.dict(
                case_contract.EXPECTED_CLONE_FRONT_DOOR[
                    "partial_recovery"
                ]["partial_clone"],
                partial,
                clear=True,
            ),
            mock.patch.dict(
                case_contract.EXPECTED_CLONE_FRONT_DOOR[
                    "partial_recovery"
                ]["move"],
                {
                    "adopt_attempt_id": "adopt",
                    "adopt_evidence_relative_path": "output-adopt",
                    "prepare_attempt_id": "prepare",
                    "prepare_evidence_relative_path": "output-prepare",
                },
            ),
            mock.patch.dict(
                case_contract.EXPECTED_CLONE_FRONT_DOOR[
                    "partial_recovery"
                ]["materialization"],
                {
                    "attempt_id": "materialize",
                    "evidence_relative_path": "output-materialize",
                },
            ),
        )
        for patch in self.patches:
            patch.start()

    def tearDown(self) -> None:
        for patch in reversed(self.patches):
            patch.stop()
        self.temporary.cleanup()

    def test_move_prepare_is_read_only_and_emits_one_ui_move(self) -> None:
        request = self.prepare_request()
        runner = FakeRunner(request)

        result = control.run_move_prepare(
            request, runner=runner, binding_validator=valid_prepare_binding
        )

        self.assertEqual(result.outcome, "move-ready")
        self.assertEqual(result.inventory_query_invocations, 1)
        self.assertEqual(result.copy_invocations, 0)
        self.assertTrue(self.default_target.exists())
        self.assertFalse(self.target.exists())
        self.assertEqual(
            manifest_names(request.output_root), bindings.MOVE_PREPARE_MEMBERS
        )
        terminal = read_json(request.output_root / "terminal.json")
        self.assertEqual(
            terminal["next_external_action"],
            "one-utm-ui-move-to-authorized-target-path",
        )

    def test_move_prepare_rejects_running_foreign(self) -> None:
        request = self.prepare_request(attempt_id="prepare-running")
        runner = FakeRunner(request, running_foreign=True)

        result = control.run_move_prepare(
            request, runner=runner, binding_validator=valid_prepare_binding
        )

        self.assertEqual(result.outcome, "precondition-rejected")
        self.assertEqual(result.inventory_query_invocations, 1)
        self.assertTrue(self.default_target.exists())
        self.assertFalse(self.target.exists())

    def test_move_adopt_is_read_only_and_preserves_partial_bytes(self) -> None:
        self.default_target.rename(self.target)
        request = self.adopt_request()
        before = bundle_hashes(self.target)
        runner = FakeRunner(request)

        result = control.run_move_adopt(
            request, runner=runner, binding_validator=valid_adopt_binding
        )

        self.assertEqual(result.outcome, "move-adopted")
        self.assertEqual(result.inventory_query_invocations, 2)
        self.assertEqual(result.copy_invocations, 0)
        self.assertEqual(bundle_hashes(self.target), before)
        self.assertEqual(
            manifest_names(request.output_root), bindings.MOVE_ADOPT_MEMBERS
        )

    def test_move_adopt_without_ui_move_fails_before_inventory(self) -> None:
        request = self.adopt_request(attempt_id="adopt-without-move")
        runner = FakeRunner(request)

        result = control.run_move_adopt(
            request, runner=runner, binding_validator=valid_adopt_binding
        )

        self.assertEqual(result.outcome, "state-indeterminate")
        self.assertEqual(result.inventory_query_invocations, 0)
        self.assertEqual(runner.calls, [])

    def test_move_prepare_evidence_is_recursively_bound(self) -> None:
        prepare = self.prepare_request(attempt_id="prepare-binding")
        result = control.run_move_prepare(
            prepare,
            runner=FakeRunner(prepare),
            binding_validator=valid_prepare_binding,
        )
        adopt = self.adopt_request(
            attempt_id="adopt-binding",
            move_prepare_root=prepare.output_root,
            move_prepare_manifest_sha256=result.manifest_sha256,
        )

        evidence = bindings.validate_move_prepare_evidence(adopt)

        self.assertEqual(evidence["entries_verified"], 16)
        self.assertEqual(evidence["outcome"], "move-ready")

    def test_move_adopt_evidence_recursively_binds_prepare(self) -> None:
        prepare = self.prepare_request(attempt_id="prepare-for-adopt")
        prepare_result = control.run_move_prepare(
            prepare,
            runner=FakeRunner(prepare),
            binding_validator=valid_prepare_binding,
        )
        self.default_target.rename(self.target)
        adopt = self.adopt_request(
            attempt_id="adopt-for-materialize",
            move_prepare_root=prepare.output_root,
            move_prepare_manifest_sha256=prepare_result.manifest_sha256,
        )
        adopt_result = control.run_move_adopt(
            adopt,
            runner=FakeRunner(adopt),
            binding_validator=valid_adopt_binding,
        )
        materialize = self.materialize_request(
            attempt_id="materialize-binding",
            move_prepare_root=prepare.output_root,
            move_prepare_manifest_sha256=prepare_result.manifest_sha256,
            move_adopt_root=adopt.output_root,
            move_adopt_manifest_sha256=adopt_result.manifest_sha256,
        )

        evidence = bindings.validate_move_adopt_evidence(materialize)

        self.assertEqual(evidence["entries_verified"], 19)
        self.assertEqual(evidence["outcome"], "move-adopted")
        self.assertEqual(
            evidence["prepare_evidence"]["outcome"], "move-ready"
        )

    def test_materialize_replaces_only_efi_and_qcow2(self) -> None:
        self.default_target.rename(self.target)
        request = self.materialize_request()
        source_before = bundle_hashes(self.source)
        target_config = sha256(self.target / "config.plist")
        runner = FakeRunner(request)

        result = control.run_materialize(
            request,
            runner=runner,
            binding_validator=valid_materialize_binding,
        )

        self.assertEqual(result.outcome, "materialized")
        self.assertEqual(result.inventory_query_invocations, 2)
        self.assertEqual(result.copy_invocations, 2)
        self.assertEqual(result.replacement_count, 2)
        self.assertEqual(sha256(self.target / "config.plist"), target_config)
        self.assertEqual(
            sha256(self.target / "Data/efi_vars.fd"),
            sha256(self.source / "Data/efi_vars.fd"),
        )
        self.assertEqual(
            sha256(self.target / "Data" / PARTIAL_QCOW2),
            sha256(self.source / "Data" / SOURCE_QCOW2),
        )
        self.assertEqual(bundle_hashes(self.source), source_before)
        self.assertEqual(
            manifest_names(request.output_root), bindings.MATERIALIZE_MEMBERS
        )

    def test_second_replacement_failure_is_not_rolled_back(self) -> None:
        self.default_target.rename(self.target)
        request = self.materialize_request(attempt_id="replace-failure")
        runner = FakeRunner(request, fail_qcow2_replace=True)

        result = control.run_materialize(
            request,
            runner=runner,
            binding_validator=valid_materialize_binding,
        )

        self.assertEqual(result.outcome, "state-indeterminate")
        self.assertEqual(result.copy_invocations, 2)
        self.assertEqual(result.replacement_count, 1)
        terminal = read_json(request.output_root / "terminal.json")
        self.assertEqual(terminal["automatic_rollback"], "not-performed")
        self.assertEqual(terminal["automatic_retry"], "not-performed")

    def test_materialize_rechecks_handles_after_inventory(self) -> None:
        self.default_target.rename(self.target)
        request = self.materialize_request(attempt_id="post-list-handle")
        runner = FakeRunner(request, handle_on_call=2)

        result = control.run_materialize(
            request,
            runner=runner,
            binding_validator=valid_materialize_binding,
        )

        self.assertEqual(result.outcome, "precondition-rejected")
        self.assertEqual(result.inventory_query_invocations, 1)
        self.assertEqual(result.copy_invocations, 0)
        self.assertEqual(result.replacement_count, 0)
        self.assertFalse(any(call[0] == "/bin/cp" for call in runner.calls))

    def test_missing_phase_authorization_creates_no_evidence(self) -> None:
        request = self.prepare_request(
            attempt_id="missing-auth", authorized_emit_one_utm_ui_move=False
        )

        with self.assertRaisesRegex(
            control.RecoveryControlError,
            "move-prepare-authorization-required",
        ):
            control.run_move_prepare(request)

        self.assertFalse(request.output_root.exists())

    def test_phase_identity_drift_creates_no_evidence(self) -> None:
        request = self.prepare_request()
        drifted = replace(request, output_root=self.operator / "wrong-root")

        with self.assertRaisesRegex(
            control.RecoveryControlError, "move-prepare-output-root-drift"
        ):
            control.run_move_prepare(drifted)

        self.assertFalse(drifted.output_root.exists())

    def test_partial_clone_binding_rejects_terminal_drift(self) -> None:
        request = self.prepare_request(attempt_id="binding")
        request = self.write_partial_clone_evidence(request)
        source = clone_bindings.validate_source_snapshot(request)
        registration = clone_bindings.validate_registration_shell(request)
        proxy = bindings._front_door_proxy(request)

        result = bindings.validate_partial_clone_evidence(
            request, proxy, source, registration
        )
        self.assertEqual(result["entries_verified"], 12)

        terminal = self.partial_root / "terminal.json"
        value = read_json(terminal)
        value["replacement_count"] = 1
        terminal.write_text(
            json.dumps(value, sort_keys=True) + "\n", encoding="utf-8"
        )
        terminal.chmod(0o600)
        with self.assertRaises(bindings.RecoveryBindingError):
            bindings.validate_partial_clone_evidence(
                request, proxy, source, registration
            )

    def prepare_request(
        self, **overrides: object
    ) -> control.MovePrepareRequest:
        values = self.common_values(str(overrides.get("attempt_id", "prepare")))
        values.update(
            {
                "authorized_move_prepare": True,
                "authorized_one_inventory_query": True,
                "authorized_emit_one_utm_ui_move": True,
                (
                    "authorized_no_move_materialize_start_clone_delete_retry_"
                    "guest_or_transaction"
                ): True,
            }
        )
        values.update(overrides)
        move = case_contract.EXPECTED_CLONE_FRONT_DOOR[
            "partial_recovery"
        ]["move"]
        move["prepare_attempt_id"] = values["attempt_id"]
        move["prepare_evidence_relative_path"] = Path(
            values["output_root"]
        ).name
        return control.MovePrepareRequest(**values)  # type: ignore[arg-type]

    def adopt_request(
        self, **overrides: object
    ) -> control.MoveAdoptRequest:
        values = self.common_values(str(overrides.get("attempt_id", "adopt")))
        values.update(
            {
                "move_prepare_root": self.operator / "prepare-evidence",
                "move_prepare_manifest_sha256": "3" * 64,
                "authorized_adopt_one_utm_ui_move_result": True,
                "authorized_two_inventory_queries": True,
                (
                    "authorized_no_materialize_start_clone_delete_retry_"
                    "guest_or_transaction"
                ): True,
            }
        )
        values.update(overrides)
        move = case_contract.EXPECTED_CLONE_FRONT_DOOR[
            "partial_recovery"
        ]["move"]
        move["adopt_attempt_id"] = values["attempt_id"]
        move["adopt_evidence_relative_path"] = Path(
            values["output_root"]
        ).name
        move["prepare_evidence_relative_path"] = Path(
            values["move_prepare_root"]
        ).name
        return control.MoveAdoptRequest(**values)  # type: ignore[arg-type]

    def materialize_request(
        self, **overrides: object
    ) -> control.MaterializeRequest:
        values = self.common_values(
            str(overrides.get("attempt_id", "materialize"))
        )
        values.update(
            {
                "move_prepare_root": self.operator / "prepare-evidence",
                "move_prepare_manifest_sha256": "3" * 64,
                "move_adopt_root": self.operator / "adopt-evidence",
                "move_adopt_manifest_sha256": "4" * 64,
                "clonefile_timeout_seconds": 30,
                "authorized_materialize_s2_once": True,
                "authorized_two_inventory_queries": True,
                "authorized_two_clonefile_copies": True,
                "authorized_two_atomic_replacements": True,
                (
                    "authorized_no_move_start_clone_delete_retry_rollback_"
                    "guest_or_transaction"
                ): True,
            }
        )
        values.update(overrides)
        recovery = case_contract.EXPECTED_CLONE_FRONT_DOOR[
            "partial_recovery"
        ]
        recovery["materialization"]["attempt_id"] = values["attempt_id"]
        recovery["materialization"]["evidence_relative_path"] = Path(
            values["output_root"]
        ).name
        recovery["move"]["prepare_evidence_relative_path"] = Path(
            values["move_prepare_root"]
        ).name
        recovery["move"]["adopt_evidence_relative_path"] = Path(
            values["move_adopt_root"]
        ).name
        return control.MaterializeRequest(**values)  # type: ignore[arg-type]

    def common_values(self, attempt_id: str) -> dict[str, object]:
        return {
            "repository_root": self.repository,
            "expected_repository_head": "9" * 40,
            "output_root": self.operator / f"output-{attempt_id}",
            "attempt_id": attempt_id,
            "operator_asset_root": self.operator,
            "utm_documents_root": self.documents,
            "asset_retirement_delete_root": self.retirement,
            "asset_retirement_delete_manifest_sha256": "d" * 64,
            "predecessor_failure_root": self.predecessor,
            "predecessor_failure_manifest_sha256": "f" * 64,
            "partial_clone_root": self.partial_root,
            "partial_clone_manifest_sha256": "2" * 64,
            "source_snapshot_root": self.source,
            "registration_shell_evidence_root": self.shell_evidence,
            "registration_shell_manifest_sha256": "5" * 64,
            "registration_shell_uuid": SHELL_UUID,
            "registration_shell_name": SHELL_NAME,
            "registration_shell_package_path": self.shell,
            "target_uuid": TARGET_UUID,
            "target_name": TARGET_NAME,
            "default_target_package_path": self.default_target,
            "target_package_path": self.target,
            "expected_vm_count": len(self.inventory),
            "expected_inventory_sha256": self.inventory_sha256,
            "command_timeout_seconds": 5,
        }

    def write_partial_clone_evidence(
        self, request: control.MovePrepareRequest
    ) -> control.MovePrepareRequest:
        self.partial_root.mkdir(mode=0o700)
        proxy = bindings._front_door_proxy(request)
        source = clone_bindings.validate_source_snapshot(request)
        registration = clone_bindings.validate_registration_shell(request)
        values: dict[str, object] = {
            name: {"synthetic": name}
            for name in bindings.PARTIAL_CLONE_MEMBERS
        }
        values["request.json"] = bindings._expected_partial_request(request)
        values["binding-preflight.json"] = (
            bindings._expected_partial_binding(request)
        )
        values["source-snapshot-preflight.json"] = source.as_json()
        values["registration-shell-preflight.json"] = registration.as_json()
        absent = bindings._absent_package_evidence(self.target)
        values["target-package-preclone.json"] = absent
        values["target-package-postclone.json"] = absent
        handles_argv = bindings._lsof_argv(source, registration)
        values["source-and-shell-handles-preflight.json"] = observation(
            handles_argv, exit_code=1
        ).as_json()
        values["utmctl-list-preclone.json"] = observation(
            ("utmctl", "list"), stdout=inventory_bytes(self.preclone)
        ).as_json()
        values["inventory-classification-preclone.json"] = (
            clone_bindings.validate_live_preclone_inventory(
                self.preclone, proxy
            )
        )
        values["utmctl-clone.json"] = observation(
            ("utmctl", "clone", SHELL_UUID, "--name", TARGET_NAME)
        ).as_json()
        values["utmctl-list-postclone.json"] = observation(
            ("utmctl", "list"), stdout=inventory_bytes(self.inventory)
        ).as_json()
        values["terminal.json"] = bindings._expected_partial_terminal(request)
        for name in bindings.PARTIAL_CLONE_MEMBERS:
            path = self.partial_root / name
            path.write_text(
                json.dumps(values[name], sort_keys=True) + "\n",
                encoding="utf-8",
            )
            path.chmod(0o600)
        manifest = self.partial_root / "files.sha256"
        manifest.write_text(
            "".join(
                f"{sha256(self.partial_root / name)}  {name}\n"
                for name in bindings.PARTIAL_CLONE_MEMBERS
            ),
            encoding="ascii",
        )
        manifest.chmod(0o600)
        digest = sha256(manifest)
        case_contract.EXPECTED_CLONE_FRONT_DOOR["partial_recovery"][
            "partial_clone"
        ]["manifest_sha256"] = digest
        return replace(request, partial_clone_manifest_sha256=digest)


def valid_prepare_binding(
    request: control.MovePrepareRequest,
) -> dict[str, object]:
    return synthetic_phase_binding(request, "move-prepare")


def valid_adopt_binding(
    request: control.MoveAdoptRequest,
) -> dict[str, object]:
    return synthetic_phase_binding(request, "move-adopt")


def valid_materialize_binding(
    request: control.MaterializeRequest,
) -> dict[str, object]:
    return {"format": bindings.EVIDENCE_FORMAT, "phase": "materialize"}


def synthetic_phase_binding(
    request: control.RecoveryRequestBase, phase: str
) -> dict[str, object]:
    return {
        "case_contract": "validated",
        "format": bindings.EVIDENCE_FORMAT,
        "partial_clone": {
            "manifest_sha256": request.partial_clone_manifest_sha256,
            "outcome": "state-indeterminate",
            "replacement_count": 0,
        },
        "phase": phase,
        "repository_clean": True,
        "repository_head": request.expected_repository_head,
    }


def inventory(
    *, foreign_status: str = "stopped"
) -> tuple[clone_control.RegisteredVm, ...]:
    return (
        clone_control.RegisteredVm(SHELL_UUID, "stopped", SHELL_NAME),
        clone_control.RegisteredVm(TARGET_UUID, "stopped", TARGET_NAME),
        clone_control.RegisteredVm(
            FOREIGN_UUID, foreign_status, "Foreign-Peer"
        ),
    )


def inventory_bytes(
    value: tuple[clone_control.RegisteredVm, ...]
) -> bytes:
    lines = ["UUID Status Name"]
    lines.extend(f"{item.uuid} {item.status} {item.name}" for item in value)
    return ("\n".join(lines) + "\n").encode("utf-8")


def make_bundle(
    root: Path,
    *,
    name: str,
    vm_uuid: str,
    qcow2_name: str,
    efi: bytes,
    qcow2: bytes,
    mode: int = 0o755,
) -> None:
    data = root / "Data"
    data.mkdir(parents=True, mode=mode)
    config = {
        "Drive": [
            {
                "Identifier": qcow2_name.removesuffix(".qcow2"),
                "ImageName": qcow2_name,
                "ImageType": "Disk",
                "Interface": "VirtIO",
            }
        ],
        "Information": {"Name": name, "UUID": vm_uuid},
        "Network": [],
    }
    config_path = root / "config.plist"
    config_path.write_bytes(plistlib.dumps(config, sort_keys=True))
    (data / "efi_vars.fd").write_bytes(efi)
    (data / qcow2_name).write_bytes(qcow2)
    root.chmod(mode)
    data.chmod(mode)
    for path in (config_path, data / "efi_vars.fd", data / qcow2_name):
        path.chmod(0o644)


def bundle_hashes(root: Path) -> tuple[str, ...]:
    value = plistlib.loads((root / "config.plist").read_bytes())
    qcow2 = value["Drive"][0]["ImageName"]
    return (
        sha256(root / "config.plist"),
        sha256(root / "Data/efi_vars.fd"),
        sha256(root / "Data" / qcow2),
    )


def observation(
    argv: tuple[str, ...],
    *,
    exit_code: int = 0,
    stdout: bytes = b"",
    stderr: bytes = b"",
) -> clone_control.CommandObservation:
    return clone_control.CommandObservation.from_bytes(
        argv, exit_code=exit_code, stdout=stdout, stderr=stderr
    )


def manifest_names(root: Path) -> tuple[str, ...]:
    return tuple(
        line[66:]
        for line in (root / "files.sha256")
        .read_text(encoding="ascii")
        .splitlines()
    )


def read_json(path: Path) -> dict[str, object]:
    return json.loads(path.read_text(encoding="utf-8"))


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        while block := source.read(1024 * 1024):
            digest.update(block)
    return digest.hexdigest()


if __name__ == "__main__":
    unittest.main()
