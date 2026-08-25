#!/usr/bin/env python3
from __future__ import annotations

import hashlib
import os
import tempfile
import unittest
from pathlib import Path

import l6_v4_boot_transport_probe as probe


ATTEMPT_ID = "synthetic-boot-transport-attempt-v1"
TARGET_UUID = "12345678-1234-4234-8234-123456789ABC"
BOOT_ID = "aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee"


class BootTransportProbeTests(unittest.TestCase):
    def test_writes_only_canonical_boot_hash_with_create_new_files(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "control"
            root.mkdir(mode=0o700)
            boot_path = Path(temporary) / "boot_id"
            boot_path.write_text(BOOT_ID + "\n", encoding="ascii")
            probe_path = Path(temporary) / "probe.py"
            probe_path.write_bytes(b"synthetic committed probe\n")
            probe_sha256 = hashlib.sha256(probe_path.read_bytes()).hexdigest()

            observed = probe.run_probe(
                attempt_id=ATTEMPT_ID,
                target_uuid=TARGET_UUID,
                expected_probe_sha256=probe_sha256,
                control_root=root,
                boot_id_path=boot_path,
                probe_path=probe_path,
                expected_owner_uid=os.getuid(),
                expected_owner_gid=os.getgid(),
                expected_control_root=root,
            )

            expected_hash = hashlib.sha256(BOOT_ID.encode("ascii")).hexdigest()
            self.assertEqual(observed, expected_hash)
            marker = root / "attempt.marker.json"
            result = root / "boot-identity.evidence.json"
            self.assertEqual(
                marker.read_bytes(),
                probe.marker_bytes(ATTEMPT_ID, probe_sha256),
            )
            self.assertEqual(
                result.read_bytes(),
                probe.result_bytes(
                    ATTEMPT_ID,
                    TARGET_UUID,
                    probe_sha256,
                    expected_hash,
                ),
            )
            self.assertNotIn(BOOT_ID, result.read_text(encoding="ascii"))
            self.assertEqual(marker.stat().st_mode & 0o777, 0o600)
            self.assertEqual(result.stat().st_mode & 0o777, 0o600)

    def test_second_attempt_cannot_overwrite_marker_or_result(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root, boot_path, probe_path, probe_sha256 = prepare(temporary)
            arguments = {
                "attempt_id": ATTEMPT_ID,
                "target_uuid": TARGET_UUID,
                "expected_probe_sha256": probe_sha256,
                "control_root": root,
                "boot_id_path": boot_path,
                "probe_path": probe_path,
                "expected_owner_uid": os.getuid(),
                "expected_owner_gid": os.getgid(),
                "expected_control_root": root,
            }
            probe.run_probe(**arguments)
            before = (root / "boot-identity.evidence.json").read_bytes()

            with self.assertRaisesRegex(
                probe.BootTransportProbeError, "create-new-failed"
            ):
                probe.run_probe(**arguments)

            self.assertEqual(
                (root / "boot-identity.evidence.json").read_bytes(), before
            )

    def test_invalid_boot_id_leaves_no_result_or_raw_value(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root, boot_path, probe_path, probe_sha256 = prepare(temporary)
            boot_path.write_text("not-a-boot-id\n", encoding="ascii")

            with self.assertRaisesRegex(
                probe.BootTransportProbeError, "boot-id-invalid"
            ):
                probe.run_probe(
                    attempt_id=ATTEMPT_ID,
                    target_uuid=TARGET_UUID,
                    expected_probe_sha256=probe_sha256,
                    control_root=root,
                    boot_id_path=boot_path,
                    probe_path=probe_path,
                    expected_owner_uid=os.getuid(),
                    expected_owner_gid=os.getgid(),
                    expected_control_root=root,
                )

            self.assertTrue((root / "attempt.marker.json").is_file())
            self.assertFalse((root / "boot-identity.evidence.json").exists())
            self.assertNotIn(
                "not-a-boot-id",
                (root / "attempt.marker.json").read_text(encoding="ascii"),
            )


def prepare(
    temporary: str,
) -> tuple[Path, Path, Path, str]:
    root = Path(temporary) / "control"
    root.mkdir(mode=0o700)
    boot_path = Path(temporary) / "boot_id"
    boot_path.write_text(BOOT_ID + "\n", encoding="ascii")
    probe_path = Path(temporary) / "probe.py"
    probe_path.write_bytes(b"synthetic committed probe\n")
    probe_sha256 = hashlib.sha256(probe_path.read_bytes()).hexdigest()
    return root, boot_path, probe_path, probe_sha256


if __name__ == "__main__":
    unittest.main()
