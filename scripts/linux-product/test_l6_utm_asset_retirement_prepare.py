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
