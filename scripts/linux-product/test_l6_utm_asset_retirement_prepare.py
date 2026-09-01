#!/usr/bin/env python3
from __future__ import annotations

import hashlib
import json
import plistlib
import tempfile
import unittest
from pathlib import Path

import l6_utm_asset_retirement_prepare as retirement
import l6_utm_clone_once as clone_control


ASSET_ID = "synthetic-retirement-candidate"
ASSET_UUID = "AAAAAAAA-AAAA-4AAA-8AAA-AAAAAAAAAAAA"
ASSET_NAME = "RadishLex-Synthetic-Retirement-Candidate"
QCOW2_NAME = "BBBBBBBB-BBBB-4BBB-8BBB-BBBBBBBBBBBB.qcow2"
PEER_UUID = "CCCCCCCC-CCCC-4CCC-8CCC-CCCCCCCCCCCC"


class FakeRunner:
    def __init__(
        self,
        observations: list[clone_control.CommandObservation],
        *,
        after_call: dict[int, object] | None = None,
    ) -> None:
        self.observations = observations
        self.after_call = after_call or {}
        self.calls: list[tuple[str, ...]] = []

    def run(
        self, argv: tuple[str, ...], timeout_seconds: int
    ) -> clone_control.CommandObservation:
        del timeout_seconds
        if not self.observations:
            raise AssertionError(f"unexpected command: {argv}")
        observation = self.observations.pop(0)
        if observation.argv != argv:
            raise AssertionError(
                f"expected command {observation.argv}, received {argv}"
            )
        self.calls.append(argv)
        callback = self.after_call.get(len(self.calls))
        if callable(callback):
            callback()
        return observation


