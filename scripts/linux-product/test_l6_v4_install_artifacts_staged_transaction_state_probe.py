#!/usr/bin/env python3
from __future__ import annotations

import hashlib
import contextlib
import io
import json
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest import mock

import l6_v4_install_artifacts_staged_transaction_state_probe as probe


class TransactionStateProbeTests(unittest.TestCase):
    def test_receipt_state_dispatch_is_mutually_exclusive(self) -> None:
        completed = {"state": "completed"}
        staged = {"state": "artifacts_staged"}
        with mock.patch.object(
            probe, "_read_receipt", return_value=(completed, b"completed")
        ), mock.patch.object(
            probe, "_validate_completed", return_value=observed("completed")
        ) as validate_completed, mock.patch.object(
            probe, "_validate_artifacts_staged"
        ) as validate_staged, mock.patch.object(
            probe,
            "BOOT_ID_PATH",
            SimpleNamespace(read_text=lambda **_: "synthetic-boot\n"),
        ):
            value = probe.observe_transaction_state()

        self.assertEqual(value["transaction"], "completed")
        validate_completed.assert_called_once_with(completed, b"completed")
        validate_staged.assert_not_called()

        with mock.patch.object(
            probe, "_read_receipt", return_value=(staged, b"staged")
        ), mock.patch.object(
            probe, "_validate_completed"
        ) as validate_completed, mock.patch.object(
            probe,
            "_validate_artifacts_staged",
            return_value=observed("artifacts-staged"),
        ) as validate_staged, mock.patch.object(
            probe,
            "BOOT_ID_PATH",
            SimpleNamespace(read_text=lambda **_: "synthetic-boot\n"),
        ):
            value = probe.observe_transaction_state()

        self.assertEqual(value["transaction"], "artifacts-staged")
        validate_completed.assert_not_called()
        validate_staged.assert_called_once_with(staged, b"staged")

    def test_unknown_receipt_state_never_falls_back(self) -> None:
        with mock.patch.object(
            probe,
            "_read_receipt",
            return_value=({"state": "package_mutating"}, b"unknown"),
        ), mock.patch.object(
            probe, "_validate_completed"
        ) as completed, mock.patch.object(
            probe, "_validate_artifacts_staged"
        ) as staged, mock.patch.object(
            probe,
            "BOOT_ID_PATH",
            SimpleNamespace(read_text=lambda **_: "synthetic-boot\n"),
        ), self.assertRaisesRegex(
            probe.TransactionStateProbeError,
            "receipt-state-not-classifiable",
        ):
            probe.observe_transaction_state()

        completed.assert_not_called()
        staged.assert_not_called()

    def test_completed_and_staged_results_expose_only_operation_hash(self) -> None:
        for transaction in ("completed", "artifacts-staged"):
            value = probe.result_value(
                attempt_id=probe.EXPECTED_ATTEMPT_ID,
                target_uuid=probe.EXPECTED_TARGET_UUID,
                probe_sha256="a" * 64,
                observation={
                    "boot_id_sha256": "b" * 64,
                    **observed(transaction),
                },
                reason="read-only-transaction-state-observed",
            )
            encoded = probe.canonical_json(value)

            self.assertEqual(value["transaction"], transaction)
            self.assertEqual(value["operation_id"], "hash-only")
            self.assertNotIn(b"0" * 32, encoded)
            self.assertEqual(value["dpkg_mutation"], "not-performed")
            self.assertEqual(value["resume_invocations"], 0)
            self.assertEqual(value["product_state_write"], "not-performed")

    def test_validation_error_publishes_indeterminate_without_retry(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "control"
            root.mkdir(mode=0o700)
            source = Path(probe.__file__)
            probe_sha256 = hashlib.sha256(source.read_bytes()).hexdigest()
            argv = arguments(root, probe_sha256)

            with mock.patch.object(probe, "EXPECTED_CONTROL_ROOT", root), mock.patch.object(
                probe.os, "geteuid", return_value=0
            ), mock.patch.object(
                probe, "require_directory"
            ), mock.patch.object(
                probe, "require_regular"
            ), mock.patch.object(
                probe.os, "chown"
            ), mock.patch.object(
                probe,
                "observe_transaction_state",
                side_effect=probe.TransactionStateProbeError(
                    "receipt-drift"
                ),
            ):
                with contextlib.redirect_stderr(io.StringIO()):
                    exit_code = probe.main(argv)

            self.assertEqual(exit_code, 0)
            result = read_json(root / "transaction-state.evidence.json")
            self.assertEqual(result["outcome"], "state-indeterminate")
            self.assertEqual(result["maintenance_invocations"], 0)
            self.assertEqual(result["dpkg_mutation"], "not-performed")
            self.assertEqual(result["automatic_retry"], "not-performed")
            phase = read_json(root / "phase.json")
            self.assertEqual(phase["phase"], "state-indeterminate")

    def test_argument_contract_rejects_attempt_or_control_root_drift(self) -> None:
        valid = SimpleNamespace(
            attempt_id=probe.EXPECTED_ATTEMPT_ID,
            target_uuid=probe.EXPECTED_TARGET_UUID,
            expected_operation_id_sha256=probe.EXPECTED_OPERATION_ID_SHA256,
            expected_probe_sha256="a" * 64,
            control_root=probe.EXPECTED_CONTROL_ROOT,
        )
        probe.validate_argument_contract(valid)

        for key, value in (
            ("attempt_id", "wrong-attempt"),
            ("control_root", Path("/var/tmp/wrong-control-root")),
            ("expected_operation_id_sha256", "b" * 64),
        ):
            changed = SimpleNamespace(**vars(valid))
            setattr(changed, key, value)
            with self.assertRaisesRegex(
                probe.TransactionStateProbeError,
                "argument-contract-mismatch",
            ):
                probe.validate_argument_contract(changed)


def observed(transaction: str) -> dict[str, object]:
    return {
        "dpkg_log_sha256": "c" * 64,
        "dpkg_log_size": 42,
        "dpkg_status_sha256": "d" * 64,
        "guard_profile": "absent-after-reboot",
        "package_profile": (
            "installed-verified"
            if transaction == "completed"
            else "not-installed-staging-preserved"
        ),
        "receipt_sha256": "e" * 64,
        "receipt_size": 123,
        "startup_profile": (
            "allowed" if transaction == "completed" else "operation-in-progress"
        ),
        "transaction": transaction,
    }


def arguments(root: Path, probe_sha256: str) -> list[str]:
    return [
        "--attempt-id",
        probe.EXPECTED_ATTEMPT_ID,
        "--target-uuid",
        probe.EXPECTED_TARGET_UUID,
        "--expected-operation-id-sha256",
        probe.EXPECTED_OPERATION_ID_SHA256,
        "--expected-probe-sha256",
        probe_sha256,
        "--control-root",
        str(root),
    ]


def read_json(path: Path) -> dict[str, object]:
    value = json.loads(path.read_text(encoding="utf-8"))
    assert isinstance(value, dict)
    return value


if __name__ == "__main__":
    unittest.main()
