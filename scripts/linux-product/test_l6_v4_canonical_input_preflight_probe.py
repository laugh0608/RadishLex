#!/usr/bin/env python3
from __future__ import annotations

import argparse
import hashlib
import json
import tempfile
import unittest
from pathlib import Path
from unittest import mock

import l6_v4_canonical_input_preflight_probe as probe


BOOT_SHA256 = hashlib.sha256(b"synthetic-boot-id").hexdigest()
PROBE_BYTES = b"synthetic negative preflight probe\n"


class LinuxL6CanonicalInputPreflightProbeTests(unittest.TestCase):
    def test_probe_passes_once_and_writes_canonical_evidence(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            probe_path = root / "negative-preflight.py"
            probe_path.write_bytes(PROBE_BYTES)
            args = make_args(root, probe_path)

            with probe_patches(root, probe_path):
                exit_code = probe.execute_probe(args)

            self.assertEqual(exit_code, probe.EXIT_PASSED)
            marker = root / "attempt.marker.json"
            evidence_path = root / "negative-preflight.evidence.json"
            self.assertEqual(
                marker.read_bytes(),
                probe.expected_marker_bytes(
                    args.preflight_attempt_id,
                    args.expected_probe_sha256,
                ),
            )
            payload = evidence_path.read_bytes()
            self.assertTrue(payload.endswith(b"\n"))
            evidence = json.loads(payload)
            self.assertEqual(evidence["outcome"], "passed")
            self.assertEqual(evidence["phase"], "complete")
            self.assertEqual(evidence["operation_id"], "not-generated")
            self.assertEqual(evidence["case_invocations"], 0)
            self.assertEqual(evidence["maintenance_invocations"], 0)
            self.assertEqual(evidence["acceptance_invocations"], 0)
            self.assertEqual(evidence["dpkg_invocations"], 0)

    def test_failed_identity_check_is_frozen_as_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            probe_path = root / "negative-preflight.py"
            probe_path.write_bytes(PROBE_BYTES)
            args = make_args(root, probe_path)

            with probe_patches(root, probe_path), mock.patch.object(
                probe,
                "validate_input_inventory",
                side_effect=probe.NegativePreflightProbeError(
                    "synthetic-input-drift"
                ),
            ):
                exit_code = probe.execute_probe(args)

            self.assertEqual(exit_code, probe.EXIT_REJECTED)
            evidence = json.loads(
                (root / "negative-preflight.evidence.json").read_bytes()
            )
            self.assertEqual(evidence["outcome"], "predicate_failed")
            self.assertEqual(evidence["phase"], "input-inventory")
            self.assertIn("synthetic-input-drift", evidence["reason"])
            self.assertEqual(evidence["operation_id"], "not-generated")
            phase = json.loads((root / "phase.json").read_bytes())
            self.assertEqual(phase["phase"], evidence["phase"])

    def test_preexisting_marker_prevents_second_invocation(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            probe_path = root / "negative-preflight.py"
            probe_path.write_bytes(PROBE_BYTES)
            (root / "attempt.marker.json").write_bytes(b"frozen\n")
            args = make_args(root, probe_path)

            with probe_patches(root, probe_path):
                exit_code = probe.execute_probe(args)

            self.assertEqual(exit_code, probe.EXIT_INDETERMINATE)
            self.assertFalse(
                (root / "negative-preflight.evidence.json").exists()
            )

    def test_dpkg_status_parser_rejects_duplicate_package(self) -> None:
        payload = (
            "Package: synthetic\nStatus: install ok installed\n\n"
            "Package: synthetic\nStatus: install ok installed\n"
        )
        with self.assertRaisesRegex(
            probe.NegativePreflightProbeError,
            "dpkg-status-duplicate-package",
        ):
            probe.parse_dpkg_status(payload)

    def test_network_requires_empty_main_routes(self) -> None:
        def command(argv: tuple[str, ...]) -> bytes:
            if argv[1:3] == ("-4", "route"):
                return b"default via 192.0.2.1 dev eth0\n"
            return b""

        with mock.patch.object(
            probe.Path,
            "iterdir",
            return_value=[Path("/sys/class/net/lo")],
        ):
            with self.assertRaisesRegex(
                probe.NegativePreflightProbeError,
                "ipv4-main-route-present",
            ):
                probe.validate_network(command)


def make_args(root: Path, probe_path: Path) -> argparse.Namespace:
    return argparse.Namespace(
        preflight_attempt_id="d75818f-v4-negative-preflight-20260823-v1",
        transfer_attempt_id="d75818f-v4-input-20260823-v1",
        resolution_attempt_id="d75818f-v4-input-resolution-20260823-v1",
        preflight_root=str(root),
        probe_path=str(probe_path),
        expected_probe_size=len(PROBE_BYTES),
        expected_probe_sha256=hashlib.sha256(PROBE_BYTES).hexdigest(),
        expected_boot_id_sha256=BOOT_SHA256,
    )


def write_exclusive_for_test(path: Path, payload: bytes) -> None:
    with path.open("xb") as destination:
        destination.write(payload)


def probe_patches(root: Path, probe_path: Path):
    del root, probe_path
    patches = (
        mock.patch.object(probe.os, "geteuid", return_value=0),
        mock.patch.object(probe.os, "getegid", return_value=0),
        mock.patch.object(probe, "require_private_directory"),
        mock.patch.object(probe, "require_regular_identity"),
        mock.patch.object(probe, "write_exclusive", side_effect=write_exclusive_for_test),
        mock.patch.object(probe, "sync_directory"),
        mock.patch.object(probe, "current_boot_id_sha256", return_value=BOOT_SHA256),
        mock.patch.object(probe, "validate_input_inventory"),
        mock.patch.object(probe, "validate_release_pair"),
        mock.patch.object(probe, "validate_negative_state"),
        mock.patch.object(
            probe,
            "validate_package_and_dependencies",
            return_value=(hashlib.sha256(b"synthetic-dpkg-log").hexdigest(), 42),
        ),
        mock.patch.object(probe, "validate_xdg_absent"),
        mock.patch.object(probe, "validate_processes"),
        mock.patch.object(probe, "validate_network"),
        mock.patch.object(probe, "run_startup", return_value=probe.EXPECTED_STARTUP_OUTPUT),
    )
    return _PatchGroup(patches)


class _PatchGroup:
    def __init__(self, patches: tuple[mock._patch, ...]) -> None:
        self.patches = patches

    def __enter__(self):
        for patch in self.patches:
            patch.start()
        return self

    def __exit__(self, exc_type, exc, traceback) -> None:
        for patch in reversed(self.patches):
            patch.stop()


if __name__ == "__main__":
    unittest.main()