class LinuxL6AssetRetirementPrepareTests(unittest.TestCase):
    def test_prepared_uses_one_inventory_query_and_two_handle_rounds(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            fixture = Fixture.create(Path(temporary).resolve())
            handles = retirement._lsof_argv(fixture.identities())
            runner = FakeRunner(
                [
                    observation(("utmctl", "list"), stdout=inventory()),
                    observation(handles, exit_code=1),
                    observation(handles, exit_code=1),
                ]
            )

            result = retirement.run_prepare(
                fixture.request,
                runner=runner,
                binding_validator=fixture.binding,
            )

            self.assertEqual(result.outcome, "prepared")
            self.assertEqual(result.exit_code, retirement.EXIT_PREPARED)
            self.assertEqual(result.inventory_queries, 1)
            self.assertEqual(result.handle_rounds, 2)
            self.assertEqual(result.asset_hash_rounds, 2)
            self.assertEqual(runner.calls.count(("utmctl", "list")), 1)
            self.assertEqual(runner.calls.count(handles), 2)
            self.assertTrue(
                all(call[0] in ("utmctl", "/usr/sbin/lsof") for call in runner.calls)
            )
            terminal = read_json(fixture.request.output_root / "terminal.json")
            self.assertEqual(terminal["outcome"], "prepared")
            self.assertEqual(terminal["utmctl_delete_invocations"], 0)
            self.assertEqual(terminal["automatic_delete"], "not-performed")
            self.assertEqual(terminal["start"], "not-performed")
            self.assertEqual(terminal["clone"], "not-performed")
            self.assertEqual(terminal["move"], "not-performed")
            self.assertEqual(terminal["guest"], "not-entered")
            assert_manifest_valid(self, fixture.request.output_root)

    def test_inventory_drift_rejects_before_handle_checks(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            fixture = Fixture.create(Path(temporary).resolve())
            running = inventory(asset_status="started")
            runner = FakeRunner(
                [observation(("utmctl", "list"), stdout=running)]
            )

            result = retirement.run_prepare(
                fixture.request,
                runner=runner,
                binding_validator=fixture.binding,
            )

            self.assertEqual(result.outcome, "precondition-rejected")
            self.assertEqual(result.inventory_queries, 1)
            self.assertEqual(result.handle_rounds, 0)
            self.assertEqual(result.asset_hash_rounds, 1)
            self.assertEqual(runner.calls, [("utmctl", "list")])
            terminal = read_json(fixture.request.output_root / "terminal.json")
            self.assertIn("inventory-not-all-stopped", terminal["reason"])
            self.assertEqual(terminal["utmctl_delete_invocations"], 0)

    def test_open_handle_rejects_without_second_handle_or_hash_round(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            fixture = Fixture.create(Path(temporary).resolve())
            handles = retirement._lsof_argv(fixture.identities())
            runner = FakeRunner(
                [
                    observation(("utmctl", "list"), stdout=inventory()),
                    observation(handles, exit_code=0, stdout=b"p123\n"),
                ]
            )

            result = retirement.run_prepare(
                fixture.request,
                runner=runner,
                binding_validator=fixture.binding,
            )

            self.assertEqual(result.outcome, "precondition-rejected")
            self.assertEqual(result.inventory_queries, 1)
            self.assertEqual(result.handle_rounds, 1)
            self.assertEqual(result.asset_hash_rounds, 1)
            self.assertEqual(len(runner.calls), 2)
            terminal = read_json(fixture.request.output_root / "terminal.json")
            self.assertIn("asset-open-handles", terminal["reason"])

    def test_disk_drift_between_rounds_rejects_closed(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            fixture = Fixture.create(Path(temporary).resolve())
            handles = retirement._lsof_argv(fixture.identities())
            runner = FakeRunner(
                [
                    observation(("utmctl", "list"), stdout=inventory()),
                    observation(handles, exit_code=1),
                    observation(handles, exit_code=1),
                ],
                after_call={3: lambda: fixture.qcow2_path.write_bytes(b"drift")},
            )

            result = retirement.run_prepare(
                fixture.request,
                runner=runner,
                binding_validator=fixture.binding,
            )

            self.assertEqual(result.outcome, "precondition-rejected")
            self.assertEqual(result.inventory_queries, 1)
            self.assertEqual(result.handle_rounds, 2)
            self.assertEqual(result.asset_hash_rounds, 1)
            terminal = read_json(fixture.request.output_root / "terminal.json")
            self.assertIn("bundle-disk-sha256-drift", terminal["reason"])

    def test_evidence_identity_mismatch_rejects_before_commands(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            fixture = Fixture.create(Path(temporary).resolve())
            payload = read_json(fixture.anchor_path)
            payload["source_vm"]["uuid"] = PEER_UUID
            fixture.anchor_path.write_text(
                json.dumps(payload, sort_keys=True) + "\n", encoding="utf-8"
            )
            fixture.anchor_path.chmod(0o600)
            changed_anchor = retirement.EvidenceAnchor(
                storage="operator",
                relative_path=fixture.anchor.relative_path,
                sha256=sha256_file(fixture.anchor_path),
                mode=0o600,
                semantic="snapshot-source-vm",
            )
            fixture.batch = retirement.RetirementBatch(
                batch_id=fixture.batch.batch_id,
                asset_ids=fixture.batch.asset_ids,
                accounting_gib=fixture.batch.accounting_gib,
                assets=(fixture.asset_with_anchor(changed_anchor),),
            )
            runner = FakeRunner([])

            result = retirement.run_prepare(
                fixture.request,
                runner=runner,
                binding_validator=fixture.binding,
            )

            self.assertEqual(result.outcome, "precondition-rejected")
            self.assertEqual(result.inventory_queries, 0)
            self.assertEqual(runner.calls, [])
            terminal = read_json(fixture.request.output_root / "terminal.json")
            self.assertIn("snapshot-source-identity-mismatch", terminal["reason"])

    def test_all_authorizations_are_required_before_output_creation(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            for field in (
                "authorized_read_only_prepare",
                "authorized_one_utmctl_list",
                "authorized_two_handle_rounds",
                "authorized_no_delete_start_clone_move_or_guest",
            ):
                fixture = Fixture.create(root / field)
                request = fixture.request_with(**{field: False})
                with self.assertRaises(retirement.RetirementPrepareError):
                    retirement.run_prepare(
                        request,
                        runner=FakeRunner([]),
                        binding_validator=fixture.binding,
                    )
                self.assertFalse(request.output_root.exists())

    def test_allowlist_rejects_non_candidate_and_more_than_four_assets(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            fixture = Fixture.create(root)
            path = root / "allowlist.json"
            asset = fixture.allowlist_asset()
            payload = {
                "format": retirement.ALLOWLIST_FORMAT,
                "storage_roots": {
                    "operator": str(fixture.request.operator_asset_root),
                    "utm-documents": str(fixture.request.utm_documents_root),
                },
                "assets": [asset],
                "batches": [
                    {
                        "id": "synthetic-batch",
                        "asset_ids": [ASSET_ID],
                        "accounting_gib": "0.01",
                    }
                ],
            }
            path.write_text(json.dumps(payload) + "\n", encoding="utf-8")
            parsed = retirement.load_allowlist(path)
            self.assertEqual(
                parsed.batches["synthetic-batch"].asset_ids, (ASSET_ID,)
            )

            payload["assets"][0]["action"] = "delete"
            path.write_text(json.dumps(payload) + "\n", encoding="utf-8")
            with self.assertRaises(retirement.RetirementPrepareError):
                retirement.load_allowlist(path)

            payload["assets"][0]["action"] = (
                "evidence-archived-delete-candidate"
            )
            payload["batches"][0]["asset_ids"] = [ASSET_ID] * 5
            path.write_text(json.dumps(payload) + "\n", encoding="utf-8")
            with self.assertRaises(retirement.RetirementPrepareError):
                retirement.load_allowlist(path)

    def test_allowlist_rejects_asset_reuse_across_batches(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            fixture = Fixture.create(root)
            path = root / "allowlist.json"
            payload = {
                "format": retirement.ALLOWLIST_FORMAT,
                "storage_roots": {
                    "operator": str(fixture.request.operator_asset_root),
                    "utm-documents": str(fixture.request.utm_documents_root),
                },
                "assets": [fixture.allowlist_asset()],
                "batches": [
                    {
                        "id": "synthetic-batch-one",
                        "asset_ids": [ASSET_ID],
                        "accounting_gib": "0.01",
                    },
                    {
                        "id": "synthetic-batch-two",
                        "asset_ids": [ASSET_ID],
                        "accounting_gib": "0.01",
                    },
                ],
            }
            path.write_text(json.dumps(payload) + "\n", encoding="utf-8")

            with self.assertRaises(retirement.RetirementPrepareError):
                retirement.load_allowlist(path)

            payload["batches"] = payload["batches"][:1]
            unassigned = dict(payload["assets"][0])
            unassigned["id"] = "synthetic-unassigned-candidate"
            unassigned["relative_path"] = "Synthetic-Unassigned.utm"
            unassigned["name"] = "Synthetic-Unassigned"
            unassigned["uuid"] = "DDDDDDDD-DDDD-4DDD-8DDD-DDDDDDDDDDDD"
            payload["assets"].append(unassigned)
            path.write_text(json.dumps(payload) + "\n", encoding="utf-8")
            with self.assertRaises(retirement.RetirementPrepareError):
                retirement.load_allowlist(path)

    def test_repository_allowlist_fixes_five_disjoint_bounded_batches(self) -> None:
        repository_root = Path(__file__).resolve().parents[2]
        allowlist = retirement.load_allowlist(
            repository_root
            / "packaging/linux/l6-asset-retirement-allowlist.json"
        )

        first = allowlist.batches["first-four-v1"]
        second = allowlist.batches["second-batch-v1"]
        third = allowlist.batches["third-batch-v1"]
        fourth = allowlist.batches["fourth-batch-v1"]
        fifth = allowlist.batches["fifth-batch-v1"]
        self.assertEqual(len(first.assets), 4)
        self.assertEqual(
            second.asset_ids,
            (
                "install-prepared-old-pair-failed-closed-80e49ce",
                "install-artifacts-staged-double-start-failed-d75818f",
                "install-artifacts-staged-single-start-failed-d75818f-v3",
                "repair-completed-noop-b891ed1",
            ),
        )
        self.assertEqual(second.accounting_gib, "37.62")
        self.assertTrue(set(first.asset_ids).isdisjoint(second.asset_ids))
        self.assertEqual(
            third.asset_ids,
            (
                "rollback-completed-80e49ce-v3",
                "remove-completed-80e49ce",
                "reinstall-completed-80e49ce",
            ),
        )
        self.assertEqual(third.accounting_gib, "27.99")
        self.assertTrue(set(first.asset_ids).isdisjoint(third.asset_ids))
        self.assertTrue(set(second.asset_ids).isdisjoint(third.asset_ids))
        self.assertEqual(
            fourth.asset_ids,
            (
                "upgrade-completed-80e49ce",
                "install-prepared-completed-stopped-d75818f-v3",
                "install-artifacts-staged-completed-stopped-d75818f-v4",
            ),
        )
        self.assertEqual(fourth.accounting_gib, "27.09")
        for prior in (first, second, third):
            self.assertTrue(set(prior.asset_ids).isdisjoint(fourth.asset_ids))
        self.assertEqual(
            fifth.asset_ids,
            retirement.FIFTH_BATCH_ASSET_IDS,
        )
        self.assertEqual(fifth.accounting_gib, "27.19")
        for prior in (first, second, third, fourth):
            self.assertTrue(set(prior.asset_ids).isdisjoint(fifth.asset_ids))
        self.assertEqual(
            {asset.uuid for asset in fifth.assets},
            {
                "9C5638D7-0F97-4BD8-8A83-ABCDFCADAC6C",
                "394217A7-BFC9-43C8-94E6-539FF3F2B6FB",
                "99FF4B3F-4894-4913-BBE3-4934DC27BEEB",
            },
        )
        self.assertEqual(
            {
                anchor.semantic
                for asset in second.assets
                for anchor in asset.evidence_anchors
            },
            {
                "frozen-disk-identity",
                "install-prepared-failed-closed-terminal",
                "maintenance-repair-terminal",
                "utm-start-terminal",
            },
        )
        self.assertEqual(
            {
                anchor.semantic
                for asset in third.assets
                for anchor in asset.evidence_anchors
            },
            {"completed-operation-terminal"},
        )
        self.assertEqual(
            {
                anchor.semantic
                for asset in fourth.assets
                for anchor in asset.evidence_anchors
            },
            {
                "install-artifacts-staged-stopped-disk",
                "install-artifacts-staged-terminal-stop",
                "install-prepared-stopped-terminal",
                "opaque-hash",
                "retained-terminal-snapshot",
            },
        )
        self.assertEqual(
            {
                anchor.semantic
                for asset in fifth.assets
                for anchor in asset.evidence_anchors
            },
            {
                "fifth-batch-terminal-projection",
                "retained-predecessor-snapshot",
            },
        )
        self.assertEqual(
            sum(
                anchor.storage == "repository"
                for asset in fifth.assets
                for anchor in asset.evidence_anchors
            ),
            3,
        )

    def test_completed_operation_terminals_bind_role_uuid_and_disk(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            fixture = Fixture.create(Path(temporary).resolve())
            asset = fixture.batch.assets[0]
            terminal = Path(temporary) / "terminal.evidence.txt"
            cases = (
                (
                    "rollback-completed-terminal",
                    "radishlex-linux-l6-rollback-host-summary-v1",
                    "rollback|target_older|completed",
                    "rollback_completed=true\n"
                    "registered_vms=7\n"
                    "registered_vms_stopped=7\n"
                    "clone_qcow2_open_handles=0\n"
                    "vm_shutdown=normal-request|stopped\n"
                    "maintenance_retry=false\n"
                    "rollback_retry=false\n",
                ),
                (
                    "remove-completed-terminal",
                    "radishlex-linux-l6-remove-host-v1",
                    "remove|not_applicable|completed",
                    "remove_completed=true\n"
                    "registered_vms=7|all-stopped\n"
                    "remove_rollback_s3_disk_handles=0\n"
                    "product_tree=absent\n"
                    "dpkg_info=absent\n",
                ),
                (
                    "reinstall-completed-terminal",
                    "radishlex-linux-l6-reinstall-host-v1",
                    "install|not_applicable|completed",
                    "reinstall_completed=true\n"
                    "registered_vms=7|all-stopped\n"
                    "reinstall_remove_rollback_s3_disk_handles=0\n"
                    "remove_rollback_s3_disks=unchanged\n",
                ),
            )
            for role, evidence_format, receipt, role_fields in cases:
                with self.subTest(role=role):
                    candidate = retirement.RetirementAsset(
                        asset_id=asset.asset_id,
                        storage=asset.storage,
                        relative_path=asset.relative_path,
                        name=asset.name,
                        uuid=asset.uuid,
                        role=role,
                        config_sha256=asset.config_sha256,
                        efi_sha256=asset.efi_sha256,
                        qcow2_sha256=asset.qcow2_sha256,
                        qcow2_name=asset.qcow2_name,
                        evidence_anchors=asset.evidence_anchors,
                    )
                    payload = (
                        f"format={evidence_format}\n"
                        f"clone_uuid={candidate.uuid}\n"
                        "maintenance_invocations=1\n"
                        "maintenance_outcome=completed\n"
                        f"receipt={'0' * 64}|{receipt}\n"
                        "acceptance_cli_executed=false\n"
                        "user_xdg_modified=false\n"
                        f"clone_final_config={candidate.config_sha256}\n"
                        f"clone_final_efi={candidate.efi_sha256}\n"
                        f"clone_final_qcow2={candidate.qcow2_sha256}|verified\n"
                        f"{role_fields}"
                    )
                    terminal.write_text(payload, encoding="utf-8")
                    retirement._validate_completed_operation_terminal(
                        terminal, candidate
                    )
                    terminal.write_text(
                        payload.replace(
                            candidate.qcow2_sha256, "0" * 64
                        ),
                        encoding="utf-8",
                    )
                    with self.assertRaises(retirement.RetirementPrepareError):
                        retirement._validate_completed_operation_terminal(
                            terminal, candidate
                        )

    def test_fourth_batch_semantics_bind_retained_and_stopped_terminals(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            fixture = Fixture.create(Path(temporary).resolve())
            original = fixture.batch.assets[0]
            retained = retirement.RetirementAsset(
                asset_id=original.asset_id,
                storage="utm-documents",
                relative_path=original.relative_path,
                name=original.name,
                uuid=original.uuid,
                role="upgrade-completed-terminal-with-retained-s3",
                config_sha256=original.config_sha256,
                efi_sha256=original.efi_sha256,
                qcow2_sha256=original.qcow2_sha256,
                qcow2_name=original.qcow2_name,
                evidence_anchors=original.evidence_anchors,
            )
            snapshot = {
                "format": "radishlex-linux-l6-local-snapshot-evidence-v1",
                "source_vm": {
                    "relative_path": f"UTM Documents/{retained.relative_path}",
                    "name": retained.name,
                    "uuid": retained.uuid,
                    "source_vm_started_for_snapshot": False,
                    "source_vm_stopped_before_snapshot": True,
                    "source_vm_stopped_after_snapshot": True,
                    "all_registered_vms_stopped_before_snapshot": True,
                    "all_registered_vms_stopped_after_snapshot": True,
                    "registered_running_vm_count_before_snapshot": 0,
                    "registered_running_vm_count_after_snapshot": 0,
                    "source_qcow_open_handles_before_snapshot": 0,
                    "snapshot_qcow_open_handles_before_publish": 0,
                },
                "snapshot": {
                    "relative_path": (
                        "RadishLex-L6-Snapshots/"
                        "S3-target-installed-80e49ce"
                    ),
                    "restorable_files": [
                        {
                            "path": "config.plist",
                            "sha256": retained.config_sha256,
                        },
                        {
                            "path": "Data/efi_vars.fd",
                            "sha256": retained.efi_sha256,
                        },
                        {
                            "path": f"Data/{retained.qcow2_name}",
                            "sha256": retained.qcow2_sha256,
                        },
                    ],
                },
            }
            retirement._validate_retained_terminal_snapshot(snapshot, retained)
            snapshot["source_vm"]["uuid"] = PEER_UUID
            with self.assertRaises(retirement.RetirementPrepareError):
                retirement._validate_retained_terminal_snapshot(snapshot, retained)

            prepared = retirement.RetirementAsset(
                **{
                    **retained.__dict__,
                    "role": "install-prepared-completed-stopped",
                }
            )
            stopped = Path(temporary) / "stopped.evidence.txt"
            stopped.write_text(
                "format=radishlex-linux-l6-host-stop-freeze-v1\n"
                f"vm_uuid={prepared.uuid}\n"
                "registered_vm_state=all_stopped\n"
                "stop_kind=normal_host_request\n"
                f"terminal_manifest_sha256={'0' * 64}\n"
                f"config_sha256={prepared.config_sha256}\n"
                f"efi_sha256={prepared.efi_sha256}\n"
                f"qcow2_sha256={prepared.qcow2_sha256}\n"
                "qcow2_hash_passes=2\n"
                "controlled_file_open_handles=0\n"
                "guest_commands_after_terminal=0\n"
                "maintenance_invocations_after_terminal=0\n"
                "next_case_started=false\n",
                encoding="utf-8",
            )
            retirement._validate_install_prepared_stopped_terminal(
                stopped, prepared
            )
            stopped.write_text(
                stopped.read_text(encoding="utf-8").replace(
                    "registered_vm_state=all_stopped",
                    "registered_vm_state=started",
                ),
                encoding="utf-8",
            )
            with self.assertRaises(retirement.RetirementPrepareError):
                retirement._validate_install_prepared_stopped_terminal(
                    stopped, prepared
                )

            staged = retirement.RetirementAsset(
                **{
                    **retained.__dict__,
                    "role": "install-artifacts-staged-completed-stopped",
                }
            )
            terminal = {
                "format": (
                    "radishlex-linux-l6-utm-install-artifacts-staged-"
                    "terminal-stop-v1"
                ),
                "target_name": staged.name,
                "target_uuid": staged.uuid,
                "outcome": "stopped-verified",
                "reason": "single-graceful-request-and-all-stopped-cross-check",
                "transaction": "completed-frozen-before-stop",
                "registered_status_preflight": "started",
                "graceful_stop_invocations": 1,
                "maintenance_resume_invocations": 0,
                "guest_exec_invocations": 0,
                "file_push_invocations": 0,
                "file_pull_invocations": 0,
                "target_handles_terminal": "absent",
                "terminal_relevant_host_process_count": 0,
                "operation_id": "not-read-or-generated",
                "automatic_retry": "not-performed",
                "automatic_cleanup": "not-performed",
                "automatic_force": "not-performed",
                "automatic_kill": "not-performed",
                "automatic_quit": "not-performed",
                "automatic_repair": "not-performed",
            }
            retirement._validate_install_artifacts_staged_terminal_stop(
                terminal, staged
            )
            disk = {
                "format": "radishlex-linux-l6-utm-guest-network-ready-v1",
                "target_package_name": staged.relative_path,
                "target_package_path_sha256": "0" * 64,
                "target_config_sha256": staged.config_sha256,
                "target_descriptors": {
                    "config": {"mode": "0644", "size": 3004},
                    "efi": {"mode": "0644", "size": 655360},
                    "qcow2": {"mode": "0644", "size": 10089988096},
                },
            }
            retirement._validate_install_artifacts_staged_stopped_disk(
                disk, staged
            )
            terminal["automatic_retry"] = "performed"
            with self.assertRaises(retirement.RetirementPrepareError):
                retirement._validate_install_artifacts_staged_terminal_stop(
                    terminal, staged
                )

    def test_frozen_disk_identity_binds_uuid_and_current_hashes(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            fixture = Fixture.create(Path(temporary).resolve())
            asset = fixture.batch.assets[0]
            evidence = Path(temporary) / "postverify.evidence.txt"
            payload = (
                "format=radishlex-linux-l6-d75818f-v3-start-failure-postverify-v1\n"
                f"target_uuid={asset.uuid}\n"
                "registered_vms=all-stopped\n"
                "target_vm=stopped\n"
                "source_target_handles=0\n"
                "terminal=failed-closed-stopped\n"
                "postverify=failed-closed-preserved\n"
                f"target_config_sha256={asset.config_sha256}\n"
                f"target_efi_sha256={asset.efi_sha256}\n"
                f"target_qcow2_sha256={asset.qcow2_sha256}\n"
            )
            evidence.write_text(payload, encoding="utf-8")

            retirement._validate_frozen_disk_identity(evidence, asset)
            evidence.write_text(
                payload.replace(asset.qcow2_sha256, "0" * 64),
                encoding="utf-8",
            )
            with self.assertRaises(retirement.RetirementPrepareError):
                retirement._validate_frozen_disk_identity(evidence, asset)

    def test_failed_start_terminal_binds_target_and_zero_mutation(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            fixture = Fixture.create(Path(temporary).resolve())
            asset = fixture.batch.assets[0]
            value = {
                "format": "radishlex-linux-l6-utm-start-once-v1",
                "clone_name": asset.name,
                "clone_uuid": asset.uuid,
                "outcome": "failed-closed-stopped",
                "start_invocations": 1,
                "automatic_retry": "not-performed",
                "automatic_stop": "not-performed",
                "guest_exec": "not-performed",
                "input_transfer": "not-performed",
                "operation_id": "not-generated",
                "transaction": "not-performed",
            }

            retirement._validate_utm_start_terminal(value, asset)
            value["guest_exec"] = "performed"
            with self.assertRaises(retirement.RetirementPrepareError):
                retirement._validate_utm_start_terminal(value, asset)

    def test_completed_noop_repair_terminal_binds_shutdown_identity(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            fixture = Fixture.create(Path(temporary).resolve())
            asset = fixture.batch.assets[0]
            value = {
                "format": (
                    "radishlex-linux-l6-maintenance-refresh-repair-"
                    "completed-noop-local-evidence-v1"
                ),
                "clone": {
                    "name": asset.name,
                    "uuid": asset.uuid,
                    "after_shutdown": {
                        "config_sha256": asset.config_sha256,
                        "efi_sha256": asset.efi_sha256,
                        "qcow2_sha256": asset.qcow2_sha256,
                        "qcow2_open_handles": 0,
                    },
                },
                "terminal": {
                    "maintenance_outcome": "completed",
                    "terminal_classification": (
                        "completed_without_package_reapply"
                    ),
                    "dpkg_mutation_executed": False,
                },
                "virtual_machines": {"all_stopped_after": True},
            }

            retirement._validate_maintenance_repair_terminal(value, asset)
            value["terminal"]["dpkg_mutation_executed"] = True
            with self.assertRaises(retirement.RetirementPrepareError):
                retirement._validate_maintenance_repair_terminal(value, asset)

    def test_install_prepared_terminal_requires_failed_closed_no_dpkg(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            evidence = Path(temporary) / "terminal.evidence.txt"
            payload = (
                "format=radishlex-linux-l6-install-prepared-failed-closed-host-summary-v1\n"
                "case=install_prepared\n"
                "acceptance_invocations=1\n"
                "checkpoint_count=1\n"
                "dpkg_mutation=not_started\n"
                "resume_invocations=0\n"
                "postflight_invocations=0\n"
                "outcome=failed-closed\n"
            )
            evidence.write_text(payload, encoding="utf-8")

            retirement._validate_install_prepared_failed_closed(evidence)
            evidence.write_text(
                payload.replace("dpkg_mutation=not_started", "dpkg_mutation=started"),
                encoding="utf-8",
            )
            with self.assertRaises(retirement.RetirementPrepareError):
                retirement._validate_install_prepared_failed_closed(evidence)


class Fixture:
    def __init__(
        self,
        *,
        request: retirement.PrepareRequest,
        batch: retirement.RetirementBatch,
        anchor: retirement.EvidenceAnchor,
        anchor_path: Path,
        config_path: Path,
        efi_path: Path,
        qcow2_path: Path,
    ) -> None:
        self.request = request
        self.batch = batch
        self.anchor = anchor
        self.anchor_path = anchor_path
        self.config_path = config_path
        self.efi_path = efi_path
        self.qcow2_path = qcow2_path

    @classmethod
    def create(cls, root: Path) -> Fixture:
        root.mkdir(mode=0o755, parents=True, exist_ok=True)
        repository = root / "repo"
        operator = root / "operator"
        utm_documents = root / "utm-documents"
        repository.mkdir(mode=0o755)
        operator.mkdir(mode=0o755)
        utm_documents.mkdir(mode=0o755)
        bundle = operator / f"{ASSET_NAME}.utm"
        data = bundle / "Data"
        data.mkdir(mode=0o755, parents=True)
        bundle.chmod(0o755)
        data.chmod(0o755)
        config = bundle / "config.plist"
        efi = data / "efi_vars.fd"
        qcow2 = data / QCOW2_NAME
        config.write_bytes(
            plistlib.dumps(
                {
                    "Information": {"Name": ASSET_NAME, "UUID": ASSET_UUID},
                    "Drive": [{"ImageType": "Disk", "ImageName": QCOW2_NAME}],
                }
            )
        )
        efi.write_bytes(b"synthetic-efi")
        qcow2.write_bytes(b"synthetic-qcow2")
        for path in (config, efi, qcow2):
            path.chmod(0o644)
        hashes = (sha256_file(config), sha256_file(efi), sha256_file(qcow2))
        anchor_dir = operator / "anchors"
        anchor_dir.mkdir(mode=0o755)
        anchor_path = anchor_dir / "snapshot.json"
        anchor_payload = {
            "format": "radishlex-linux-l6-local-snapshot-evidence-v1",
            "source_vm": {
                "relative_path": f"{ASSET_NAME}.utm",
                "name": ASSET_NAME,
                "uuid": ASSET_UUID,
            },
            "snapshot": {
                "restorable_files": [
                    {"path": "config.plist", "sha256": hashes[0]},
                    {"path": "Data/efi_vars.fd", "sha256": hashes[1]},
                    {"path": f"Data/{QCOW2_NAME}", "sha256": hashes[2]},
                ]
            },
        }
        anchor_path.write_text(
            json.dumps(anchor_payload, sort_keys=True) + "\n", encoding="utf-8"
        )
        anchor_path.chmod(0o600)
        anchor = retirement.EvidenceAnchor(
            storage="operator",
            relative_path="anchors/snapshot.json",
            sha256=sha256_file(anchor_path),
            mode=0o600,
            semantic="snapshot-source-vm",
        )
        asset = retirement.RetirementAsset(
            asset_id=ASSET_ID,
            storage="operator",
            relative_path=f"{ASSET_NAME}.utm",
            name=ASSET_NAME,
            uuid=ASSET_UUID,
            role="synthetic",
            config_sha256=hashes[0],
            efi_sha256=hashes[1],
            qcow2_sha256=hashes[2],
            qcow2_name=QCOW2_NAME,
            evidence_anchors=(anchor,),
        )
        batch = retirement.RetirementBatch(
            batch_id="synthetic-batch",
            asset_ids=(ASSET_ID,),
            accounting_gib="0.01",
            assets=(asset,),
        )
        request = retirement.PrepareRequest(
            repository_root=repository,
            expected_repository_head="a" * 40,
            output_root=operator / "retirement-prepare-evidence",
            attempt_id="synthetic-attempt",
            operator_asset_root=operator,
            utm_documents_root=utm_documents,
            expected_allowlist_sha256="b" * 64,
            batch_id=batch.batch_id,
            expected_vm_count=2,
            expected_inventory_sha256=inventory_sha256(inventory()),
            command_timeout_seconds=15,
            authorized_read_only_prepare=True,
            authorized_one_utmctl_list=True,
            authorized_two_handle_rounds=True,
            authorized_no_delete_start_clone_move_or_guest=True,
        )
        return cls(
            request=request,
            batch=batch,
            anchor=anchor,
            anchor_path=anchor_path,
            config_path=config,
            efi_path=efi,
            qcow2_path=qcow2,
        )

    def binding(self, request: retirement.PrepareRequest) -> retirement.BindingResult:
        return retirement.BindingResult(
            evidence={
                "allowlist_asset_count": len(self.batch.assets),
                "allowlist_sha256": request.expected_allowlist_sha256,
                "batch_accounting_gib": self.batch.accounting_gib,
                "batch_id": self.batch.batch_id,
                "control_sha256": "d" * 64,
                "format": retirement.EVIDENCE_FORMAT,
                "repository_clean": True,
                "repository_head": request.expected_repository_head,
            },
            batch=self.batch,
        )

    def identities(self) -> tuple[retirement.BundleIdentity, ...]:
        return retirement.read_batch_identities(self.request, self.batch)

    def request_with(self, **overrides: object) -> retirement.PrepareRequest:
        values = dict(self.request.__dict__)
        values.update(overrides)
        return retirement.PrepareRequest(**values)  # type: ignore[arg-type]

    def asset_with_anchor(
        self, anchor: retirement.EvidenceAnchor
    ) -> retirement.RetirementAsset:
        values = dict(self.batch.assets[0].__dict__)
        values["evidence_anchors"] = (anchor,)
        return retirement.RetirementAsset(**values)  # type: ignore[arg-type]

    def allowlist_asset(self) -> dict[str, object]:
        asset = self.batch.assets[0]
        return {
            "id": asset.asset_id,
            "kind": "utm-vm-bundle",
            "action": "evidence-archived-delete-candidate",
            "storage": asset.storage,
            "relative_path": asset.relative_path,
            "name": asset.name,
            "uuid": asset.uuid,
            "role": asset.role,
            "config_sha256": asset.config_sha256,
            "efi_sha256": asset.efi_sha256,
            "qcow2_sha256": asset.qcow2_sha256,
            "qcow2_name": asset.qcow2_name,
            "evidence_anchors": [
                {
                    "storage": self.anchor.storage,
                    "relative_path": self.anchor.relative_path,
                    "sha256": self.anchor.sha256,
                    "mode": "0600",
                    "semantic": self.anchor.semantic,
                }
            ],
        }


def observation(
    argv: tuple[str, ...],
    *,
    exit_code: int | None = 0,
    timed_out: bool = False,
    stdout: bytes = b"",
    stderr: bytes = b"",
) -> clone_control.CommandObservation:
    return clone_control.CommandObservation.from_bytes(
        argv,
        exit_code=exit_code,
        timed_out=timed_out,
        stdout=stdout,
        stderr=stderr,
    )


def inventory(*, asset_status: str = "stopped") -> bytes:
    return (
        "UUID Status Name\n"
        f"{PEER_UUID} stopped Synthetic-Peer\n"
        f"{ASSET_UUID} {asset_status} {ASSET_NAME}\n"
    ).encode("utf-8")


def inventory_sha256(value: bytes) -> str:
    registered = clone_control.parse_utmctl_list(
        observation(("utmctl", "list"), stdout=value)
    )
    return clone_control.canonical_inventory_sha256(registered)


def sha256_file(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def read_json(path: Path) -> dict[str, object]:
    return json.loads(path.read_text(encoding="utf-8"))


def assert_manifest_valid(test: unittest.TestCase, root: Path) -> None:
    lines = (root / "files.sha256").read_text(encoding="ascii").splitlines()
    test.assertGreaterEqual(len(lines), 1)
    for line in lines:
        expected_hash, name = line.split("  ", maxsplit=1)
        test.assertEqual(sha256_file(root / name), expected_hash)


if __name__ == "__main__":
    unittest.main()
