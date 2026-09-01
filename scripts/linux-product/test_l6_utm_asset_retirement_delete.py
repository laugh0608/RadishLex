#!/usr/bin/env python3
from __future__ import annotations

import hashlib
import shutil
import tempfile
import unittest
from pathlib import Path

import l6_utm_asset_retirement_delete as deletion
import l6_utm_asset_retirement_prepare as prepare
import l6_utm_clone_once as clone_control
import test_l6_utm_asset_retirement_prepare as prepare_test


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


class LinuxL6AssetRetirementDeleteTests(unittest.TestCase):
    def test_deleted_requires_exact_delta_and_absent_package(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            fixture = DeleteFixture.create(Path(temporary).resolve())
            handles = fixture.handles_argv()
            delete_argv = ("utmctl", "delete", prepare_test.ASSET_UUID)
            runner = FakeRunner(
                [
                    observation(("utmctl", "list"), stdout=predelete_inventory()),
                    observation(handles, exit_code=1),
                    observation(handles, exit_code=1),
                    observation(delete_argv),
                    observation(("utmctl", "list"), stdout=postdelete_inventory()),
                ],
                after_call={4: fixture.remove_bundle},
            )

            result = deletion.run_delete(
                fixture.request,
                runner=runner,
                binding_validator=fixture.binding,
            )

            self.assertEqual(result.outcome, "deleted")
            self.assertEqual(result.exit_code, deletion.EXIT_DELETED)
            self.assertEqual(result.inventory_queries, 2)
            self.assertEqual(result.delete_invocations, 1)
            self.assertEqual(result.deleted_assets, 1)
            self.assertEqual(runner.calls.count(delete_argv), 1)
            terminal = read_json(fixture.request.output_root / "terminal.json")
            self.assertEqual(terminal["deleted_asset_ids"], [prepare_test.ASSET_ID])
            self.assertEqual(terminal["automatic_retry"], "not-performed")
            self.assertEqual(terminal["automatic_rollback"], "not-performed")
            self.assertEqual(terminal["start"], "not-performed")
            self.assertEqual(terminal["clone"], "not-performed")
            self.assertEqual(terminal["move"], "not-performed")
            self.assertEqual(terminal["guest"], "not-entered")
            assert_manifest_valid(self, fixture.request.output_root)

    def test_delete_failure_is_indeterminate_and_never_retried(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            fixture = DeleteFixture.create(Path(temporary).resolve())
            handles = fixture.handles_argv()
            delete_argv = ("utmctl", "delete", prepare_test.ASSET_UUID)
            runner = FakeRunner(
                [
                    observation(("utmctl", "list"), stdout=predelete_inventory()),
                    observation(handles, exit_code=1),
                    observation(handles, exit_code=1),
                    observation(delete_argv, exit_code=1, stderr=b"synthetic\n"),
                    observation(("utmctl", "list"), stdout=predelete_inventory()),
                ]
            )

            result = deletion.run_delete(
                fixture.request,
                runner=runner,
                binding_validator=fixture.binding,
            )

            self.assertEqual(result.outcome, "state-indeterminate")
            self.assertEqual(result.exit_code, deletion.EXIT_STATE_INDETERMINATE)
            self.assertEqual(result.delete_invocations, 1)
            self.assertEqual(result.deleted_assets, 0)
            self.assertEqual(runner.calls.count(delete_argv), 1)
            self.assertTrue(fixture.bundle.exists())
            terminal = read_json(fixture.request.output_root / "terminal.json")
            self.assertIn("delete-command-not-clean-success", terminal["reason"])

    def test_registry_removed_but_package_present_stops_indeterminate(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            fixture = DeleteFixture.create(Path(temporary).resolve())
            handles = fixture.handles_argv()
            delete_argv = ("utmctl", "delete", prepare_test.ASSET_UUID)
            runner = FakeRunner(
                [
                    observation(("utmctl", "list"), stdout=predelete_inventory()),
                    observation(handles, exit_code=1),
                    observation(handles, exit_code=1),
                    observation(delete_argv),
                    observation(("utmctl", "list"), stdout=postdelete_inventory()),
                ]
            )

            result = deletion.run_delete(
                fixture.request,
                runner=runner,
                binding_validator=fixture.binding,
            )

            self.assertEqual(result.outcome, "state-indeterminate")
            self.assertEqual(result.deleted_assets, 0)
            terminal = read_json(fixture.request.output_root / "terminal.json")
            self.assertIn("postdelete-package-not-absent", terminal["reason"])

    def test_package_removed_but_registry_present_stops_indeterminate(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            fixture = DeleteFixture.create(Path(temporary).resolve())
            handles = fixture.handles_argv()
            delete_argv = ("utmctl", "delete", prepare_test.ASSET_UUID)
            runner = FakeRunner(
                [
                    observation(("utmctl", "list"), stdout=predelete_inventory()),
                    observation(handles, exit_code=1),
                    observation(handles, exit_code=1),
                    observation(delete_argv),
                    observation(("utmctl", "list"), stdout=predelete_inventory()),
                ],
                after_call={4: fixture.remove_bundle},
            )

            result = deletion.run_delete(
                fixture.request,
                runner=runner,
                binding_validator=fixture.binding,
            )

            self.assertEqual(result.outcome, "state-indeterminate")
            self.assertEqual(result.deleted_assets, 0)
            terminal = read_json(fixture.request.output_root / "terminal.json")
            self.assertIn("inventory-count-delta-invalid", terminal["reason"])

    def test_active_inventory_rejects_before_delete(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            fixture = DeleteFixture.create(Path(temporary).resolve())
            running = predelete_inventory(asset_status="started")
            runner = FakeRunner(
                [observation(("utmctl", "list"), stdout=running)]
            )

            result = deletion.run_delete(
                fixture.request,
                runner=runner,
                binding_validator=fixture.binding,
            )

            self.assertEqual(result.outcome, "precondition-rejected")
            self.assertEqual(result.delete_invocations, 0)
            self.assertEqual(runner.calls, [("utmctl", "list")])
            self.assertTrue(fixture.bundle.exists())

    def test_all_destructive_authorizations_are_required_before_output(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            for field in (
                "authorized_delete_first_four_v1",
                "authorized_at_most_one_delete_per_asset",
                "authorized_stop_without_retry_or_rollback",
                "acknowledge_irreversible_bundle_removal",
            ):
                fixture = DeleteFixture.create(root / field)
                request = fixture.request_with(**{field: False})
                with self.assertRaises(deletion.RetirementDeleteError):
                    deletion.run_delete(
                        request,
                        runner=FakeRunner([]),
                        binding_validator=fixture.binding,
                    )
                self.assertFalse(request.output_root.exists())

    def test_later_batches_use_distinct_authorization_and_terminal_reason(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            for batch_id, authorization in (
                ("second-batch-v1", "delete_second_batch_v1"),
                ("third-batch-v1", "delete_third_batch_v1"),
                ("fourth-batch-v1", "delete_fourth_batch_v1"),
            ):
                with self.subTest(batch_id=batch_id):
                    fixture = DeleteFixture.create(
                        root / batch_id, batch_id=batch_id
                    )
                    handles = fixture.handles_argv()
                    delete_argv = ("utmctl", "delete", prepare_test.ASSET_UUID)
                    runner = FakeRunner(
                        [
                            observation(
                                ("utmctl", "list"), stdout=predelete_inventory()
                            ),
                            observation(handles, exit_code=1),
                            observation(handles, exit_code=1),
                            observation(delete_argv),
                            observation(
                                ("utmctl", "list"), stdout=postdelete_inventory()
                            ),
                        ],
                        after_call={4: fixture.remove_bundle},
                    )

                    result = deletion.run_delete(
                        fixture.request,
                        runner=runner,
                        binding_validator=fixture.binding,
                    )

                    self.assertEqual(result.outcome, "deleted")
                    request = read_json(
                        fixture.request.output_root / "request.json"
                    )
                    self.assertEqual(
                        request["authorization"],
                        {
                            "acknowledge_irreversible_bundle_removal": True,
                            "at_most_one_delete_per_asset": True,
                            authorization: True,
                            "stop_without_retry_or_rollback": True,
                        },
                    )
                    terminal = read_json(
                        fixture.request.output_root / "terminal.json"
                    )
                    self.assertEqual(
                        terminal["reason"], f"{batch_id}-deleted-and-verified"
                    )

    def test_batch_authorization_must_match_before_output(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            second = DeleteFixture.create(root / "second", batch_id="second-batch-v1")
            wrong_second = second.request_with(
                authorized_delete_first_four_v1=True,
                authorized_delete_second_batch_v1=False,
            )
            with self.assertRaises(deletion.RetirementDeleteError):
                deletion.run_delete(wrong_second, runner=FakeRunner([]))
            self.assertFalse(wrong_second.output_root.exists())

            first = DeleteFixture.create(root / "first")
            ambiguous_first = first.request_with(
                authorized_delete_second_batch_v1=True
            )
            with self.assertRaises(deletion.RetirementDeleteError):
                deletion.run_delete(ambiguous_first, runner=FakeRunner([]))
            self.assertFalse(ambiguous_first.output_root.exists())

            third = DeleteFixture.create(root / "third", batch_id="third-batch-v1")
            wrong_third = third.request_with(
                authorized_delete_second_batch_v1=True,
                authorized_delete_third_batch_v1=False,
            )
            with self.assertRaises(deletion.RetirementDeleteError):
                deletion.run_delete(wrong_third, runner=FakeRunner([]))
            self.assertFalse(wrong_third.output_root.exists())

            fourth = DeleteFixture.create(
                root / "fourth", batch_id="fourth-batch-v1"
            )
            wrong_fourth = fourth.request_with(
                authorized_delete_third_batch_v1=True,
                authorized_delete_fourth_batch_v1=False,
            )
            with self.assertRaises(deletion.RetirementDeleteError):
                deletion.run_delete(wrong_fourth, runner=FakeRunner([]))
            self.assertFalse(wrong_fourth.output_root.exists())

    def test_fifth_and_unknown_batches_are_rejected_before_output(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            for batch_id in ("fifth-batch-v1", "unknown-batch-v1"):
                with self.subTest(batch_id=batch_id):
                    fixture = DeleteFixture.create(root / batch_id)
                    request = fixture.request_with(batch_id=batch_id)
                    with self.assertRaises(deletion.RetirementDeleteError):
                        deletion.run_delete(request, runner=FakeRunner([]))
                    self.assertFalse(request.output_root.exists())

    def test_prior_prepare_is_recursively_bound_and_rejects_extra_entry(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            for batch_id in (
                "first-four-v1",
                "second-batch-v1",
                "third-batch-v1",
                "fourth-batch-v1",
            ):
                fixture = DeleteFixture.create(root / batch_id, batch_id=batch_id)
                prior_root = fixture.request.prior_prepare_root
                prepare_request = fixture.prepare_request(prior_root)
                handles = fixture.handles_argv()
                result = prepare.run_prepare(
                    prepare_request,
                    runner=prepare_test.FakeRunner(
                        [
                            observation(
                                ("utmctl", "list"), stdout=predelete_inventory()
                            ),
                            observation(handles, exit_code=1),
                            observation(handles, exit_code=1),
                        ]
                    ),
                    binding_validator=fixture.prepare_binding,
                )
                self.assertEqual(result.outcome, "prepared")
                fixture.request = fixture.request_with(
                    prior_prepare_manifest_sha256=result.manifest_sha256
                )

                self.assertEqual(
                    deletion.validate_prior_prepare(
                        fixture.request, fixture.batch
                    ),
                    9,
                )
                extra = prior_root / "unexpected.json"
                extra.write_text("{}\n", encoding="utf-8")
                extra.chmod(0o600)
                with self.assertRaises(deletion.RetirementDeleteError):
                    deletion.validate_prior_prepare(
                        fixture.request, fixture.batch
                    )


class DeleteFixture:
    def __init__(
        self,
        fixture: prepare_test.Fixture,
        request: deletion.DeleteRequest,
        batch: prepare.RetirementBatch,
    ) -> None:
        self.fixture = fixture
        self.request = request
        self.batch = batch

    @classmethod
    def create(
        cls, root: Path, *, batch_id: str = "first-four-v1"
    ) -> DeleteFixture:
        fixture = prepare_test.Fixture.create(root)
        batch = prepare.RetirementBatch(
            batch_id=batch_id,
            asset_ids=fixture.batch.asset_ids,
            accounting_gib=fixture.batch.accounting_gib,
            assets=fixture.batch.assets,
        )
        request = deletion.DeleteRequest(
            repository_root=fixture.request.repository_root,
            expected_repository_head="a" * 40,
            output_root=fixture.request.operator_asset_root / "delete-evidence",
            attempt_id="synthetic-delete-attempt",
            operator_asset_root=fixture.request.operator_asset_root,
            utm_documents_root=fixture.request.utm_documents_root,
            expected_allowlist_sha256="b" * 64,
            batch_id=batch_id,
            prior_prepare_root=(
                fixture.request.operator_asset_root / "prior-prepare-evidence"
            ),
            prior_prepare_manifest_sha256="c" * 64,
            expected_predelete_vm_count=2,
            expected_predelete_inventory_sha256=inventory_sha256(
                predelete_inventory()
            ),
            command_timeout_seconds=15,
            delete_timeout_seconds=60,
            authorized_delete_first_four_v1=batch_id == "first-four-v1",
            authorized_delete_second_batch_v1=batch_id == "second-batch-v1",
            authorized_delete_third_batch_v1=batch_id == "third-batch-v1",
            authorized_delete_fourth_batch_v1=batch_id == "fourth-batch-v1",
            authorized_at_most_one_delete_per_asset=True,
            authorized_stop_without_retry_or_rollback=True,
            acknowledge_irreversible_bundle_removal=True,
        )
        return cls(fixture, request, batch)

    @property
    def bundle(self) -> Path:
        return self.fixture.config_path.parent

    def binding(self, request: deletion.DeleteRequest) -> deletion.DeleteBinding:
        return deletion.DeleteBinding(
            evidence={
                "allowlist_asset_count": len(self.batch.assets),
                "allowlist_sha256": request.expected_allowlist_sha256,
                "batch_accounting_gib": self.batch.accounting_gib,
                "batch_id": self.batch.batch_id,
                "control_sha256": "d" * 64,
                "format": deletion.EVIDENCE_FORMAT,
                "prior_prepare_entries_verified": 9,
                "prior_prepare_manifest_sha256": (
                    request.prior_prepare_manifest_sha256
                ),
                "repository_clean": True,
                "repository_head": request.expected_repository_head,
            },
            batch=self.batch,
        )

    def prepare_binding(
        self, request: prepare.PrepareRequest
    ) -> prepare.BindingResult:
        return prepare.BindingResult(
            evidence={
                "allowlist_asset_count": len(self.batch.assets),
                "allowlist_sha256": request.expected_allowlist_sha256,
                "batch_accounting_gib": self.batch.accounting_gib,
                "batch_id": self.batch.batch_id,
                "control_sha256": "d" * 64,
                "format": prepare.EVIDENCE_FORMAT,
                "repository_clean": True,
                "repository_head": request.expected_repository_head,
            },
            batch=self.batch,
        )

    def handles_argv(self) -> tuple[str, ...]:
        return prepare._lsof_argv(
            prepare.read_batch_identities(
                self.request.prepare_request(), self.batch
            )
        )

    def remove_bundle(self) -> None:
        shutil.rmtree(self.bundle)

    def request_with(self, **overrides: object) -> deletion.DeleteRequest:
        values = dict(self.request.__dict__)
        values.update(overrides)
        return deletion.DeleteRequest(**values)  # type: ignore[arg-type]

    def prepare_request(self, output_root: Path) -> prepare.PrepareRequest:
        value = self.request.prepare_request()
        values = dict(value.__dict__)
        values["output_root"] = output_root
        return prepare.PrepareRequest(**values)  # type: ignore[arg-type]


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


def predelete_inventory(*, asset_status: str = "stopped") -> bytes:
    return (
        "UUID Status Name\n"
        f"{prepare_test.PEER_UUID} stopped Synthetic-Peer\n"
        f"{prepare_test.ASSET_UUID} {asset_status} {prepare_test.ASSET_NAME}\n"
    ).encode("utf-8")


def postdelete_inventory() -> bytes:
    return (
        "UUID Status Name\n"
        f"{prepare_test.PEER_UUID} stopped Synthetic-Peer\n"
    ).encode("utf-8")


def inventory_sha256(value: bytes) -> str:
    registered = clone_control.parse_utmctl_list(
        observation(("utmctl", "list"), stdout=value)
    )
    return clone_control.canonical_inventory_sha256(registered)


def read_json(path: Path) -> dict[str, object]:
    import json

    return json.loads(path.read_text(encoding="utf-8"))


def assert_manifest_valid(test: unittest.TestCase, root: Path) -> None:
    lines = (root / "files.sha256").read_text(encoding="ascii").splitlines()
    test.assertGreaterEqual(len(lines), 1)
    for line in lines:
        expected_hash, name = line.split("  ", maxsplit=1)
        actual_hash = hashlib.sha256((root / name).read_bytes()).hexdigest()
        test.assertEqual(actual_hash, expected_hash)


if __name__ == "__main__":
    unittest.main()
