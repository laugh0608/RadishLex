#!/usr/bin/env python3
from __future__ import annotations

import hashlib
import json
import tempfile
import unittest
from pathlib import Path
from unittest import mock

import l6_v4_install_artifacts_staged_checkpoint_driver as driver


BOOT_SHA256 = hashlib.sha256(b"synthetic-boot").hexdigest()
SECRET = b"0" * 32
OPERATION_SHA256 = hashlib.sha256(SECRET).hexdigest()
GUEST_IDENTITY_SHA256 = "2" * 64


class LinuxL6InstallArtifactsStagedCheckpointDriverTests(unittest.TestCase):
    def test_negative_preflight_requires_canonical_passed_evidence(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "negative-preflight.json"
            value = passed_negative_preflight()
            payload = driver.canonical_json(value)
            path.write_bytes(payload)

            parsed = driver.validate_negative_preflight(
                path, hashlib.sha256(payload).hexdigest(), BOOT_SHA256
            )

            self.assertEqual(parsed["outcome"], "passed")
            self.assertEqual(parsed["operation_id"], "not-generated")

    def test_negative_preflight_rejects_generated_operation(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "negative-preflight.json"
            value = passed_negative_preflight()
            value["operation_id"] = "generated"
            payload = driver.canonical_json(value)
            path.write_bytes(payload)

            with self.assertRaisesRegex(
                driver.CheckpointDriverError, "semantics-invalid"
            ):
                driver.validate_negative_preflight(
                    path, hashlib.sha256(payload).hexdigest(), BOOT_SHA256
                )

    def test_field_parser_rejects_duplicate_and_noncanonical_lines(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "fields.txt"
            path.write_bytes(b"key=one\nkey=two\n")
            with self.assertRaisesRegex(
                driver.CheckpointDriverError, "field-file-invalid"
            ):
                driver.read_fields(path)
            path.write_bytes(b"key=one\r\n")
            with self.assertRaisesRegex(
                driver.CheckpointDriverError, "field-file-not-canonical"
            ):
                driver.read_fields(path)

    def test_checkpoint_state_exports_only_operation_hash(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            output = root / "output"
            run = root / "run"
            checkpoint_root = root / "evidence"
            output.mkdir()
            run.mkdir()
            (checkpoint_root / "checkpoints").mkdir(parents=True)
            write_checkpoint_fixture(output, run, checkpoint_root, SECRET)

            with mock.patch.multiple(
                driver,
                OUTPUT_ROOT=output,
                RUN_ROOT=run,
                CHECKPOINT_ROOT=checkpoint_root,
            ), mock.patch.object(driver, "require_regular"):
                state = driver.validate_checkpoint_state(
                    GUEST_IDENTITY_SHA256,
                    BOOT_SHA256,
                    passed_negative_preflight(),
                )

            self.assertEqual(state["operation_id_sha256"], OPERATION_SHA256)
            self.assertNotIn("operation_id", state)
            self.assertNotIn(SECRET.decode("ascii"), json.dumps(state))

    def test_checkpoint_state_rejects_secret_hash_mismatch(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            output = root / "output"
            run = root / "run"
            checkpoint_root = root / "evidence"
            output.mkdir()
            run.mkdir()
            (checkpoint_root / "checkpoints").mkdir(parents=True)
            write_checkpoint_fixture(output, run, checkpoint_root, b"1" * 32)

            with mock.patch.multiple(
                driver,
                OUTPUT_ROOT=output,
                RUN_ROOT=run,
                CHECKPOINT_ROOT=checkpoint_root,
            ), mock.patch.object(driver, "require_regular"):
                with self.assertRaisesRegex(
                    driver.CheckpointDriverError, "operation-secret-invalid"
                ):
                    driver.validate_checkpoint_state(
                        GUEST_IDENTITY_SHA256,
                        BOOT_SHA256,
                        passed_negative_preflight(),
                    )

    def test_mutation_preflight_rejects_negative_preflight_dpkg_log_drift(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            output = root / "output"
            run = root / "run"
            checkpoint_root = root / "evidence"
            output.mkdir()
            run.mkdir()
            (checkpoint_root / "checkpoints").mkdir(parents=True)
            write_checkpoint_fixture(output, run, checkpoint_root, SECRET)
            negative = passed_negative_preflight()
            negative["dpkg_log_sha256"] = "4" * 64

            with mock.patch.object(driver, "OUTPUT_ROOT", output), mock.patch.object(
                driver, "require_regular"
            ):
                with self.assertRaisesRegex(
                    driver.CheckpointDriverError, "mutation-preflight-invalid"
                ):
                    driver.validate_mutation_preflight(
                        GUEST_IDENTITY_SHA256, BOOT_SHA256, negative
                    )


def passed_negative_preflight() -> dict[str, object]:
    return {
        "boot_id_sha256": BOOT_SHA256,
        "checkpoint_evidence": "absent",
        "dpkg_config_sha256": driver.EXPECTED_DPKG_CONFIG_SHA256,
        "dpkg_log_sha256": "3" * 64,
        "dpkg_log_size": 879640,
        "dpkg_status_sha256": driver.EXPECTED_DPKG_STATUS_SHA256,
        "fcitx_startup": driver.EXPECTED_PREFLIGHT_STARTUP,
        "guard": "absent",
        "input_inventory_count": 12,
        "input_inventory_identity": "matched",
        "manager_startup": driver.EXPECTED_PREFLIGHT_STARTUP,
        "network": "loopback-only-main-routes-empty",
        "operation_id": "not-generated",
        "outcome": "passed",
        "package": "not-installed",
        "phase": "complete",
        "product_processes": "absent",
        "receipt_terminal": "absent",
        "state_root": "absent",
        "transaction": "not-performed",
        "user_xdg": "absent",
    }


def write_checkpoint_fixture(
    output: Path, run: Path, checkpoint_root: Path, secret: bytes
) -> None:
    mutation = fields(
        format="radishlex-linux-l6-install-artifacts-staged-mutation-preflight-v1",
        boot_id_sha256=BOOT_SHA256,
        dpkg_status_sha256=driver.EXPECTED_DPKG_STATUS_SHA256,
        dpkg_log_sha256="3" * 64,
        dpkg_log_size="879640",
        package="not-installed",
        state_root="absent",
        checkpoint_evidence="absent",
        guard="absent",
        guest_identity_sha256=GUEST_IDENTITY_SHA256,
        snapshot_identity_sha256=driver.EXPECTED_SNAPSHOT_SHA256,
        manager_startup=driver.EXPECTED_PREFLIGHT_STARTUP,
        fcitx_startup=driver.EXPECTED_PREFLIGHT_STARTUP,
        startup_reason="ReceiptMissing",
        receipt_terminal="absent",
        user_xdg="absent",
        product_processes="absent",
        network="loopback-only-main-routes-empty",
        operation_id="not-generated",
        mutation_preflight="passed",
    )
    (output / "mutation-preflight.evidence.txt").write_bytes(mutation)
    crash = fields(
        format="radishlex-linux-l6-install-artifacts-staged-crash-result-v1",
        operation_id_sha256=OPERATION_SHA256,
        acceptance_invocations="1",
        checkpoint_count="1",
        fault="process_group_terminated",
        crash_result="passed",
    )
    (output / "crash-result.evidence.txt").write_bytes(crash)
    checkpoint = {
        "build_identity": "radishlex-linux-l6-acceptance-v1",
        "checkpoint": "artifacts_staged",
        "expected_terminal": "completed",
        "format": "radishlex-linux-l6-checkpoint-evidence-v1",
        "guest_identity_sha256": GUEST_IDENTITY_SHA256,
        "operation": {
            "matrix_operation": "install_source",
            "operation_id_sha256": OPERATION_SHA256,
        },
        "repository_commit": driver.EXPECTED_REPOSITORY_COMMIT,
        "scenario": "install_artifacts_staged",
        "termination": {
            "checkpoint_notification": "inherited-pipe-v1",
            "dpkg_child": "absent",
            "process_group": "terminated",
            "process_group_member_count": 0,
            "process_inspection": "complete",
            "signal": "sigkill",
            "worker": "signaled",
        },
    }
    checkpoint_payload = json.dumps(checkpoint, sort_keys=True).encode("utf-8") + b"\n"
    checkpoint_path = (
        checkpoint_root
        / "checkpoints"
        / f"install_artifacts_staged-{OPERATION_SHA256[:16]}.json"
    )
    checkpoint_path.write_bytes(checkpoint_payload)
    state = fields(
        format="radishlex-linux-l6-install-artifacts-staged-crash-state-v1",
        operation_id_sha256=OPERATION_SHA256,
        checkpoint_sha256=hashlib.sha256(checkpoint_payload).hexdigest(),
        receipt="install|not_applicable|artifacts_staged|chain-1",
        guard="present-valid-unlocked",
        dpkg_mutation_executed="false",
        dpkg_status_sha256=driver.EXPECTED_DPKG_STATUS_SHA256,
        dpkg_log_sha256="3" * 64,
        manager_startup=driver.EXPECTED_STARTUP,
        fcitx_startup=driver.EXPECTED_STARTUP,
        user_xdg="absent",
        product_processes="absent",
        network="loopback-only-main-routes-empty",
        crash_state="passed",
    )
    (output / "crash-state.evidence.txt").write_bytes(state)
    (run / "operation-id.secret").write_bytes(secret)


def fields(**values: str) -> bytes:
    return "".join(f"{key}={value}\n" for key, value in values.items()).encode(
        "utf-8"
    )


if __name__ == "__main__":
    unittest.main()
