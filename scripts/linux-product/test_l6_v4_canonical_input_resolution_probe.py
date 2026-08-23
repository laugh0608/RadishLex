#!/usr/bin/env python3
from __future__ import annotations

import hashlib
import json
import os
import tempfile
import unittest
from pathlib import Path
from unittest import mock

import l6_utm_canonical_input_resolution as resolution
import l6_v4_canonical_input_resolution_probe as probe


class LinuxL6CanonicalInputResolutionProbeTests(unittest.TestCase):
    def test_probe_observes_static_paths_without_modifying_them(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request, final_root, staging_root = make_probe_fixture(
                Path(temporary)
            )
            before = fixture_hashes(request, final_root)
            with mock.patch.object(
                probe.ResolutionProbeRequest, "validate", return_value=None
            ):
                evidence, exit_code = probe.run_resolution_probe(
                    request,
                    identity_uid=os.getuid(),
                    identity_gid=os.getgid(),
                    final_input_root=final_root,
                    staging_root=staging_root,
                    process_observer=lambda _: {
                        "active_installer_count": 0,
                        "suspicious_reference_count": 0,
                    },
                )

            self.assertEqual(exit_code, 0)
            self.assertEqual(evidence["outcome"], "passed")
            self.assertEqual(evidence["final_input_root_state"], "private-directory")
            self.assertEqual(
                evidence["final_input_inventory_identity"], "matched"
            )
            self.assertEqual(evidence["staging_root_state"], "absent")
            self.assertEqual(before, fixture_hashes(request, final_root))
            self.assertFalse(staging_root.exists())

    def test_probe_active_process_stops_before_original_path_reads(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            resolution_root = Path(temporary) / "resolution"
            resolution_root.mkdir(mode=0o700)
            resolution_root.chmod(0o700)
            probe_path = resolution_root / "resolution-probe.py"
            probe_path.write_bytes(b"probe")
            probe_path.chmod(0o600)
            request = make_probe_request(
                resolution_root=resolution_root,
                transfer_root=Path(temporary) / "missing-transfer",
                installer=b"installer",
                bundle=b"bundle",
                transfer_evidence=b"evidence",
            )
            with mock.patch.object(
                probe.ResolutionProbeRequest, "validate", return_value=None
            ):
                evidence, exit_code = probe.run_resolution_probe(
                    request,
                    identity_uid=os.getuid(),
                    identity_gid=os.getgid(),
                    final_input_root=Path(temporary) / "missing-final",
                    process_observer=lambda _: {
                        "active_installer_count": 1,
                        "suspicious_reference_count": 0,
                    },
                )
            self.assertEqual(exit_code, 10)
            self.assertEqual(evidence["outcome"], "indeterminate")
            self.assertEqual(evidence["transfer_root_state"], "unknown")

    def test_probe_rejects_current_final_input_inventory_drift(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request, final_root, staging_root = make_probe_fixture(
                Path(temporary)
            )
            final_file = final_root / "source/synthetic-input.txt"
            final_file.write_bytes(b"drifted")
            final_file.chmod(0o600)
            with mock.patch.object(
                probe.ResolutionProbeRequest, "validate", return_value=None
            ):
                evidence, exit_code = probe.run_resolution_probe(
                    request,
                    identity_uid=os.getuid(),
                    identity_gid=os.getgid(),
                    final_input_root=final_root,
                    staging_root=staging_root,
                    process_observer=lambda _: {
                        "active_installer_count": 0,
                        "suspicious_reference_count": 0,
                    },
                )

            self.assertEqual(exit_code, probe.EXIT_INDETERMINATE)
            self.assertEqual(evidence["outcome"], "indeterminate")
            self.assertEqual(
                evidence["final_input_root_state"], "private-directory"
            )
            self.assertEqual(
                evidence["final_input_inventory_identity"], "not-observed"
            )

    def test_proc_observer_counts_exact_and_suspicious_references(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            request = make_probe_request(
                resolution_root=Path(
                    "/var/tmp/radishlex-l6-v4-input-resolution-resolve-v1"
                ),
                transfer_root=Path(
                    "/var/tmp/radishlex-l6-v4-input-transfer-"
                    f"{resolution.REQUIRED_TRANSFER_ATTEMPT_ID}"
                ),
                installer=b"installer",
                bundle=b"bundle",
                transfer_evidence=b"evidence",
            )
            exact = root / "900001"
            exact.mkdir()
            exact_cmdline = (
                "/usr/bin/python3",
                str(request.installer_path),
                "--attempt-id",
                request.transfer_attempt_id,
                "--transfer-root",
                str(request.transfer_root),
                "--bundle-path",
                str(request.bundle_path),
                "--expected-bundle-size",
                str(request.expected_bundle_size),
                "--expected-bundle-sha256",
                request.expected_bundle_sha256,
            )
            (exact / "cmdline").write_bytes(
                b"\0".join(item.encode("utf-8") for item in exact_cmdline)
                + b"\0"
            )
            suspicious = root / "900002"
            suspicious.mkdir()
            (suspicious / "cmdline").write_bytes(
                b"wrapper\0" + str(request.transfer_root).encode("utf-8") + b"\0"
            )

            observed = probe.observe_related_processes(
                request, proc_root=root
            )

            self.assertEqual(observed["active_installer_count"], 1)
            self.assertEqual(observed["suspicious_reference_count"], 1)


def make_probe_fixture(
    temporary_root: Path,
) -> tuple[probe.ResolutionProbeRequest, Path, Path]:
    resolution_root = temporary_root / "resolution"
    transfer_root = temporary_root / "transfer"
    final_root = temporary_root / "final"
    staging_root = temporary_root / "staging"
    for directory in (resolution_root, transfer_root, final_root):
        directory.mkdir(mode=0o700)
        directory.chmod(0o700)
    installer = b"synthetic installer"
    bundle = b"synthetic bundle"
    final_payload = b"synthetic final input"
    final_member = {
        "mode": "0600",
        "path": "source/synthetic-input.txt",
        "sha256": hashlib.sha256(final_payload).hexdigest(),
        "size": len(final_payload),
    }
    transfer_evidence_payload = (
        json.dumps(
            {"inventory": [final_member], "inventory_count": 1},
            indent=2,
            sort_keys=True,
        )
        + "\n"
    ).encode("utf-8")
    request = make_probe_request(
        resolution_root=resolution_root,
        transfer_root=transfer_root,
        installer=installer,
        bundle=bundle,
        transfer_evidence=transfer_evidence_payload,
    )
    for path, payload in (
        (request.probe_path, b"synthetic probe"),
        (request.installer_path, installer),
        (request.bundle_path, bundle),
        (
            request.transfer_marker_path,
            (request.transfer_attempt_id + "\n").encode("ascii"),
        ),
        (request.transfer_evidence_path, transfer_evidence_payload),
    ):
        path.write_bytes(payload)
        path.chmod(0o600)
    final_file = final_root / final_member["path"]
    final_file.parent.mkdir(mode=0o700)
    final_file.parent.chmod(0o700)
    final_file.write_bytes(final_payload)
    final_file.chmod(0o600)
    return request, final_root, staging_root


def make_probe_request(
    *,
    resolution_root: Path,
    transfer_root: Path,
    installer: bytes,
    bundle: bytes,
    transfer_evidence: bytes,
) -> probe.ResolutionProbeRequest:
    return probe.ResolutionProbeRequest(
        resolution_attempt_id="d75818f-v4-input-resolution-20260823-v1",
        transfer_attempt_id=resolution.REQUIRED_TRANSFER_ATTEMPT_ID,
        resolution_root=resolution_root,
        transfer_root=transfer_root,
        expected_installer_size=len(installer),
        expected_installer_sha256=hashlib.sha256(installer).hexdigest(),
        expected_bundle_size=len(bundle),
        expected_bundle_sha256=hashlib.sha256(bundle).hexdigest(),
        expected_transfer_evidence_size=len(transfer_evidence),
        expected_transfer_evidence_sha256=hashlib.sha256(
            transfer_evidence
        ).hexdigest(),
    )


def fixture_hashes(
    request: probe.ResolutionProbeRequest, final_root: Path
) -> dict[str, str]:
    paths = (
        request.installer_path,
        request.bundle_path,
        request.transfer_marker_path,
        request.transfer_evidence_path,
    )
    values = {
        str(path): hashlib.sha256(path.read_bytes()).hexdigest()
        for path in paths
    }
    values[str(final_root)] = str(final_root.stat().st_mode & 0o777)
    for path in sorted(final_root.rglob("*")):
        values[str(path)] = (
            hashlib.sha256(path.read_bytes()).hexdigest()
            if path.is_file()
            else str(path.stat().st_mode & 0o777)
        )
    return values


if __name__ == "__main__":
    unittest.main()
