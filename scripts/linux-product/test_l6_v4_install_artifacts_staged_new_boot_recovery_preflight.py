#!/usr/bin/env python3
from __future__ import annotations

import hashlib
import stat
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest import mock

import l6_v4_install_artifacts_staged_new_boot_recovery_preflight as probe


class NewBootRecoveryPreflightProbeTests(unittest.TestCase):
    def test_persistent_transaction_uses_product_directory_modes(self) -> None:
        operation_id = "a" * 32
        state_root = Path("/var/lib/radishlex/install-v1")
        operations_root = state_root / "operations"
        operation_root = operations_root / operation_id
        receipt = {
            "failure_after_state": None,
            "failure_code": None,
            "distribution_identity": "debian-local-deb-v1",
            "initial_package": {"artifact": None, "state": "not_installed"},
            "manual_recovery_required": False,
            "operation_chain": [operation_id],
            "operation_id": operation_id,
            "operation_kind": "install",
            "product_id": "radishlex-linux",
            "receipt_format": "radishlex-linux-install-receipt-v1",
            "source_artifact": None,
            "source_proof": None,
            "staged_artifacts": [
                {
                    "artifact": {
                        "evidence_sha256": "evidence",
                        "package_sha256": "package",
                    },
                    "slot": "target",
                }
            ],
            "state": "artifacts_staged",
            "target_artifact": {
                "evidence_sha256": "evidence",
                "package_sha256": "package",
                "package_version": "26.7.1+38-1",
            },
            "target_proof": None,
            "version_relation": "not_applicable",
        }
        driver = SimpleNamespace(
            STATE_ROOT=state_root,
            RECEIPT_PATH=state_root / "receipt.json",
            EXPECTED_RECEIPT_SIZE=1,
            EXPECTED_RECEIPT_SHA256="receipt",
            EXPECTED_OPERATION_ID_SHA256=probe.EXPECTED_OPERATION_ID_SHA256,
            EXPECTED_SOURCE_PACKAGE_SHA256="package",
            EXPECTED_SOURCE_EVIDENCE_SHA256="evidence",
            HEX_32=probe.re.compile(r"[0-9a-f]{32}"),
            read_bounded=mock.Mock(return_value=probe.json.dumps(receipt).encode()),
            sha256_bytes=mock.Mock(
                return_value=probe.EXPECTED_OPERATION_ID_SHA256
            ),
            require_regular=mock.Mock(),
        )
        directory_modes: list[tuple[Path, str, int]] = []
        with mock.patch.object(
            probe,
            "require_directory",
            side_effect=lambda path, label, mode: directory_modes.append(
                (path, label, mode)
            ),
        ), mock.patch.object(
            probe, "require_private_directory"
        ) as private, mock.patch.object(
            probe.Path, "exists", return_value=False
        ), mock.patch.object(
            probe.Path,
            "iterdir",
            side_effect=(
                (operation_root,),
                (
                    operation_root / "target.deb",
                    operation_root / "target.evidence.json",
                ),
            ),
        ):
            result = probe.validate_persistent_transaction(driver)

        self.assertEqual(result, operation_id)
        self.assertEqual(
            directory_modes,
            [
                (state_root, "state-root", 0o755),
                (operations_root, "operations-root", 0o755),
            ],
        )
        private.assert_called_once_with(operation_root, "operation-root")
        self.assertFalse(any("/run/" in str(call) for call in directory_modes))

    def test_absent_guard_maps_to_operation_in_progress_startup(self) -> None:
        guard = mock.Mock()
        guard.lstat.side_effect = FileNotFoundError
        driver = SimpleNamespace(GUARD_PATH=guard)

        profile, startup = probe.classify_guard(driver)

        self.assertEqual(profile, "absent-after-reboot")
        self.assertEqual(startup, probe.EXPECTED_OPERATION_IN_PROGRESS_STARTUP)

    def test_valid_unlocked_guard_maps_to_active_guard_startup(self) -> None:
        guard = mock.Mock()
        guard.lstat.return_value = SimpleNamespace(
            st_mode=stat.S_IFREG | 0o600,
            st_uid=0,
            st_gid=0,
            st_nlink=1,
            st_size=0,
        )
        driver = SimpleNamespace(GUARD_PATH=guard)
        with mock.patch.object(probe.os, "open", return_value=19), mock.patch.object(
            probe.os, "close"
        ), mock.patch.object(probe.fcntl, "flock") as flock:
            profile, startup = probe.classify_guard(driver)

        self.assertEqual(profile, "present-valid-unlocked")
        self.assertEqual(startup, probe.EXPECTED_ACTIVE_GUARD_STARTUP)
        self.assertEqual(flock.call_count, 2)

    def test_locked_guard_rejects_recovery_qualification(self) -> None:
        guard = mock.Mock()
        guard.lstat.return_value = SimpleNamespace(
            st_mode=stat.S_IFREG | 0o600,
            st_uid=0,
            st_gid=0,
            st_nlink=1,
            st_size=0,
        )
        driver = SimpleNamespace(GUARD_PATH=guard)
        with mock.patch.object(probe.os, "open", return_value=19), mock.patch.object(
            probe.os, "close"
        ), mock.patch.object(
            probe.fcntl,
            "flock",
            side_effect=(BlockingIOError(), None),
        ):
            with self.assertRaisesRegex(
                probe.RecoveryPreflightError, "guard-active"
            ):
                probe.classify_guard(driver)

    def test_live_validation_temporarily_binds_new_boot_and_guard_profile(self) -> None:
        observed: list[tuple[str, str]] = []
        driver = SimpleNamespace(
            EXPECTED_BOOT_ID_SHA256="prior",
            EXPECTED_ACTIVE_GUARD_STARTUP="active",
        )

        def validate() -> None:
            observed.append(
                (
                    driver.EXPECTED_BOOT_ID_SHA256,
                    driver.EXPECTED_ACTIVE_GUARD_STARTUP,
                )
            )

        driver.validate_live_system = validate
        probe.validate_live_new_boot(
            driver, probe.EXPECTED_OPERATION_IN_PROGRESS_STARTUP
        )

        self.assertEqual(
            observed,
            [
                (
                    probe.EXPECTED_CURRENT_BOOT_ID_SHA256,
                    probe.EXPECTED_OPERATION_IN_PROGRESS_STARTUP,
                )
            ],
        )
        self.assertEqual(driver.EXPECTED_BOOT_ID_SHA256, "prior")
        self.assertEqual(driver.EXPECTED_ACTIVE_GUARD_STARTUP, "active")

    def test_fresh_attempt_accepts_bound_dynamic_boot_but_not_original(self) -> None:
        current_boot = "c" * 64
        control_root = probe.expected_control_root_for(
            probe.FRESH_BOOT_RECOVERY_ATTEMPT_ID
        )
        args = probe.parse_args(
            fresh_argv(control_root, "d" * 64, current_boot)
        )

        probe.validate_argument_contract(args)

        args.current_boot_id_sha256 = probe.EXPECTED_PRIOR_BOOT_ID_SHA256
        with self.assertRaisesRegex(
            probe.RecoveryPreflightError, "fresh-boot-identity-not-new"
        ):
            probe.validate_argument_contract(args)

    def test_main_is_read_only_and_never_dispatches_resume(self) -> None:
        terminals: list[dict[str, object]] = []
        phases: list[str] = []
        driver = SimpleNamespace(
            EXPECTED_OPERATION_ID_SHA256=probe.EXPECTED_OPERATION_ID_SHA256,
            run_case=mock.Mock(),
            derive_terminal_case=mock.Mock(),
            validate_resume_result=mock.Mock(),
        )
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            probe_sha256 = hashlib.sha256(Path(probe.__file__).read_bytes()).hexdigest()
            with mock.patch.object(
                probe.os, "geteuid", return_value=0
            ), mock.patch.object(
                probe, "EXPECTED_CONTROL_ROOT", root
            ), mock.patch.object(
                probe, "require_private_directory"
            ), mock.patch.object(
                probe, "require_regular"
            ), mock.patch.object(
                probe, "load_resume_driver", return_value=driver
            ), mock.patch.object(
                probe, "create_new_file"
            ), mock.patch.object(
                probe,
                "replace_phase",
                side_effect=lambda _root, phase: phases.append(phase),
            ), mock.patch.object(
                probe, "validate_persistent_transaction"
            ), mock.patch.object(
                probe,
                "classify_guard",
                return_value=(
                    "absent-after-reboot",
                    probe.EXPECTED_OPERATION_IN_PROGRESS_STARTUP,
                ),
            ), mock.patch.object(
                probe, "validate_live_new_boot"
            ), mock.patch.object(
                probe,
                "publish_terminal",
                side_effect=lambda _root, value: terminals.append(value),
            ):
                result = probe.main(valid_argv(root, probe_sha256))

        self.assertEqual(result, 0)
        self.assertEqual(
            phases,
            [
                "persistent-transaction",
                "cross-reboot-guard",
                "live-new-boot",
                "complete",
            ],
        )
        self.assertEqual(terminals[0]["outcome"], "recovery-qualified")
        self.assertEqual(terminals[0]["maintenance_resume_invocations"], 0)
        self.assertFalse(terminals[0]["dpkg_mutation_executed"])
        driver.run_case.assert_not_called()
        driver.derive_terminal_case.assert_not_called()
        driver.validate_resume_result.assert_not_called()


def valid_argv(control_root: Path, probe_sha256: str) -> list[str]:
    return [
        "--attempt-id",
        probe.EXPECTED_ATTEMPT_ID,
        "--target-uuid",
        probe.EXPECTED_TARGET_UUID,
        "--prior-boot-id-sha256",
        probe.EXPECTED_PRIOR_BOOT_ID_SHA256,
        "--current-boot-id-sha256",
        probe.EXPECTED_CURRENT_BOOT_ID_SHA256,
        "--resume-driver",
        str(control_root / "resume.py"),
        "--expected-resume-driver-sha256",
        probe.EXPECTED_RESUME_DRIVER_SHA256,
        "--expected-probe-sha256",
        probe_sha256,
        "--control-root",
        str(control_root),
    ]


def fresh_argv(
    control_root: Path, probe_sha256: str, current_boot: str
) -> list[str]:
    value = valid_argv(control_root, probe_sha256)
    value[value.index(probe.EXPECTED_ATTEMPT_ID)] = (
        probe.FRESH_BOOT_RECOVERY_ATTEMPT_ID
    )
    value[value.index(probe.EXPECTED_CURRENT_BOOT_ID_SHA256)] = current_boot
    return value


if __name__ == "__main__":
    unittest.main()
