#!/usr/bin/env python3
from __future__ import annotations

import hashlib
import json
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest import mock

import l6_v4_install_artifacts_staged_fresh_boot_resume_driver as driver


RAW_SECRET = "0" * 32


class FreshBootResumeDriverTests(unittest.TestCase):
    def test_reconstructs_transient_secret_create_new_from_bound_receipt(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            run_root = Path(temporary) / "run-root"
            frozen = SimpleNamespace(
                RUN_ROOT=run_root,
                create_new_file=lambda path, payload, mode: path.write_bytes(payload),
                require_regular=lambda *args: None,
                sha256_bytes=lambda value: hashlib.sha256(value).hexdigest(),
            )
            with mock.patch.object(driver.os, "chown"), mock.patch.object(
                driver, "require_private_directory"
            ), mock.patch.object(driver, "sync_directory"), mock.patch.object(
                driver,
                "EXPECTED_OPERATION_ID_SHA256",
                hashlib.sha256(RAW_SECRET.encode("ascii")).hexdigest(),
            ):
                driver.reconstruct_transient_secret(frozen, RAW_SECRET)

            self.assertEqual(
                (run_root / "operation-id.secret").read_text(encoding="ascii"),
                RAW_SECRET,
            )

    def test_existing_transient_root_rejects_without_overwrite(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            run_root = Path(temporary) / "run-root"
            run_root.mkdir()
            frozen = SimpleNamespace(RUN_ROOT=run_root)
            with self.assertRaisesRegex(
                driver.FreshBootResumeDriverError,
                "transient-run-root-already-present",
            ):
                driver.reconstruct_transient_secret(frozen, RAW_SECRET)

    def test_main_runs_exactly_one_resume_and_postflight(self) -> None:
        calls: list[str] = []
        result, terminal = run_main_case(calls=calls)

        self.assertEqual(result, 0)
        self.assertEqual(calls, ["resume", "postflight"])
        self.assertEqual(terminal["outcome"], "resume-completed")
        self.assertEqual(terminal["maintenance_resume_invocations"], 1)
        self.assertEqual(terminal["postflight_invocations"], 1)
        self.assertNotIn(RAW_SECRET, json.dumps(terminal))

    def test_failure_before_resume_is_rejected_without_invocation(self) -> None:
        calls: list[str] = []
        result, terminal = run_main_case(
            calls=calls,
            readiness_error=driver.FreshBootResumeDriverError("synthetic-drift"),
        )

        self.assertEqual(result, 10)
        self.assertEqual(calls, [])
        self.assertEqual(terminal["outcome"], "resume-rejected")
        self.assertEqual(terminal["maintenance_resume_invocations"], 0)
        self.assertEqual(
            terminal["transaction"], "artifacts-staged-preserved-no-resume"
        )

    def test_failure_after_resume_start_is_indeterminate_without_retry(self) -> None:
        calls: list[str] = []

        def fail_resume(_root, _case, phase):
            calls.append(phase)
            raise driver.FreshBootResumeDriverError("synthetic-result-loss")

        result, terminal = run_main_case(calls=calls, run_case=fail_resume)

        self.assertEqual(result, 12)
        self.assertEqual(calls, ["resume"])
        self.assertEqual(terminal["outcome"], "state-indeterminate")
        self.assertEqual(terminal["maintenance_resume_invocations"], 1)
        self.assertEqual(terminal["automatic_retry"], "not-performed")

    def test_source_has_no_utmctl_retry_cleanup_or_stop_path(self) -> None:
        source = Path(driver.__file__).read_text(encoding="utf-8")
        self.assertNotIn("utmctl", source)
        self.assertNotIn('run_case(args.control_root, terminal_case, "retry")', source)
        self.assertIn('run_case(args.control_root, terminal_case, "resume")', source)
        self.assertIn('run_case(args.control_root, terminal_case, "postflight")', source)

    def test_failure_reason_redacts_raw_operation_id(self) -> None:
        reason = driver.sanitize_reason(f"cannot-open:/run/{RAW_SECRET}/receipt")
        self.assertNotIn(RAW_SECRET, reason)
        self.assertIn("[redacted]", reason)


def run_main_case(
    *,
    calls: list[str],
    readiness_error: Exception | None = None,
    run_case=None,
) -> tuple[int, dict[str, object]]:
    frozen = SimpleNamespace()
    recovery = SimpleNamespace()
    terminals: list[dict[str, object]] = []

    def successful_case(_root, _case, phase):
        calls.append(phase)
        stdout = {
            "resume": b"install_artifacts_staged_resume_outcome=completed\n",
            "postflight": b"install_artifacts_staged_postflight_outcome=passed\n",
        }[phase]
        return SimpleNamespace(returncode=0, stdout=stdout, stderr=b"")

    frozen.derive_terminal_case = lambda root: (root / "terminal-case.sh", "b" * 64)
    frozen.run_case = run_case or successful_case
    frozen.require_case_success = lambda observation, expected, phase: (
        None
        if observation.returncode == 0
        and observation.stdout == expected
        and observation.stderr == b""
        else (_ for _ in ()).throw(
            driver.FreshBootResumeDriverError(f"case-{phase}-failed")
        )
    )

    with tempfile.TemporaryDirectory() as temporary:
        root = Path(temporary)
        driver_sha = hashlib.sha256(Path(driver.__file__).read_bytes()).hexdigest()
        with mock.patch.object(driver.os, "geteuid", return_value=0), mock.patch.object(
            driver, "EXPECTED_CONTROL_ROOT", root
        ), mock.patch.object(driver, "require_private_directory"), mock.patch.object(
            driver, "require_regular"
        ), mock.patch.object(
            driver, "create_new_file"
        ), mock.patch.object(
            driver, "replace_phase"
        ), mock.patch.object(
            driver, "load_module", side_effect=[frozen, recovery]
        ), mock.patch.object(
            driver,
            "validate_fresh_boot_readiness",
            side_effect=readiness_error or None,
            return_value=RAW_SECRET,
        ), mock.patch.object(
            driver, "reconstruct_transient_secret"
        ), mock.patch.object(
            driver, "revalidate_before_resume"
        ), mock.patch.object(
            driver, "validate_completed_result", return_value=completed_artifacts()
        ), mock.patch.object(
            driver,
            "publish_terminal",
            side_effect=lambda _root, value: terminals.append(value),
        ):
            result = driver.main(valid_argv(root, driver_sha))
    return result, terminals[0]


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
        driver.EXPECTED_ATTEMPT_ID,
        "--checkpoint-attempt-id",
        driver.EXPECTED_CHECKPOINT_ATTEMPT_ID,
        "--target-uuid",
        driver.EXPECTED_TARGET_UUID,
        "--prior-boot-id-sha256",
        driver.EXPECTED_PRIOR_BOOT_ID_SHA256,
        "--current-boot-id-sha256",
        driver.EXPECTED_CURRENT_BOOT_ID_SHA256,
        "--expected-operation-id-sha256",
        driver.EXPECTED_OPERATION_ID_SHA256,
        "--expected-checkpoint-sha256",
        driver.EXPECTED_CHECKPOINT_SHA256,
        "--expected-crash-state-sha256",
        driver.EXPECTED_CRASH_STATE_SHA256,
        "--resume-driver",
        str(control_root / "frozen-resume-driver.py"),
        "--expected-resume-driver-sha256",
        driver.EXPECTED_RESUME_DRIVER_SHA256,
        "--recovery-probe",
        str(control_root / "frozen-recovery-probe.py"),
        "--expected-recovery-probe-sha256",
        driver.EXPECTED_RECOVERY_PROBE_SHA256,
        "--expected-driver-sha256",
        driver_sha256,
        "--control-root",
        str(control_root),
    ]


if __name__ == "__main__":
    unittest.main()
