#!/usr/bin/env python3
from __future__ import annotations

import hashlib
import json
import tempfile
import unittest
from pathlib import Path
from unittest import mock

import l6_v4_install_artifacts_staged_resume_driver as driver


RAW_SECRET = "0" * 32


class LinuxL6InstallArtifactsStagedResumeDriverTests(unittest.TestCase):
    def test_terminal_case_exposes_only_resume_and_postflight(self) -> None:
        source = b"prefix\n" + driver.ORIGINAL_DISPATCH

        rendered = driver.derive_terminal_case_bytes(source)

        self.assertNotIn(driver.ORIGINAL_DISPATCH, rendered)
        self.assertEqual(rendered.count(driver.TERMINAL_DISPATCH), 1)
        self.assertNotIn(b"archive) archive", rendered)
        self.assertNotIn(b"retry", rendered)

    def test_terminal_case_rejects_preexisting_terminal_dispatch(self) -> None:
        with self.assertRaisesRegex(
            driver.ResumeDriverError, "canonical-case-dispatch-invalid"
        ):
            driver.derive_terminal_case_bytes(driver.TERMINAL_DISPATCH)

    def test_receipt_validation_keeps_raw_secret_guest_local(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            run = root / "run"
            state = root / "state"
            operations = state / "operations"
            operation = operations / RAW_SECRET
            run.mkdir()
            operation.mkdir(parents=True)
            (run / "operation-id.secret").write_text(
                RAW_SECRET, encoding="ascii"
            )
            (operation / "target.deb").write_bytes(b"synthetic")
            (operation / "target.evidence.json").write_bytes(b"synthetic")
            receipt = staged_receipt(RAW_SECRET)
            receipt_path = state / "receipt.json"
            receipt_path.write_text(json.dumps(receipt), encoding="utf-8")

            with mock.patch.multiple(
                driver,
                RUN_ROOT=run,
                STATE_ROOT=state,
                RECEIPT_PATH=receipt_path,
                EXPECTED_OPERATION_ID_SHA256=hashlib.sha256(
                    RAW_SECRET.encode("ascii")
                ).hexdigest(),
            ), mock.patch.object(driver, "require_regular"), mock.patch.object(
                driver, "require_private_directory"
            ):
                driver.validate_secret_and_receipt()

    def test_receipt_validation_rejects_secret_hash_drift(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            run = root / "run"
            state = root / "state"
            operation = state / "operations" / RAW_SECRET
            run.mkdir()
            operation.mkdir(parents=True)
            (run / "operation-id.secret").write_text(
                RAW_SECRET, encoding="ascii"
            )
            (operation / "target.deb").write_bytes(b"synthetic")
            (operation / "target.evidence.json").write_bytes(b"synthetic")
            receipt_path = state / "receipt.json"
            receipt_path.write_text(
                json.dumps(staged_receipt(RAW_SECRET)), encoding="utf-8"
            )

            with mock.patch.multiple(
                driver,
                RUN_ROOT=run,
                STATE_ROOT=state,
                RECEIPT_PATH=receipt_path,
                EXPECTED_OPERATION_ID_SHA256="f" * 64,
            ), mock.patch.object(driver, "require_regular"), mock.patch.object(
                driver, "require_private_directory"
            ):
                with self.assertRaisesRegex(
                    driver.ResumeDriverError, "operation-secret-invalid"
                ):
                    driver.validate_secret_and_receipt()

    def test_main_runs_resume_and_postflight_once(self) -> None:
        calls: list[str] = []
        terminals: list[dict[str, object]] = []

        def run_case(_root, _case, phase):
            calls.append(phase)
            stdout = {
                "resume": b"install_artifacts_staged_resume_outcome=completed\n",
                "postflight": b"install_artifacts_staged_postflight_outcome=passed\n",
            }[phase]
            return type(
                "Observation",
                (),
                {"returncode": 0, "stdout": stdout, "stderr": b""},
            )()

        with tempfile.TemporaryDirectory() as temporary, mock.patch.object(
            driver.os, "geteuid", return_value=0
        ), mock.patch.object(
            driver, "EXPECTED_CONTROL_ROOT", Path(temporary)
        ), mock.patch.object(driver, "require_private_directory"), mock.patch.object(
            driver, "require_regular"
        ), mock.patch.object(
            driver, "sha256_file", return_value="a" * 64
        ), mock.patch.object(
            driver, "create_new_file"
        ), mock.patch.object(
            driver, "replace_phase"
        ), mock.patch.object(
            driver, "validate_frozen_checkpoint"
        ), mock.patch.object(
            driver, "validate_secret_and_receipt"
        ), mock.patch.object(
            driver, "validate_guard"
        ), mock.patch.object(
            driver, "validate_live_system"
        ), mock.patch.object(
            driver,
            "derive_terminal_case",
            return_value=(Path(temporary) / "terminal.sh", "b" * 64),
        ), mock.patch.object(
            driver, "run_case", side_effect=run_case
        ), mock.patch.object(
            driver,
            "validate_resume_result",
            return_value=completed_artifacts(),
        ), mock.patch.object(
            driver,
            "publish_terminal",
            side_effect=lambda _root, value: terminals.append(value),
        ):
            result = driver.main(valid_argv(Path(temporary), "a" * 64))

        self.assertEqual(result, 0)
        self.assertEqual(calls, ["resume", "postflight"])
        self.assertEqual(len(terminals), 1)
        self.assertEqual(terminals[0]["outcome"], "resume-completed")
        self.assertEqual(terminals[0]["maintenance_resume_invocations"], 1)
        self.assertNotIn(RAW_SECRET, json.dumps(terminals[0]))

    def test_main_failure_after_resume_is_indeterminate_without_retry(self) -> None:
        calls: list[str] = []
        terminals: list[dict[str, object]] = []

        def fail_resume(_root, _case, phase):
            calls.append(phase)
            raise driver.ResumeDriverError("synthetic-resume-observation-loss")

        with tempfile.TemporaryDirectory() as temporary, mock.patch.object(
            driver.os, "geteuid", return_value=0
        ), mock.patch.object(
            driver, "EXPECTED_CONTROL_ROOT", Path(temporary)
        ), mock.patch.object(driver, "require_private_directory"), mock.patch.object(
            driver, "require_regular"
        ), mock.patch.object(
            driver, "sha256_file", return_value="a" * 64
        ), mock.patch.object(
            driver, "create_new_file"
        ), mock.patch.object(
            driver, "replace_phase"
        ), mock.patch.object(
            driver, "validate_frozen_checkpoint"
        ), mock.patch.object(
            driver, "validate_secret_and_receipt"
        ), mock.patch.object(
            driver, "validate_guard"
        ), mock.patch.object(
            driver, "validate_live_system"
        ), mock.patch.object(
            driver,
            "derive_terminal_case",
            return_value=(Path(temporary) / "terminal.sh", "b" * 64),
        ), mock.patch.object(
            driver, "run_case", side_effect=fail_resume
        ), mock.patch.object(
            driver,
            "publish_terminal",
            side_effect=lambda _root, value: terminals.append(value),
        ):
            result = driver.main(valid_argv(Path(temporary), "a" * 64))

        self.assertEqual(result, 12)
        self.assertEqual(calls, ["resume"])
        self.assertEqual(terminals[0]["outcome"], "state-indeterminate")
        self.assertEqual(terminals[0]["maintenance_resume_invocations"], 1)
        self.assertEqual(terminals[0]["automatic_retry"], "not-performed")

    def test_driver_has_no_utmctl_retry_cleanup_or_stop_path(self) -> None:
        source = Path(driver.__file__).read_text(encoding="utf-8")
        self.assertNotIn("utmctl", source)
        self.assertNotIn('run_case(control_root, terminal_case, "retry")', source)
        self.assertNotIn('run_case(control_root, terminal_case, "archive")', source)
        self.assertIn('run_case(control_root, terminal_case, "resume")', source)
        self.assertIn('run_case(control_root, terminal_case, "postflight")', source)

    def test_unbound_control_root_is_rejected_without_terminal_write(self) -> None:
        with tempfile.TemporaryDirectory() as temporary, mock.patch.object(
            driver.os, "geteuid", return_value=0
        ), mock.patch.object(driver, "publish_terminal") as publish_terminal:
            result = driver.main(valid_argv(Path(temporary), "a" * 64))

        self.assertEqual(result, 10)
        publish_terminal.assert_not_called()

    def test_failure_reason_redacts_raw_operation_id(self) -> None:
        reason = driver.sanitize_reason(f"cannot-open:/run/{RAW_SECRET}/receipt")

        self.assertNotIn(RAW_SECRET, reason)
        self.assertIn("[redacted]", reason)


def staged_receipt(secret: str) -> dict[str, object]:
    artifact = {
        "package_version": "26.7.1+38-1",
        "package_sha256": driver.EXPECTED_SOURCE_PACKAGE_SHA256,
        "evidence_sha256": driver.EXPECTED_SOURCE_EVIDENCE_SHA256,
    }
    return {
        "receipt_format": "radishlex-linux-install-receipt-v1",
        "product_id": "radishlex-linux",
        "distribution_identity": "debian-local-deb-v1",
        "operation_chain": [secret],
        "operation_id": secret,
        "operation_kind": "install",
        "version_relation": "not_applicable",
        "state": "artifacts_staged",
        "failure_code": None,
        "failure_after_state": None,
        "manual_recovery_required": False,
        "source_artifact": None,
        "target_artifact": artifact,
        "initial_package": {"state": "not_installed", "artifact": None},
        "staged_artifacts": [{"slot": "target", "artifact": dict(artifact)}],
        "source_proof": None,
        "target_proof": None,
    }


def completed_artifacts() -> dict[str, object]:
    return {
        "dpkg_delta_sha256": "c" * 64,
        "dpkg_delta_size": 1512,
        "dpkg_log_sha256": "d" * 64,
        "dpkg_log_size": 881152,
        "receipt_sha256": "e" * 64,
        "receipt_size": 4000,
        "resume_result_sha256": "f" * 64,
        "terminal_postflight_sha256": "1" * 64,
    }


def valid_argv(control_root: Path, driver_sha256: str) -> list[str]:
    return [
        "--resume-attempt-id",
        driver.EXPECTED_RESUME_ATTEMPT_ID,
        "--checkpoint-attempt-id",
        driver.EXPECTED_CHECKPOINT_ATTEMPT_ID,
        "--target-uuid",
        driver.EXPECTED_TARGET_UUID,
        "--expected-boot-id-sha256",
        driver.EXPECTED_BOOT_ID_SHA256,
        "--expected-operation-id-sha256",
        driver.EXPECTED_OPERATION_ID_SHA256,
        "--expected-checkpoint-sha256",
        driver.EXPECTED_CHECKPOINT_SHA256,
        "--expected-crash-state-sha256",
        driver.EXPECTED_CRASH_STATE_SHA256,
        "--expected-driver-sha256",
        driver_sha256,
        "--control-root",
        str(control_root),
    ]


if __name__ == "__main__":
    unittest.main()
